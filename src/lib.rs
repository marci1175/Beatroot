// #![feature(portable_simd)]

pub const APP_NAME: &str = "Beatroot";
pub const IS_DEBUG: bool = cfg!(debug_assertions);
pub const VALID_TIME_SIG_DENOMINATORS: &[u8] = &[1, 2, 4, 8, 16, 32, 64];

pub mod app;
pub mod audio;
pub mod internals;
pub mod plugins;
pub mod project_manager;
pub mod ui;
