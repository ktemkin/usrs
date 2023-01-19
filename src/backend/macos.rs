//! Core, low-level functionality for macOS.

use std::{ffi::c_void, time::Duration};

use self::{
    device::{open_usb_device, MacOsDevice},
    endpoint::{address_for_in_endpoint, address_for_out_endpoint},
    iokit::{to_iokit_timeout, OsDevice, OsInterface},
    iokit_c::IOUSBDevRequest,
};

use super::{Backend, BackendDevice, DeviceInformation};
use crate::{backend::macos::iokit_c::IOUSBDevRequestTO, device::Device, error::UsbResult, Error};

mod device;
mod endpoint;
mod enumeration;
mod interface;
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
    unsafe fn device_backend_mut<'a>(&self, device: &'a mut Device) -> &'a mut MacOsDevice {
        device
            .backend_data_mut()
            .as_mut_any()
            .downcast_mut()
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
            let timeout_ms = to_iokit_timeout(timeout);

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

    // Helper that converts an endpoint address into a interface + pipeRef.
    unsafe fn resources_for_endpoint<'a>(
        &self,
        device: &'a Device,
        address: u8,
    ) -> UsbResult<(u8, &'a OsInterface)> {
        // Unpack the raw OS device from inside of our USRs device.
        let backend_device = self.device_backend(device);

        // Find the endpoint metadata for the relevant endpoint...
        let endpoint_info = backend_device
            .endpoint_metadata
            .get(&address)
            .ok_or(Error::InvalidEndpoint)?;

        // ... and get the associated interface.
        let interface = backend_device
            .interfaces
            .get(&endpoint_info.interface_number)
            .expect("endpoint points to an invalid interface");

        Ok((endpoint_info.pipe_ref, interface))
    }

    // Helper that converts an IN endpoint number into a interface + pipeRef.
    unsafe fn resources_for_in_endpoint<'a>(
        &self,
        device: &'a Device,
        number: u8,
    ) -> UsbResult<(u8, &'a OsInterface)> {
        self.resources_for_endpoint(device, address_for_in_endpoint(number))
    }

    // Helper that converts an OUT endpoint number into a interface + pipeRef.
    unsafe fn resources_for_out_endpoint<'a>(
        &self,
        device: &'a Device,
        number: u8,
    ) -> UsbResult<(u8, &'a OsInterface)> {
        self.resources_for_endpoint(device, address_for_out_endpoint(number))
    }
}

impl Backend for MacOsBackend {
    fn get_devices(&self) -> UsbResult<Vec<DeviceInformation>> {
        enumeration::enumerate_devices()
    }

    fn open(&self, information: &DeviceInformation) -> UsbResult<Box<dyn BackendDevice>> {
        open_usb_device(information)
    }

    fn release_kernel_driver(&self, _device: &mut Device, _interface: u8) -> UsbResult<()> {
        // We don't currently have a way of making macOS release kernel drivers.
        //
        // Theoretically, if the target binary is signed with the `com.apple.vm.device-access`
        // entitlement, we'd be able to do this; but this isn't something we yet support.
        Err(Error::Unsupported)
    }

    fn claim_interface(&self, device: &mut Device, interface: u8) -> UsbResult<()> {
        unsafe {
            // Unpack the raw OS device from inside of our USRs device.
            let backend_device = self.device_backend_mut(device);

            // If we don't have a handle on that interface, error out.
            let interface = backend_device
                .interfaces
                .get_mut(&interface)
                .ok_or(Error::InvalidArgument)?;

            // Otherwise, open the relevant interface, claiming it.
            interface.open()
        }
    }

    fn unclaim_interface(&self, device: &mut Device, interface: u8) -> UsbResult<()> {
        unsafe {
            // Unpack the raw OS device from inside of our USRs device.
            let backend_device = self.device_backend_mut(device);

            // If we don't have a handle on that interface, error out.
            let interface = backend_device
                .interfaces
                .get_mut(&interface)
                .ok_or(Error::InvalidArgument)?;

            // Otherwise, close the relevant interface, releasing our claim.
            interface.close();
            Ok(())
        }
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

    fn read(
        &self,
        device: &Device,
        endpoint: u8,
        buffer: &mut [u8],
        timeout: Option<Duration>,
    ) -> UsbResult<usize> {
        unsafe {
            let (pipe_ref, interface) = self.resources_for_in_endpoint(device, endpoint)?;

            if let Some(timeout) = timeout {
                interface.read_with_timeout(pipe_ref, buffer, to_iokit_timeout(timeout))
            } else {
                interface.read(pipe_ref, buffer)
            }
        }
    }

    fn write(
        &self,
        device: &Device,
        endpoint: u8,
        data: &[u8],
        timeout: Option<Duration>,
    ) -> UsbResult<()> {
        unsafe {
            let (pipe_ref, interface) = self.resources_for_out_endpoint(device, endpoint)?;

            if let Some(timeout) = timeout {
                interface.write_with_timeout(pipe_ref, data, to_iokit_timeout(timeout))
            } else {
                interface.write(pipe_ref, data)
            }
        }
    }
}
