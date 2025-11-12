#[cfg(target_os = "windows")]
extern crate winapi;

#[cfg(target_os = "windows")]
pub mod dxgi;

pub mod common;

pub use common::*;
