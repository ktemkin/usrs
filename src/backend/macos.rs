//! Core, low-level functionality for macOS.

use std::{ffi::c_void, time::Duration};

use log::warn;

use self::{
    device::{open_usb_device, MacOsDevice},
    iokit::OsDevice,
    iokit_c::IOUSBDevRequest,
};

use super::{Backend, BackendDevice, DeviceInformation};
use crate::{backend::macos::iokit_c::IOUSBDevRequestTO, device::Device, error::UsbResult, Error};

mod device;
mod enumeration;
mod iokit;
mod iokit_c;

/// Per-OS data for the MacOS backend.
#[derive(Debug)]
pub struct MacOsBackend {}

impl MacOsBackend {
    pub fn new() -> UsbResult<MacOsBackend> {
        Ok(MacOsBackend {})
    }

    /// Helper that fetches the MacOsBackend for the relevant device.
    unsafe fn device_backend<'a>(&self, device: &'a Device) -> &'a MacOsDevice {
        device
            .backend_data()
            .as_any()
            .downcast_ref()
            .expect("internal consistency: tried to open a type from another backend?")
    }

    /// Helper that fetches the MacOsBackend for the relevant device.
    unsafe fn os_device_for<'a>(&self, device: &'a Device) -> &'a OsDevice {
        &self.device_backend(device).device
    }

    /// Helper for issuing control requests.
    unsafe fn control(
        &self,
        device: &Device,
        request_type: u8,
        request_number: u8,
        value: u16,
        index: u16,
        data: *mut c_void,
        length: u16,
        timeout: Option<Duration>,
    ) -> UsbResult<usize> {
        // Unpack the raw OS device from inside of our USRs device.
        let device = self.os_device_for(device);

        // If we have a timeout, use the *TO request function.
        if let Some(timeout) = timeout {
            let mut timeout_ms = timeout.as_millis() as u32;

            // Truncate this to a u32, since more would be a heckuva long time anyway.
            if timeout.as_millis() > (u32::MAX as u128) {
                warn!(
                    "A wildly long timeout ({}s) was truncated to u32::MAX ({}s).",
                    timeout.as_secs_f64(),
                    Duration::from_millis(u32::MAX as u64).as_secs_f64()
                );
                timeout_ms = u32::MAX;
            }

            // Populate the request-with-TimeOut structure, which will be passed to macOS.
            let mut request_struct = IOUSBDevRequestTO {
                bmRequestType: request_type,
                bRequest: request_number,
                wValue: value,
                wIndex: index,
                wLength: length,
                pData: data,
                wLenDone: 0,
                noDataTimeout: timeout_ms,
                completionTimeout: timeout_ms,
            };

            // And finally, perform the request.
            device.device_request_with_timeout(&mut request_struct)?;
            Ok(request_struct.wLenDone as usize)
        } else {
            // Populate the (no timeout) request structure, which will be passed to macOS.
            let mut request_struct = IOUSBDevRequest {
                bmRequestType: request_type,
                bRequest: request_number,
                wValue: value,
                wIndex: index,
                wLength: length,
                pData: data,
                wLenDone: 0,
            };

            // And finally, perform the request.
            device.device_request(&mut request_struct)?;
            Ok(request_struct.wLenDone as usize)
        }
    }
}

impl Backend for MacOsBackend {
    fn get_devices(&self) -> UsbResult<Vec<DeviceInformation>> {
        enumeration::enumerate_devices()
    }

    fn open(&self, information: &DeviceInformation) -> UsbResult<Box<dyn BackendDevice>> {
        open_usb_device(information)
    }

    fn release_kernel_driver(&self, _interface: u8) -> UsbResult<()> {
        // We don't currently have a way of making macOS release interfaces.
        Err(Error::Unsupported)
    }

    fn claim_interface(&self, _interface: u8) -> UsbResult<()> {
        // This is handled automatically by macOS; claiming interfaces is done when they're opened.
        // In the future we may way to handle this more explicitly; but for now this will mostly
        // not be exposed to the user, anyway.
        Ok(())
    }

    fn control_read(
        &self,
        device: &Device,
        request_type: u8,
        request_number: u8,
        value: u16,
        index: u16,
        target: &mut [u8],
        timeout: Option<Duration>,
    ) -> UsbResult<usize> {
        if target.len() > (u16::MAX as usize) {
            return Err(Error::Overrun);
        }

        unsafe {
            self.control(
                device,
                request_type,
                request_number,
                value,
                index,
                target.as_mut_ptr() as *mut c_void,
                target.len() as u16,
                timeout,
            )
        }
    }

    fn control_write(
        &self,
        device: &Device,
        request_type: u8,
        request_number: u8,
        value: u16,
        index: u16,
        data: &[u8],
        timeout: Option<Duration>,
    ) -> UsbResult<()> {
        if data.len() > (u16::MAX as usize) {
            return Err(Error::Overrun);
        }

        unsafe {
            self.control(
                device,
                request_type,
                request_number,
                value,
                index,
                data.as_ptr() as *mut c_void,
                data.len() as u16,
                timeout,
            )?;
            Ok(())
        }
    }
}
