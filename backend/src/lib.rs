//! Tauri Vue3 App Library
//!
//! A modern desktop application built with Tauri v2 and Vue 3.

mod error;
mod logging;

pub use error::AppError;
pub use logging::{init_logging, send_log, send_log_with_handle, LogLevel, ResultExt};
