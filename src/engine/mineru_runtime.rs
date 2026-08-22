use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
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
    install_dir: PathBuf,
    /// Captures mineru-api stdout/stderr so startup failures can be reported
    /// to the user instead of a silent 120s timeout.
    startup_log: Arc<Mutex<String>>,
}

const HEALTH_TIMEOUT_SECS: u64 = 600;
const START_BACKOFF_SECS: u64 = 3;
const MAX_RESTART_ATTEMPTS: u32 = 3;
/// Cap on captured startup log to avoid unbounded memory growth.
const STARTUP_LOG_CAP: usize = 8192;

impl MinerURuntime {
    pub fn new(port: u16, install_dir: PathBuf) -> Self {
        MinerURuntime {
            port,
            child: Mutex::new(None),
            stopping: AtomicBool::new(false),
            base_url: format!("http://127.0.0.1:{}", port),
            install_dir,
            startup_log: Arc::new(Mutex::new(String::new())),
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

        // Prefer the bundled Python's own `mineru-api` console script. MinerU
        // 3.4.5 ships the server as `python/Scripts/mineru-api.exe` (entry
        // point `mineru.cli.fast_api:main`) — there is no top-level
        // `mineru_api` module, so `python -m mineru_api` must NOT be used.
        // Falls back to a system-installed `mineru-api` on PATH otherwise.
        let bundled_api = self
            .install_dir
            .join("python")
            .join("Scripts")
            .join("mineru-api.exe");

        let (mut cmd, cmd_name): (Command, String) = if bundled_api.exists() {
            info!("Using bundled mineru-api: {}", bundled_api.display());
            (
                Command::new(&bundled_api),
                bundled_api.to_string_lossy().to_string(),
            )
        } else {
            info!("Bundled mineru-api not found, falling back to system PATH mineru-api");
            (Command::new("mineru-api"), "mineru-api".to_string())
        };

        // Reset the startup log so each (re)start captures a fresh trace.
        *self.startup_log.lock().unwrap() = String::new();
        let log = self.startup_log.clone();

        let mut child = cmd
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(self.port.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                format!(
                    "无法启动 mineru-api（{}）。请确认 MinerU 已安装并可通过命令行访问。原始错误：{}",
                    cmd_name, e
                )
            })?;

        // Spawn blocking tasks that read stdout and stderr concurrently. If they
        // were read sequentially, the second stream would be discarded while the
        // first one (the process) is still producing output, so we read both in
        // parallel on separate blocking threads.
        let stdout = child.stdout.take().expect("child stdout should be piped");
        let log_stdout = log.clone();
        tokio::task::spawn_blocking(move || {
            MinerURuntime::read_stdio(stdout, "mineru-api stdout", log_stdout);
        });
        let stderr = child.stderr.take().expect("child stderr should be piped");
        tokio::task::spawn_blocking(move || {
            MinerURuntime::read_stdio(stderr, "mineru-api stderr", log);
        });

        *self.child.lock().unwrap() = Some(child);
        Ok(())
    }

    /// Reads a std::io::Read stream on a blocking thread, logs each complete
    /// line, and appends the *entire* output (including a trailing partial line
    /// that has no newline) to the shared startup log so startup failures such
    /// as `No module named mineru_api` are never silently dropped.
    fn read_stdio<R: std::io::Read>(mut stream: R, stream_name: &str, log: Arc<Mutex<String>>) {
        let mut content = String::new();
        // `read_to_string` blocks until EOF; a closed pipe (process exit) ends it.
        if stream.read_to_string(&mut content).is_err() {
            return;
        }

        // Log each complete line for the tracing sink.
        for l in content.lines() {
            if !l.is_empty() {
                info!("[{}] {}", stream_name, l);
            }
        }

        // Append the WHOLE output (partial last line included) to the buffer.
        if !content.is_empty() {
            let mut buf = log.lock().unwrap();
            buf.push_str(stream_name);
            buf.push_str(":\n");
            buf.push_str(&content);
            if !buf.ends_with('\n') {
                buf.push('\n');
            }
            if buf.len() > STARTUP_LOG_CAP {
                let drop = buf.len() - STARTUP_LOG_CAP;
                buf.replace_range(..drop, "");
            }
        }
    }

    /// Poll until the service is listening or timeout.
    ///
    /// Compatibility note: different MinerU releases expose `/health`
    /// differently (some return 200, some 404, some don't serve it at all).
    /// We treat *any* HTTP response on the port as "the server is up and ready
    /// to accept tasks" rather than requiring a specific status code — a
    /// connection that is accepted means uvicorn is serving. A transport error
    /// (connection refused) means it is still starting (or has crashed).
    async fn wait_healthy(&self) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(HEALTH_TIMEOUT_SECS);
        let client = reqwest::Client::new();
        let url = format!("{}/health", self.base_url);

        while Instant::now() < deadline {
            if self.stopping.load(Ordering::Relaxed) {
                return Err("MinerU runtime is stopping".to_string());
            }
            // Reap a crashed child so we can detect and report it.
            self.reap_child();

            // If the child already exited, fail fast with its captured output
            // instead of waiting out the full timeout.
            if self.child.lock().unwrap().is_none() {
                // Give the reader threads a moment to flush the tail of the
                // stream into the log buffer before we read it.
                tokio::time::sleep(Duration::from_millis(300)).await;
                let log = self.startup_log.lock().unwrap().clone();
                if log.trim().is_empty() {
                    return Err(
                        "mineru-api 启动后意外退出，且未输出任何日志（静默退出）。\
请打开终端手动运行 `mineru-api --host 127.0.0.1 --port 18628` 查看真实报错，\
并确认 mineru 已用 `pip install mineru` 正确安装。"
                            .to_string(),
                    );
                }
                return Err(format!(
                    "mineru-api 启动后意外退出（可能未安装 mineru / 模块缺失 / 端口被占用）。启动日志：\n{}",
                    log
                ));
            }

            match client.get(&url).send().await {
                // Any HTTP response means the server is listening and ready.
                Ok(_) => {
                    info!("mineru-api is ready at {}", self.base_url);
                    return Ok(());
                }
                Err(_) => {
                    tokio::time::sleep(Duration::from_millis(1500)).await;
                }
            }
        }

        let log = self.startup_log.lock().unwrap().clone();
        Err(format!(
            "MinerU 服务在 {} 秒内未就绪（可能是首次加载模型，请稍后重试）。启动日志：\n{}",
            HEALTH_TIMEOUT_SECS, log
        ))
    }

    pub async fn is_healthy(&self) -> bool {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap_or_default();
        let url = format!("{}/health", self.base_url);
        // Any response (including a non-200) means the server is up.
        client.get(&url).send().await.is_ok()
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
