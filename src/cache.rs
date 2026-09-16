//! 设置页“存储”区块的后端：统计并清理 WebView2 磁盘缓存与应用日志。
//!
//! 缓存清理走 WebView2 官方 Clear Browsing Data API（只传 DISK_CACHE 一类），
//! 由浏览器进程自己释放文件，运行中即可完成。localStorage（应用设置）、
//! IndexedDB、Cookie 等不在 DISK_CACHE 范围内，不会被误删。

use serde::Serialize;
use tauri::Manager;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheInfo {
    /// WebView2 磁盘缓存字节数；清理入口仅 Windows 提供，其余平台为 null。
    webview_cache_bytes: Option<u64>,
    logs_bytes: u64,
}

/// 递归统计目录大小；无法读取的条目按 0 计（文件被占用或权限不足时忽略）。
fn dir_size(path: &std::path::Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    let mut total = 0;
    for entry in entries.flatten() {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_file() {
            total += metadata.len();
        } else if metadata.is_dir() {
            total += dir_size(&entry.path());
        }
    }
    total
}

/// WebView2 用户数据目录中属于磁盘缓存的子目录（相对 EBWebView 根）。
#[cfg(windows)]
const WEBVIEW_CACHE_DIRS: &[&str] = &[
    "Default/Cache",
    "Default/Code Cache",
    "Default/Service Worker/CacheStorage",
    "Default/Service Worker/ScriptCache",
    "Default/GPUCache",
    "GPUCache",
    "GrShaderCache",
];

fn get_cache_info_inner(app: &tauri::AppHandle) -> Result<CacheInfo, String> {
    let logs_bytes = dir_size(&crate::log_dir());
    let webview_cache_bytes = webview_cache_size(app);
    Ok(CacheInfo {
        webview_cache_bytes,
        logs_bytes,
    })
}

/// Tauri 在 Windows 上把 WebView2 用户数据放在 app_local_data_dir/EBWebView。
#[cfg(windows)]
fn webview_cache_size(app: &tauri::AppHandle) -> Option<u64> {
    let root = app.path().app_local_data_dir().ok()?.join("EBWebView");
    Some(
        WEBVIEW_CACHE_DIRS
            .iter()
            .map(|dir| dir_size(&root.join(dir)))
            .sum(),
    )
}

#[cfg(not(windows))]
fn webview_cache_size(_app: &tauri::AppHandle) -> Option<u64> {
    None
}

#[tauri::command]
pub fn get_cache_info(app: tauri::AppHandle) -> Result<CacheInfo, String> {
    get_cache_info_inner(&app)
}

/// 清除 WebView2 磁盘缓存：请求运行中的浏览器进程按 DISK_CACHE 类型清理，
/// 收到完成回调后重新统计并返回最新占用。
#[cfg(windows)]
#[tauri::command]
pub fn clear_webview_cache(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<CacheInfo, String> {
    use std::sync::mpsc;
    use webview2_com::ClearBrowsingDataCompletedHandler;
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2_13, ICoreWebView2Profile2, COREWEBVIEW2_BROWSING_DATA_KINDS_DISK_CACHE,
    };
    use windows_core::Interface as _;

    let (tx, rx) = mpsc::channel::<Result<(), String>>();
    let tx_handler = tx.clone();

    // 闭包在主线程执行；完成回调经 channel 送回本命令线程等待。
    window
        .with_webview(move |platform| {
            let controller = platform.controller();
            let handler = ClearBrowsingDataCompletedHandler::create(Box::new(move |hr| {
                let _ = tx_handler.send(hr.map_err(|e| format!("ClearBrowsingData 失败: {e}")));
                Ok(())
            }));
            let dispatch = || -> Result<(), String> {
                unsafe {
                    let webview = controller.CoreWebView2().map_err(|e| e.to_string())?;
                    let profile = webview
                        .cast::<ICoreWebView2_13>()
                        .and_then(|wv| wv.Profile())
                        .map_err(|e| e.to_string())?;
                    profile
                        .cast::<ICoreWebView2Profile2>()
                        .and_then(|profile| {
                            profile.ClearBrowsingData(
                                COREWEBVIEW2_BROWSING_DATA_KINDS_DISK_CACHE,
                                &handler,
                            )
                        })
                        .map_err(|e| e.to_string())
                }
            };
            if let Err(err) = dispatch() {
                let _ = tx.send(Err(err));
            }
        })
        .map_err(|e| format!("无法访问 WebView 实例: {e}"))?;

    match rx.recv_timeout(std::time::Duration::from_secs(15)) {
        Ok(Ok(())) => get_cache_info_inner(&app),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("清除缓存超时，请重试".into()),
    }
}

// 非 Windows 平台不支持该命令；前端根据 get_cache_info 返回的 null 隐藏入口。
#[cfg(not(windows))]
#[tauri::command]
pub fn clear_webview_cache(
    app: tauri::AppHandle,
    _window: tauri::WebviewWindow,
) -> Result<CacheInfo, String> {
    let _ = app;
    Err("当前平台不支持清理网页缓存".into())
}

/// 清理应用日志：正在写入的 omnimd.log 被本进程占用无法删除，改为截断内容
/// （共享句柄允许打开写入，后续追加从文件头继续）；其余日志文件直接删除。
#[tauri::command]
pub fn clear_logs(app: tauri::AppHandle) -> Result<CacheInfo, String> {
    let dir = crate::log_dir();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if name == "omnimd.log" {
                let _ = std::fs::OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(&path);
            } else {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    get_cache_info_inner(&app)
}
