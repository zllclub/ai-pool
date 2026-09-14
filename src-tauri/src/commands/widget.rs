use crate::{
    error::{AppError, Result},
    storage::atomic,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};
use tauri::{
    Emitter, Manager, PhysicalPosition, Position, Size, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

const COLLAPSED_WIDTH: f64 = 180.0;
const COLLAPSED_HEIGHT: f64 = 36.0;
const EXPANDED_WIDTH: f64 = 240.0;
const EXPANDED_HEIGHT: f64 = 108.0;
const EDGE_GAP: f64 = 8.0;
const POSITIONS_FILE: &str = "widget-positions.v1.json";
static POSITION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedPosition {
    monitor_name: Option<String>,
    monitor_x: i32,
    monitor_y: i32,
    monitor_width: u32,
    monitor_height: u32,
    x: f64,
    y: f64,
    right: bool,
}

#[tauri::command]
pub fn show_account_widget(app: tauri::AppHandle, account_id: String) -> Result<bool> {
    if account_id.trim().is_empty() {
        return Err(AppError::new("WIDGET_ACCOUNT", "缺少悬浮账号"));
    }
    let digest = Sha256::digest(account_id.as_bytes());
    let label = format!("account-widget-{}", hex_prefix(&digest));
    if let Some(window) = app.get_webview_window(&label) {
        save_position(&app, &window, &account_id)?;
        window
            .close()
            .map_err(|e| widget_error("WIDGET_CLOSE", "无法关闭悬浮窗", e))?;
        return Ok(false);
    }

    let index = app
        .webview_windows()
        .keys()
        .filter(|name| name.starts_with("account-widget-"))
        .count();
    let target = initial_position(&app, &account_id, index);
    let encoded: String = url::form_urlencoded::byte_serialize(account_id.as_bytes()).collect();
    let page = format!("index.html?widget=1&account={encoded}");
    let window = WebviewWindowBuilder::new(&app, label, WebviewUrl::App(page.into()))
        .title("AI Pool 额度")
        .inner_size(COLLAPSED_WIDTH, COLLAPSED_HEIGHT)
        .position(20.0, 20.0)
        .resizable(false)
        .decorations(false)
        .accept_first_mouse(true)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(false)
        .build()
        .map_err(|e| widget_error("WIDGET_CREATE", "无法创建悬浮窗", e))?;
    if let Some(position) = target {
        window
            .set_position(Position::Physical(position))
            .map_err(|e| widget_error("WIDGET_POSITION", "无法恢复悬浮窗位置", e))?;
    }
    Ok(true)
}

#[tauri::command]
pub fn close_account_widget(
    app: tauri::AppHandle,
    window: WebviewWindow,
    account_id: String,
) -> Result<()> {
    save_position(window.app_handle(), &window, &account_id)?;
    window
        .close()
        .map_err(|e| widget_error("WIDGET_CLOSE", "无法关闭悬浮窗", e))?;
    app.emit_to("main", "widget-closed", account_id)
        .map_err(|e| widget_error("WIDGET_EVENT", "无法同步悬浮状态", e))?;
    Ok(())
}

#[tauri::command]
pub fn set_widget_expanded(
    window: WebviewWindow,
    account_id: String,
    expanded: bool,
) -> Result<()> {
    let (width, height) = if expanded {
        (EXPANDED_WIDTH, EXPANDED_HEIGHT)
    } else {
        (COLLAPSED_WIDTH, COLLAPSED_HEIGHT)
    };
    let monitor = window
        .current_monitor()
        .map_err(|e| widget_error("WIDGET_MONITOR", "无法读取屏幕", e))?;
    let old_position = window
        .outer_position()
        .map_err(|e| widget_error("WIDGET_POSITION", "无法读取悬浮窗位置", e))?;
    let old_size = window
        .outer_size()
        .map_err(|e| widget_error("WIDGET_SIZE", "无法读取悬浮窗尺寸", e))?;
    window
        .set_size(Size::Logical(tauri::LogicalSize::new(width, height)))
        .map_err(|e| widget_error("WIDGET_SIZE", "无法调整悬浮窗", e))?;
    if let Some(monitor) = monitor {
        let scale = monitor.scale_factor();
        let origin = monitor.position();
        let screen = monitor.size();
        let screen_right = origin.x + screen.width as i32;
        let old_center = old_position.x + old_size.width as i32 / 2;
        let right_side = old_center > origin.x + screen.width as i32 / 2;
        let x = if right_side {
            screen_right - (width * scale) as i32 - (EDGE_GAP * scale) as i32
        } else {
            origin.x + (EDGE_GAP * scale) as i32
        };
        let max_y =
            origin.y + screen.height as i32 - (height * scale) as i32 - (EDGE_GAP * scale) as i32;
        let y = old_position
            .y
            .clamp(origin.y + (EDGE_GAP * scale) as i32, max_y);
        window
            .set_position(Position::Physical(PhysicalPosition::new(x, y)))
            .map_err(|e| widget_error("WIDGET_POSITION", "无法调整悬浮窗位置", e))?;
    }
    save_position(window.app_handle(), &window, &account_id)
}

#[tauri::command]
pub fn snap_account_widget(
    window: WebviewWindow,
    account_id: String,
    side: Option<String>,
) -> Result<()> {
    let Some(monitor) = window
        .current_monitor()
        .map_err(|e| widget_error("WIDGET_MONITOR", "无法读取屏幕", e))?
    else {
        return Ok(());
    };
    let position = window
        .outer_position()
        .map_err(|e| widget_error("WIDGET_POSITION", "无法读取悬浮窗位置", e))?;
    let size = window
        .outer_size()
        .map_err(|e| widget_error("WIDGET_SIZE", "无法读取悬浮窗尺寸", e))?;
    let origin = monitor.position();
    let screen = monitor.size();
    let gap = (EDGE_GAP * monitor.scale_factor()) as i32;
    let use_right = match side.as_deref() {
        Some("left") => false,
        Some("right") => true,
        _ => position.x + size.width as i32 / 2 > origin.x + screen.width as i32 / 2,
    };
    let x = if use_right {
        origin.x + screen.width as i32 - size.width as i32 - gap
    } else {
        origin.x + gap
    };
    let max_y = origin.y + screen.height as i32 - size.height as i32 - gap;
    let y = position.y.clamp(origin.y + gap, max_y);
    window
        .set_position(Position::Physical(PhysicalPosition::new(x, y)))
        .map_err(|e| widget_error("WIDGET_POSITION", "无法吸附悬浮窗位置", e))?;
    save_position(window.app_handle(), &window, &account_id)
}

fn positions_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf> {
    app.path()
        .app_data_dir()
        .map(|path| path.join(POSITIONS_FILE))
        .map_err(|_| AppError::new("APP_DATA", "找不到应用数据目录"))
}

fn load_positions(app: &tauri::AppHandle) -> Result<HashMap<String, SavedPosition>> {
    let path = positions_path(app)?;
    match atomic::read_optional(&path)? {
        Some(bytes) => serde_json::from_slice(&bytes)
            .map_err(|_| AppError::new("WIDGET_POSITIONS_CORRUPT", "悬浮窗位置记录损坏")),
        None => Ok(HashMap::new()),
    }
}

fn save_position(app: &tauri::AppHandle, window: &WebviewWindow, account_id: &str) -> Result<()> {
    let Some(monitor) = window
        .current_monitor()
        .map_err(|e| widget_error("WIDGET_MONITOR", "无法读取屏幕", e))?
    else {
        return Ok(());
    };
    let position = window
        .outer_position()
        .map_err(|e| widget_error("WIDGET_POSITION", "无法读取悬浮窗位置", e))?;
    let size = window
        .outer_size()
        .map_err(|e| widget_error("WIDGET_SIZE", "无法读取悬浮窗尺寸", e))?;
    let origin = monitor.position();
    let screen = monitor.size();
    let scale = monitor.scale_factor();
    let saved = SavedPosition {
        monitor_name: monitor.name().cloned(),
        monitor_x: origin.x,
        monitor_y: origin.y,
        monitor_width: screen.width,
        monitor_height: screen.height,
        x: (position.x - origin.x) as f64 / scale,
        y: (position.y - origin.y) as f64 / scale,
        right: position.x + size.width as i32 / 2 > origin.x + screen.width as i32 / 2,
    };
    let _guard = POSITION_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| AppError::new("WIDGET_POSITION_LOCK", "无法保存悬浮窗位置"))?;
    let mut all = load_positions(app).unwrap_or_default();
    all.insert(account_id.to_owned(), saved);
    let bytes = serde_json::to_vec_pretty(&all)
        .map_err(|_| AppError::new("SERIALIZE", "悬浮窗位置序列化失败"))?;
    atomic::write(&positions_path(app)?, &bytes)
}

fn initial_position(
    app: &tauri::AppHandle,
    account_id: &str,
    index: usize,
) -> Option<PhysicalPosition<i32>> {
    let monitors = app.available_monitors().ok()?;
    if let Ok(all) = load_positions(app) {
        if let Some(saved) = all.get(account_id) {
            let monitor = monitors
                .iter()
                .find(|monitor| {
                    saved.monitor_name.is_some() && monitor.name() == saved.monitor_name.as_ref()
                })
                .or_else(|| {
                    monitors.iter().find(|monitor| {
                        monitor.position().x == saved.monitor_x
                            && monitor.position().y == saved.monitor_y
                            && monitor.size().width == saved.monitor_width
                            && monitor.size().height == saved.monitor_height
                    })
                });
            if let Some(monitor) = monitor {
                let scale = monitor.scale_factor();
                let origin = monitor.position();
                let screen = monitor.size();
                let gap = (EDGE_GAP * scale) as i32;
                let width = (COLLAPSED_WIDTH * scale) as i32;
                let height = (COLLAPSED_HEIGHT * scale) as i32;
                let x = if saved.right {
                    origin.x + screen.width as i32 - width - gap
                } else {
                    (origin.x + (saved.x * scale) as i32)
                        .clamp(origin.x + gap, origin.x + screen.width as i32 - width - gap)
                };
                let y = (origin.y + (saved.y * scale) as i32).clamp(
                    origin.y + gap,
                    origin.y + screen.height as i32 - height - gap,
                );
                return Some(PhysicalPosition::new(x, y));
            }
        }
    }

    let monitor = app
        .get_webview_window("main")
        .and_then(|window| window.current_monitor().ok().flatten())
        .or_else(|| monitors.first().cloned())?;
    let scale = monitor.scale_factor();
    let origin = monitor.position();
    let screen = monitor.size();
    let width = (COLLAPSED_WIDTH * scale) as i32;
    let height = (COLLAPSED_HEIGHT * scale) as i32;
    let gap = (EDGE_GAP * scale) as i32;
    let slots = ((screen.height as i32 - gap * 2) / (height + gap)).max(1) as usize;
    let row = index % slots;
    let column = index / slots;
    Some(PhysicalPosition::new(
        origin.x + screen.width as i32 - width - gap - column as i32 * (width + gap),
        origin.y + gap + row as i32 * (height + gap),
    ))
}

fn hex_prefix(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn widget_error(code: &str, message: &str, error: impl std::fmt::Display) -> AppError {
    AppError::new(code, &format!("{message}: {error}"))
}
