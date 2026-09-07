use serde::Serialize;
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub detail: Option<String>,
}
pub type Result<T> = std::result::Result<T, AppError>;
impl AppError {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            detail: None,
        }
    }
    pub fn io(path: &std::path::Path, e: std::io::Error) -> Self {
        let code = if e.kind() == std::io::ErrorKind::PermissionDenied {
            "FILE_PERMISSION"
        } else {
            "FILE_IO"
        };
        // Only paths and OS error kinds: never serialize HTTP bodies, JSON errors or tokens.
        Self {
            code: code.into(),
            message: format!("无法访问 {}", path.display()),
            detail: Some(format!("{:?}", e.kind())),
        }
    }
}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for AppError {}
