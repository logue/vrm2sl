//! Tauri Vue3 App Library
//!
//! A modern desktop application built with Tauri v2 and Vue 3.

pub mod convert;
pub mod correction;
mod error;
pub mod ipc;
mod logging;
pub mod notify;
pub mod pipeline;
pub mod project;
pub mod texture;

pub use error::AppError;
pub use logging::{LogLevel, ResultExt, init_logging, send_log, send_log_with_handle};
