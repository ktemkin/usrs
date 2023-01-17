//! Core, low-level functionality for macOS.

use super::{Backend, DeviceInformation};
use crate::error::UsbResult;

mod enumeration;
mod iokit;

/// Per-OS data for the MacOS backend.
pub struct MacOsBackend {}

impl MacOsBackend {
    pub fn new() -> UsbResult<MacOsBackend> {
        return Ok(MacOsBackend {});
    }
}

impl Backend for MacOsBackend {
    fn get_devices(&self) -> UsbResult<Vec<DeviceInformation>> {
        enumeration::enumerate_devices()
    }
}
