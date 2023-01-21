//! Core, low-level functionality for macOS.

use std::{
    cell::RefCell,
    ffi::c_void,
    sync::Arc,
    time::{Duration, SystemTime},
};

use self::{
    callback::{delegate_iousb_callback, CallbackRefconType},
    device::{open_usb_device, MacOsDevice},
    endpoint::{address_for_in_endpoint, address_for_out_endpoint},
    iokit::{leak_to_iokit, to_iokit_timeout, OsDevice, OsInterface},
    iokit_c::IOUSBDevRequest,
};

use super::{Backend, BackendDevice, DeviceInformation};
use crate::{backend::macos::iokit_c::IOUSBDevRequestTO, device::Device, error::UsbResult, Error};

mod callback;
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

    /// Helper for issuing async control requests.
    unsafe fn control_nonblocking(
        &self,
        device: &Device,
        request_type: u8,
        request_number: u8,
        value: u16,
        index: u16,
        data: *mut c_void,
        length: u16,
        callback: Box<CallbackRefconType>,
        timeout: Option<Duration>,
    ) -> UsbResult<()> {
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
            device.device_request_nonblocking_with_timeout(
                &mut request_struct,
                delegate_iousb_callback,
                leak_to_iokit(callback),
            )?;
            Ok(())
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
            device.device_request_nonblocking(
                &mut request_struct,
                delegate_iousb_callback,
                leak_to_iokit(callback),
            )?;
            Ok(())
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

    fn active_configuration(&self, device: &Device) -> UsbResult<u8> {
        unsafe {
            let backend_device = self.os_device_for(device);
            backend_device.get_configuration()
        }
    }

    fn set_active_configuration(&self, device: &Device, configuration_index: u8) -> UsbResult<()> {
        unsafe {
            let backend_device = self.os_device_for(device);
            backend_device.set_configuration(configuration_index)
        }
    }

    fn reset_device(&self, device: &Device) -> UsbResult<()> {
        unsafe {
            let backend_device = self.os_device_for(device);
            backend_device.reset()
        }
    }

    fn clear_stall(&self, device: &Device, endpoint_address: u8) -> UsbResult<()> {
        unsafe {
            let (pipe_ref, interface) = self.resources_for_endpoint(device, endpoint_address)?;
            interface.clear_stall(pipe_ref)
        }
    }

    fn set_alternate_setting(&self, device: &Device, interface: u8, setting: u8) -> UsbResult<()> {
        unsafe {
            let backend_data = self.device_backend(device);
            let interface = backend_data
                .interfaces
                .get(&interface)
                .ok_or(Error::InvalidInterface)?;

            interface.set_alternate_setting(setting)
        }
    }

    fn current_bus_frame(&self, _device: &Device) -> UsbResult<(u64, SystemTime)> {
        // In theory, this should be easy. We call get_frame_number, which gives us
        // the u64 frame number and the AbsoluteTime. In practice, I currently have no
        // idea _which_ macOS absolute time that its, and I'm worried it's a mach absolute time,
        // which is in terms of _number of scheduler ticks_. Once we figure out how to convert
        // an IOKit AbsoluteTime to a meaningful time, we can do the math here to return this.
        Err(Error::Unsupported)
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

    fn control_read_nonblocking(
        &self,
        device: &Device,
        request_type: u8,
        request_number: u8,
        value: u16,
        index: u16,
        target: Arc<RefCell<dyn AsMut<[u8]>>>,
        callback: Box<CallbackRefconType>,
        timeout: Option<Duration>,
    ) -> UsbResult<()> {
        unsafe {
            // Extract the data we were passed from the user, so we can pass it to IOKit.
            let mut data_dyn = (*target).borrow_mut();
            let data = data_dyn.as_mut();

            // If the data is too long for a control request, error out.
            if data.len() > (u16::MAX as usize) {
                return Err(Error::Overrun);
            }

            self.control_nonblocking(
                device,
                request_type,
                request_number,
                value,
                index,
                data.as_ptr() as *mut c_void,
                data.len() as u16,
                callback,
                timeout,
            )?;
            Ok(())
        }
    }

    fn control_write_nonblocking(
        &self,
        device: &Device,
        request_type: u8,
        request_number: u8,
        value: u16,
        index: u16,
        data: Arc<dyn AsRef<[u8]>>,
        callback: Box<CallbackRefconType>,
        timeout: Option<Duration>,
    ) -> UsbResult<()> {
        unsafe {
            let data = (*data).as_ref();

            // If the data is too long for a control request, error out.
            if data.len() > (u16::MAX as usize) {
                return Err(Error::Overrun);
            }

            self.control_nonblocking(
                device,
                request_type,
                request_number,
                value,
                index,
                data.as_ptr() as *mut c_void,
                data.len() as u16,
                callback,
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

    fn read_nonblocking(
        &self,
        device: &Device,
        endpoint: u8,
        buffer: Arc<RefCell<dyn AsMut<[u8]>>>,
        callback: Box<dyn FnOnce(UsbResult<usize>)>,
        timeout: Option<Duration>,
    ) -> UsbResult<()> {
        unsafe {
            let (pipe_ref, interface) = self.resources_for_in_endpoint(device, endpoint)?;

            // Extract the data we were passed from the user, so we can pass it to IOKit.
            let mut data_dyn = (*buffer).borrow_mut();
            let data = data_dyn.as_mut();

            if let Some(timeout) = timeout {
                interface.read_with_timeout_nonblocking(
                    pipe_ref,
                    data.as_mut_ptr() as *mut c_void,
                    data.len() as u32,
                    delegate_iousb_callback,
                    leak_to_iokit(callback),
                    to_iokit_timeout(timeout),
                )
            } else {
                interface.read_nonblocking(
                    pipe_ref,
                    data.as_mut_ptr() as *mut c_void,
                    data.len() as u32,
                    delegate_iousb_callback,
                    leak_to_iokit(callback),
                )
            }
        }
    }

    fn write_nonblocking(
        &self,
        device: &Device,
        endpoint: u8,
        data: Arc<dyn AsRef<[u8]>>,
        callback: Box<dyn FnOnce(UsbResult<usize>)>,
        timeout: Option<Duration>,
    ) -> UsbResult<()> {
        unsafe {
            let (pipe_ref, interface) = self.resources_for_out_endpoint(device, endpoint)?;

            // Extract the data we were passed from the user, so we can pass it to IOKit.
            let data = (*data).as_ref();

            if let Some(timeout) = timeout {
                interface.write_with_timeout_nonblocking(
                    pipe_ref,
                    data.as_ptr() as *mut c_void,
                    data.len() as u32,
                    delegate_iousb_callback,
                    leak_to_iokit(callback),
                    to_iokit_timeout(timeout),
                )
            } else {
                interface.write_nonblocking(
                    pipe_ref,
                    data.as_ptr() as *mut c_void,
                    data.len() as u32,
                    delegate_iousb_callback,
                    leak_to_iokit(callback),
                )
            }
        }
    }
}
