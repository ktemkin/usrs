//! Universal Serial Rust -- tools for working with USB from Rust.

#![cfg_attr(feature = "nightly-features", feature(cell_leak))]

pub use device::{DeviceInformation, DeviceSelector};
pub use error::{Error, UsbResult};
pub use host::{all_devices, device, devices, open, Host};

pub mod backend;
pub mod device;
pub mod error;
pub mod host;
pub mod request;
