//! Interface for working with USB devices.

use std::{rc::Rc, time::Duration};

use crate::{
    backend::{Backend, BackendDevice},
    request::{DescriptorType, RequestType, StandardDeviceRequest, STANDARD_IN_FROM_DEVICE},
    Error, UsbResult,
};

/// Contains known information for an unopened device.
#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct DeviceInformation {
    /// The Vendor ID (idVendor) assigned to the device.
    pub vendor_id: u16,

    /// The Product ID (idProduct) associated with the device.
    pub product_id: u16,

    /// The serial string associated with the device, if we were able to get one.
    pub serial: Option<String>,

    /// The vendor string associated with the device, if and only if the OS has read it.
    pub vendor: Option<String>,

    /// The product string associated with the device, if and only if the OS has read it.
    pub product: Option<String>,

    /// Numeric field for backend use; can be used to contain a hint used to re-find the device for opening.
    pub(crate) backend_numeric_location: Option<u64>,

    /// String field for backend use; can be used to contain a hint used to re-find the device for opening.
    pub(crate) backend_string_location: Option<String>,
}

impl DeviceInformation {
    /// Allows external backend implementers to create a DeviceInformation object.
    ///
    /// This should only be used if you're implementing your own backend; otherwise, you should
    /// use the DeviceInformation you get from enumeration. The internal backends *will* panic if
    /// you pass them self-constructed device information.
    ///
    /// (Of course, if you're familiar enough with our internals, you're going to ignore me,
    /// right? I'm just a docstring; what the hell do I know?)
    pub fn new(
        vendor_id: u16,
        product_id: u16,
        serial: Option<String>,
        vendor: Option<String>,
        product: Option<String>,
    ) -> DeviceInformation {
        DeviceInformation {
            vendor_id,
            product_id,
            serial,
            vendor,
            product,
            ..Default::default()
        }
    }
}

/// Information used to find a specific device.
#[derive(Debug, Default)]
pub struct DeviceSelector {
    /// If specified, searches for a device with the given VID.
    pub vendor_id: Option<u16>,

    /// If specified, searches for a device with the given PID.
    pub product_id: Option<u16>,

    /// The serial string associated with the device.
    pub serial: Option<String>,
}

impl DeviceSelector {
    pub fn matches(&self, device: &DeviceInformation) -> bool {
        // Oh, gods.
        //
        // This could be made so much tinier if we wanted to commit terrible sins.
        // We don't, so enjoy this wonderful boilerplate.

        // Check VID.
        if let Some(vid) = self.vendor_id {
            if vid != device.vendor_id {
                return false;
            }
        }

        // Check PID.
        if let Some(pid) = self.product_id {
            if pid != device.product_id {
                return false;
            }
        }

        // Check serial.
        if self.serial.is_some() {
            if self.serial != device.serial {
                return false;
            }
        }

        true
    }
}

/// Object for working with an -opened- USB device.
#[derive(Debug)]
#[allow(dead_code)]
pub struct Device {
    /// The backend associated with this device.
    backend: Rc<dyn Backend>,

    /// The per-backend inner device interface.
    backend_device: Box<dyn BackendDevice>,
}

impl Device {
    /// Attempts to release the current device from its kernel driver.
    /// Not supported on all platforms; unsupported platforms will return [Error::Unsupported].
    pub fn release_kernel_driver(&mut self, interface_number: u8) -> UsbResult<()> {
        self.backend.release_kernel_driver(interface_number)
    }

    /// Attempts to release the current device from its kernel driver, if possible.
    ///
    /// Convenience variant that returns Ok() if the current platform doesn't support the
    /// operation; allowing this to be safely used for cases where you're more interested in
    /// failures that happen later, e.g. on first real device access.
    pub fn release_kernel_driver_if_possible(&mut self, interface_number: u8) -> UsbResult<()> {
        match self.backend.release_kernel_driver(interface_number) {
            Err(Error::Unsupported) => Ok(()),
            other => other,
        }
    }

    /// Attempts to take ownership of a given interface, claiming it for exclusive access.
    pub fn claim_interface(&mut self, interface_number: u8) -> UsbResult<()> {
        self.backend.claim_interface(interface_number)
    }

    /// Performs an IN control request, with the following parameters:
    /// - [request_type] specifies the USB control request type. It's recommended this is
    /// - [request_number] is the request number. See e.g. USB 2.0 Chapter 9.
    /// - [value] and [index] are arguments to the request. For requests with a recipient
    ///   other than the device, [index] is usually the index of the target. See USB 2.0 Chapter 9.
    /// - [target] is the data to be transmitted as part of the request. It must be between [0, 65535]B.
    /// - [timeout] is how long we should wait for the request. If not provided, we'll wait
    ///   indefinitely.
    pub fn control_read(
        &mut self,
        request_type: RequestType,
        request_number: u8,
        value: u16,
        index: u16,
        target: &mut [u8],
        timeout: Option<Duration>,
    ) -> UsbResult<usize> {
        self.backend.control_read(
            self,
            request_type.into(),
            request_number,
            value,
            index,
            target,
            timeout,
        )
    }

    /// Performs an IN control request, with the parameters below.
    /// This convenience variant generates a vector, for ease of use, but may be slower than
    /// e.g. re-using an appropriately sized buffer for multiple requests.
    ///
    /// - [request_type] specifies the USB control request type. It's recommended this is
    /// - [request_number] is the request number. See e.g. USB 2.0 Chapter 9.
    /// - [value] and [index] are arguments to the request. For requests with a recipient
    ///   other than the device, [index] is usually the index of the target. See USB 2.0 Chapter 9.
    /// - [max_length] is the maximum length to be requested.
    /// - [timeout] is how long we should wait for the request. If not provided, we'll wait
    ///   indefinitely.
    ///
    /// Returns a vector of the read response.
    pub fn control_read_to_vec(
        &mut self,
        request_type: RequestType,
        request_number: u8,
        value: u16,
        index: u16,
        max_length: u16,
        timeout: Option<Duration>,
    ) -> UsbResult<Vec<u8>> {
        // Perform the request into a temporary buffer...
        let mut buffer = vec![0; max_length as usize];
        let actual_size = self.backend.control_read(
            self,
            request_type.into(),
            request_number,
            value,
            index,
            &mut buffer,
            timeout,
        )?;

        // ... clamp it down to the actual length...
        buffer.truncate(actual_size);

        // ... and return it.
        Ok(buffer)
    }

    /// Performs an OUT control request, with the following parameters:
    /// - [request_type] specifies the USB control request type, which defines several parameters
    ///   of this request.
    /// - [request_number] is the request number. See e.g. USB 2.0 Chapter 9.
    /// - [value] and [index] are arguments to the request. For requests with a recipient
    ///   other than the device, [index] is usually the index of the target. See USB 2.0 Chapter 9.
    /// - [target] is the data to be transmitted as part of the request. It must be between [0, 65535]B.
    /// - [timeout] is how long we should wait for the request. If not provided, we'll wait
    ///   indefinitely.
    pub fn control_write(
        &mut self,
        request_type: RequestType,
        request_number: u8,
        value: u16,
        index: u16,
        target: &mut [u8],
        timeout: Option<Duration>,
    ) -> UsbResult<usize> {
        self.backend.control_read(
            self,
            request_type.into(),
            request_number,
            value,
            index,
            target,
            timeout,
        )
    }

    /// Performs an unchecked IN control request.
    /// See [control_read] for argument documentation.
    ///
    /// This convenience variant allows illegal USB transfers, where the backend supports it --
    /// the vector length isn't bounds-checked, and invalid request_types are allowed.
    pub unsafe fn raw_control_read(
        &self,
        request_type: u8,
        request_number: u8,
        value: u16,
        index: u16,
        target: &mut [u8],
        timeout: Option<Duration>,
    ) -> UsbResult<usize> {
        self.backend.control_read(
            self,
            request_type,
            request_number,
            value,
            index,
            target,
            timeout,
        )
    }

    /// Performs an unchecked OUT control request.
    /// See [control_write] for argument documentation.
    ///
    /// This convenience variant allows illegal USB transfers, where the backend supports it --
    /// the vector length isn't bounds-checked, and invalid request_types are allowed.
    pub unsafe fn raw_control_write(
        &self,
        request_type: u8,
        request_number: u8,
        value: u16,
        index: u16,
        target: &mut [u8],
        timeout: Option<Duration>,
    ) -> UsbResult<()> {
        self.backend.control_write(
            self,
            request_type,
            request_number,
            value,
            index,
            target,
            timeout,
        )
    }

    /// Reads a device-level, non-string descriptor from the target device.
    ///
    /// (Technically, this can get string descriptors, too, but it'll use the Not Strictly Correct
    ///  default language ID, langID '0').
    ///
    pub fn read_descriptor(
        &mut self,
        descriptor_type: u8,
        descriptor_index: u8,
    ) -> UsbResult<Vec<u8>> {
        let value = ((descriptor_type as u16) << 8) | (descriptor_index as u16);
        self.control_read_to_vec(
            STANDARD_IN_FROM_DEVICE,
            StandardDeviceRequest::GetDescriptor.into(),
            value,
            0,
            u16::MAX,
            None,
        )
    }

    /// Reads a device-level, non-string descriptor from the target device.
    ///
    /// (Technically, this can get string descriptors, too, but it'll use the Not Strictly Correct
    ///  default language ID, langID '0').
    ///
    pub fn read_standard_descriptor(
        &mut self,
        descriptor_type: DescriptorType,
        descriptor_index: u8,
    ) -> UsbResult<Vec<u8>> {
        self.read_descriptor(descriptor_type.into(), descriptor_index)
    }

    /// Gains access to the device's per-backend data.
    ///
    /// Generically, the only reason this should be used _outside of this library_
    /// is if you are implementing your own backend!
    pub unsafe fn backend_data_mut(&mut self) -> &mut dyn BackendDevice {
        self.backend_device.as_mut()
    }

    /// Gains access to the device's per-backend data.
    ///
    /// Generically, the only reason this should be used _outside of this library_
    /// is if you are implementing your own backend!
    pub unsafe fn backend_data(&self) -> &dyn BackendDevice {
        self.backend_device.as_ref()
    }

    /// Wraps a backend device abstraction with our user-facing device.
    ///
    /// This should only be used if you're implementing a custom device backend; otherwise,
    /// you should get your Device from Host::open().
    pub fn from_backend_device(
        backend_device: Box<dyn BackendDevice>,
        backend: Rc<dyn Backend>,
    ) -> Device {
        Device {
            backend,
            backend_device,
        }
    }
}
