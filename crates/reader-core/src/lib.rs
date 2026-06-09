#![recursion_limit = "256"]

pub mod app_state;
pub mod crawler;
pub mod dto;
pub mod error;
pub mod facade;
pub mod model;
pub mod parser;
pub mod service;
pub mod source_runtime;
pub mod storage;
pub mod util;

pub use app_state::{ReaderCoreOptions, SecureMode};
pub use dto::*;
pub use error::{CommandError, ReaderCoreError};
pub use facade::ReaderCore;
