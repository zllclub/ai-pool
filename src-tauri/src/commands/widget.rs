use crate::error::{AppError, Result};
use sha2::{Digest, Sha256};
use tauri::{Emitter, Manager, PhysicalPosition, Position, Size, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

const COLLAPSED_WIDTH: f64 = 180.0;
const COLLAPSED_HEIGHT: f64 = 36.0;
const EXPANDED_WIDTH: f64 = 240.0;
const EXPANDED_HEIGHT: f64 = 108.0;
const EDGE_GAP: f64 = 8.0;

#[tauri::command]
pub fn show_account_widget(app: tauri::AppHandle, account_id: String) -> Result<bool> {
    if account_id.trim().is_empty() {
        return Err(AppError::new("WIDGET_ACCOUNT", "缺少悬浮账号"));
    }
    let digest = Sha256::digest(account_id.as_bytes());
    let label = format!("account-widget-{}", hex_prefix(&digest));
    if let Some(window) = app.get_webview_window(&label) {
        window.close().map_err(|e| widget_error("WIDGET_CLOSE", "无法关闭悬浮窗", e))?;
        return Ok(false);
    }

    let index = app.webview_windows().keys().filter(|name| name.starts_with("account-widget-")).count();
    let (x, y) = initial_edge_position(&app, index);
    let encoded: String = url::form_urlencoded::byte_serialize(account_id.as_bytes()).collect();
    let page = format!("index.html?widget=1&account={encoded}");
    WebviewWindowBuilder::new(&app, label, WebviewUrl::App(page.into()))
        .title("AI Pool 额度")
        .inner_size(COLLAPSED_WIDTH, COLLAPSED_HEIGHT)
        .position(x, y)
        .resizable(false)
        .decorations(false)
        .accept_first_mouse(true)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(false)
        .build()
        .map_err(|e| widget_error("WIDGET_CREATE", "无法创建悬浮窗", e))?;
    Ok(true)
}

#[tauri::command]
pub fn close_account_widget(app: tauri::AppHandle, window: WebviewWindow, account_id: String) -> Result<()> {
    window.close().map_err(|e| widget_error("WIDGET_CLOSE", "无法关闭悬浮窗", e))?;
    app.emit_to("main", "widget-closed", account_id)
        .map_err(|e| widget_error("WIDGET_EVENT", "无法同步悬浮状态", e))?;
    Ok(())
}

#[tauri::command]
pub fn set_widget_expanded(window: WebviewWindow, expanded: bool) -> Result<()> {
    let (width, height) = if expanded { (EXPANDED_WIDTH, EXPANDED_HEIGHT) } else { (COLLAPSED_WIDTH, COLLAPSED_HEIGHT) };
    let monitor = window.current_monitor().map_err(|e| widget_error("WIDGET_MONITOR", "无法读取屏幕", e))?;
    let old_position = window.outer_position().map_err(|e| widget_error("WIDGET_POSITION", "无法读取悬浮窗位置", e))?;
    let old_size = window.outer_size().map_err(|e| widget_error("WIDGET_SIZE", "无法读取悬浮窗尺寸", e))?;
    window.set_size(Size::Logical(tauri::LogicalSize::new(width, height)))
        .map_err(|e| widget_error("WIDGET_SIZE", "无法调整悬浮窗", e))?;
    if let Some(monitor) = monitor {
        let scale = monitor.scale_factor();
        let origin = monitor.position();
        let screen = monitor.size();
        let screen_right = origin.x + screen.width as i32;
        let old_center = old_position.x + old_size.width as i32 / 2;
        let right_side = old_center > origin.x + screen.width as i32 / 2;
        let x = if right_side { screen_right - (width * scale) as i32 - (EDGE_GAP * scale) as i32 } else { origin.x + (EDGE_GAP * scale) as i32 };
        let max_y = origin.y + screen.height as i32 - (height * scale) as i32 - (EDGE_GAP * scale) as i32;
        let y = old_position.y.clamp(origin.y + (EDGE_GAP * scale) as i32, max_y);
        window.set_position(Position::Physical(PhysicalPosition::new(x, y)))
            .map_err(|e| widget_error("WIDGET_POSITION", "无法调整悬浮窗位置", e))?;
    }
    Ok(())
}

#[tauri::command]
pub fn snap_account_widget(window: WebviewWindow, side: Option<String>) -> Result<()> {
    let Some(monitor) = window.current_monitor().map_err(|e| widget_error("WIDGET_MONITOR", "无法读取屏幕", e))? else { return Ok(()); };
    let position = window.outer_position().map_err(|e| widget_error("WIDGET_POSITION", "无法读取悬浮窗位置", e))?;
    let size = window.outer_size().map_err(|e| widget_error("WIDGET_SIZE", "无法读取悬浮窗尺寸", e))?;
    let origin = monitor.position();
    let screen = monitor.size();
    let gap = (EDGE_GAP * monitor.scale_factor()) as i32;
    let use_right = match side.as_deref() {
        Some("left") => false,
        Some("right") => true,
        _ => position.x + size.width as i32 / 2 > origin.x + screen.width as i32 / 2,
    };
    let x = if use_right { origin.x + screen.width as i32 - size.width as i32 - gap } else { origin.x + gap };
    let max_y = origin.y + screen.height as i32 - size.height as i32 - gap;
    let y = position.y.clamp(origin.y + gap, max_y);
    window.set_position(Position::Physical(PhysicalPosition::new(x, y)))
        .map_err(|e| widget_error("WIDGET_POSITION", "无法吸附悬浮窗", e))?;
    Ok(())
}

fn hex_prefix(bytes: &[u8]) -> String {
    bytes.iter().take(8).map(|byte| format!("{byte:02x}")).collect()
}

fn initial_edge_position(app: &tauri::AppHandle, index: usize) -> (f64, f64) {
    let monitor = app.get_webview_window("main").and_then(|window| window.current_monitor().ok().flatten());
    let Some(monitor) = monitor else { return (20.0, 20.0); };
    let scale = monitor.scale_factor();
    let origin = monitor.position();
    let size = monitor.size();
    let left = origin.x as f64 / scale;
    let top = origin.y as f64 / scale;
    let width = size.width as f64 / scale;
    let height = size.height as f64 / scale;
    let slots = ((height - 40.0) / (COLLAPSED_HEIGHT + EDGE_GAP)).floor().max(1.0) as usize;
    let row = index % slots;
    let column = index / slots;
    let x = left + width - COLLAPSED_WIDTH - EDGE_GAP - column as f64 * (COLLAPSED_WIDTH + EDGE_GAP);
    let y = top + 20.0 + row as f64 * (COLLAPSED_HEIGHT + EDGE_GAP);
    (x, y)
}

fn widget_error(code: &str, message: &str, error: impl std::fmt::Display) -> AppError {
    AppError::new(code, &format!("{message}: {error}"))
}
