pub mod account;
pub mod quota;
pub mod switch;
pub mod widget;
use crate::{account::service::AccountService, error::Result};
use std::sync::Arc;
pub struct AppState(pub Result<Arc<AccountService>>);
impl AppState {
    pub fn service(&self) -> Result<&AccountService> {
        self.0.as_ref().map(|s| s.as_ref()).map_err(Clone::clone)
    }
}
