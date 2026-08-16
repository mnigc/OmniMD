use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tracing::{error, info, warn};

/// Manages the lifecycle of the official `mineru-api` subprocess.
///
/// OmniMD does not implement any Python service of its own; it shells out to
/// MinerU's FastAPI server (`mineru-api --host 127.0.0.1 --port <port>`) and
/// talks to it over HTTP. This struct owns the child process, performs health
/// checks and restarts the service with backoff if it crashes (crash
/// isolation requirement: a MinerU crash must never take the Tauri app down).
pub struct MinerURuntime {
    port: u16,
    child: Mutex<Option<Child>>,
    stopping: AtomicBool,
    pub base_url: String,
}

const HEALTH_TIMEOUT_SECS: u64 = 120;
const START_BACKOFF_SECS: u64 = 3;
const MAX_RESTART_ATTEMPTS: u32 = 3;

impl MinerURuntime {
    pub fn new(port: u16) -> Self {
        MinerURuntime {
            port,
            child: Mutex::new(None),
            stopping: AtomicBool::new(false),
            base_url: format!("http://127.0.0.1:{}", port),
        }
    }

    /// Start `mineru-api` on the configured port (no-op if already running
    /// and healthy). Blocks until the service answers `GET /health`.
    pub async fn ensure_running(&self) -> Result<(), String> {
        if self.is_healthy().await {
            return Ok(());
        }

        if let Some(child) = self.child.lock().unwrap().as_mut() {
            // A process exists but is unhealthy: kill it and restart below.
            let _ = child.kill();
            let _ = child.wait();
        }

        self.start_process()?;
        self.wait_healthy().await
    }

    fn start_process(&self) -> Result<(), String> {
        if self.stopping.load(Ordering::Relaxed) {
            return Err("MinerU runtime is stopping".to_string());
        }

        info!("Starting mineru-api on port {}", self.port);

        #[cfg(target_os = "windows")]
        let program = "mineru-api";

        #[cfg(not(target_os = "windows"))]
        let program = "mineru-api";

        let child = Command::new(program)
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(self.port.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                format!(
                    "无法启动 mineru-api（{}）。请确认 MinerU 已安装并可通过命令行访问。原始错误：{}",
                    program, e
                )
            })?;

        *self.child.lock().unwrap() = Some(child);
        Ok(())
    }

    /// Poll `GET /health` until ready or timeout.
    async fn wait_healthy(&self) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(HEALTH_TIMEOUT_SECS);
        let client = reqwest::Client::new();
        let url = format!("{}/health", self.base_url);

        while Instant::now() < deadline {
            if self.stopping.load(Ordering::Relaxed) {
                return Err("MinerU runtime is stopping".to_string());
            }
            // Reap a crashed child so we can detect and restart it.
            self.reap_child();

            match client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    info!("mineru-api is healthy at {}", self.base_url);
                    return Ok(());
                }
                _ => {
                    tokio::time::sleep(Duration::from_millis(1500)).await;
                }
            }
        }

        Err(format!(
            "MinerU 服务在 {} 秒内未就绪（可能是首次加载模型，请稍后重试）",
            HEALTH_TIMEOUT_SECS
        ))
    }

    pub async fn is_healthy(&self) -> bool {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap_or_default();
        let url = format!("{}/health", self.base_url);
        matches!(
            client.get(&url).send().await,
            Ok(resp) if resp.status().is_success()
        )
    }

    /// Check whether the child process has exited; if so, remove it from our
    /// bookkeeping so a subsequent `ensure_running` can spawn a fresh one.
    fn reap_child(&self) {
        let mut guard = self.child.lock().unwrap();
        if let Some(child) = guard.as_mut() {
            match child.try_wait() {
                Ok(Some(_)) => {
                    warn!("mineru-api process exited unexpectedly");
                    *guard = None;
                }
                Ok(None) => {}
                Err(e) => {
                    warn!("failed to poll mineru-api process: {}", e);
                    *guard = None;
                }
            }
        }
    }

    /// Crash-restart with backoff. Returns Ok if the service eventually
    /// becomes healthy; Err if all attempts fail.
    pub async fn restart(&self) -> Result<(), String> {
        self.reap_child();
        let mut attempt = 0u32;
        while attempt < MAX_RESTART_ATTEMPTS {
            attempt += 1;
            match self.ensure_running().await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    error!("mineru-api restart attempt {}/{} failed: {}", attempt, MAX_RESTART_ATTEMPTS, e);
                    tokio::time::sleep(Duration::from_secs(START_BACKOFF_SECS * attempt as u64)).await;
                }
            }
        }
        Err("MinerU 服务多次重启仍失败".to_string())
    }

    /// Gracefully terminate the child process.
    pub fn stop(&self) {
        self.stopping.store(true, Ordering::Relaxed);
        if let Some(mut child) = self.child.lock().unwrap().take() {
            let _ = child.kill();
            let _ = child.wait();
            info!("mineru-api stopped");
        }
    }
}

impl Drop for MinerURuntime {
    fn drop(&mut self) {
        self.stop();
    }
}
