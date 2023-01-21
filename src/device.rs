//! Interface for working with USB devices.

use std::{rc::Rc, time::Duration};

use crate::{
    backend::{Backend, BackendDevice},
    request::{DescriptorType, RequestType, StandardDeviceRequest, STANDARD_IN_FROM_DEVICE},
    AsyncCallback, Error, ReadBuffer, UsbResult, WriteBuffer,
};

#[cfg(feature = "async")]
use crate::futures::UsbFuture;

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
        let backend = Rc::clone(&self.backend);
        backend.release_kernel_driver(self, interface_number)
    }

    /// Attempts to release the current device from its kernel driver, if possible.
    ///
    /// Convenience variant that returns Ok() if the current platform doesn't support the
    /// operation; allowing this to be safely used for cases where you're more interested in
    /// failures that happen later, e.g. on first real device access.
    pub fn release_kernel_driver_if_possible(&mut self, interface_number: u8) -> UsbResult<()> {
        let backend = Rc::clone(&self.backend);

        match backend.release_kernel_driver(self, interface_number) {
            Err(Error::Unsupported) => Ok(()),
            other => other,
        }
    }

    /// Attempts to take ownership of a given interface, claiming it for exclusive access.
    pub fn claim_interface(&mut self, interface_number: u8) -> UsbResult<()> {
        let backend = Rc::clone(&self.backend);
        backend.claim_interface(self, interface_number)
    }

    /// Releases ownership of a given interface, allowing it to be claimed by others.
    pub fn unclaim_interface(&mut self, interface_number: u8) -> UsbResult<()> {
        let backend = Rc::clone(&self.backend);
        backend.unclaim_interface(self, interface_number)
    }

    /// Performs an IN control request, with the following parameters:
    /// - [request_type] specifies the USB control request type. It's recommended this is
    /// - [request_number] is the request number. See e.g. USB 2.0 Chapter 9.
    /// - [value] and [index] are arguments to the request. For requests with a recipient
    ///   other than the device, [index] is usually the index of the target. See USB 2.0 Chapter 9.
    /// - [target] is the data to be transmitted as part of the request. It must be between [0, 65535]B.
    /// - [timeout] is how long we should wait for the request. If not provided, we'll wait
    ///   indefinitely.
    ///
    /// Returns the actual length read.
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

    /// Performs an asynchronous IN control request, with the following parameters:
    /// - [request_type] specifies the USB control request type. It's recommended this is
    /// - [request_number] is the request number. See e.g. USB 2.0 Chapter 9.
    /// - [value] and [index] are arguments to the request. For requests with a recipient
    ///   other than the device, [index] is usually the index of the target. See USB 2.0 Chapter 9.
    /// - [target] is the data to be transmitted as part of the request. It must be between [0, 65535]B.
    /// - [timeout] is how long we should wait for the request. If not provided, we'll wait
    ///   indefinitely.
    ///
    /// The provided callback is called once the operation completes, and receives the actual
    /// length read (or status, on failure).
    #[cfg(feature = "callbacks")]
    pub fn control_read_and_call_back(
        &mut self,
        request_type: RequestType,
        request_number: u8,
        value: u16,
        index: u16,
        target: ReadBuffer,
        callback: AsyncCallback,
        timeout: Option<Duration>,
    ) -> UsbResult<()> {
        self.backend.control_read_nonblocking(
            self,
            request_type.into(),
            request_number,
            value,
            index,
            target,
            callback,
            timeout,
        )
    }

    /// Performs an asynchronous IN control request, with the following parameters:
    /// - [request_type] specifies the USB control request type. It's recommended this is
    /// - [request_number] is the request number. See e.g. USB 2.0 Chapter 9.
    /// - [value] and [index] are arguments to the request. For requests with a recipient
    ///   other than the device, [index] is usually the index of the target. See USB 2.0 Chapter 9.
    /// - [target] is the data to be transmitted as part of the request. It must be between [0, 65535]B.
    /// - [timeout] is how long we should wait for the request. If not provided, we'll wait
    ///   indefinitely.
    ///
    /// Like a typical async function, this method returns a future. However, since _submission_
    /// can fail before the asynchronous component, the future is wrapped in a UsbResult.
    #[cfg(feature = "async")]
    pub fn control_read_async(
        &mut self,
        request_type: RequestType,
        request_number: u8,
        value: u16,
        index: u16,
        target: ReadBuffer,
        timeout: Option<Duration>,
    ) -> UsbResult<UsbFuture> {
        // Create the future, and get a copy of it for our inner callback API,
        // because everyone needs to get themselves a copy.
        let future = UsbFuture::new();
        let shared_state = future.clone_state();

        // Convert our inner callback-API into an async API by having our callback just... complete the future.
        let callback = Box::new(move |result| shared_state.lock().unwrap().complete(result));

        // Finally, trigger the actual async control read.
        self.backend.control_read_nonblocking(
            self,
            request_type.into(),
            request_number,
            value,
            index,
            target,
            callback,
            timeout,
        )?;

        Ok(future)
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
        data: &[u8],
        timeout: Option<Duration>,
    ) -> UsbResult<()> {
        self.backend.control_write(
            self,
            request_type.into(),
            request_number,
            value,
            index,
            data,
            timeout,
        )
    }

    /// Performs an asynchronous OUT control request, with the following parameters:
    /// - [request_type] specifies the USB control request type. It's recommended this is
    /// - [request_number] is the request number. See e.g. USB 2.0 Chapter 9.
    /// - [value] and [index] are arguments to the request. For requests with a recipient
    ///   other than the device, [index] is usually the index of the target. See USB 2.0 Chapter 9.
    /// - [data] is the data to be transmitted as part of the request. It must be between [0, 65535]B.
    /// - [timeout] is how long we should wait for the request. If not provided, we'll wait
    ///   indefinitely.
    ///
    /// The provided callback is called once the operation completes, and receives the actual
    /// length written (or status, on failure).
    #[cfg(feature = "callbacks")]
    pub fn control_write_and_call_back(
        &mut self,
        request_type: RequestType,
        request_number: u8,
        value: u16,
        index: u16,
        data: WriteBuffer,
        callback: AsyncCallback,
        timeout: Option<Duration>,
    ) -> UsbResult<()> {
        self.backend.control_write_nonblocking(
            self,
            request_type.into(),
            request_number,
            value,
            index,
            data,
            callback,
            timeout,
        )
    }

    /// Performs an asynchronous IN control request, with the following parameters:
    /// - [request_type] specifies the USB control request type. It's recommended this is
    /// - [request_number] is the request number. See e.g. USB 2.0 Chapter 9.
    /// - [value] and [index] are arguments to the request. For requests with a recipient
    ///   other than the device, [index] is usually the index of the target. See USB 2.0 Chapter 9.
    /// - [target] is the data to be transmitted as part of the request. It must be between [0, 65535]B.
    /// - [timeout] is how long we should wait for the request. If not provided, we'll wait
    ///   indefinitely.
    ///
    /// Like a typical async function, this method returns a future. However, since _submission_
    /// can fail before the asynchronous component, the future is wrapped in a UsbResult.
    #[cfg(feature = "async")]
    pub fn control_write_async(
        &mut self,
        request_type: RequestType,
        request_number: u8,
        value: u16,
        index: u16,
        target: WriteBuffer,
        timeout: Option<Duration>,
    ) -> UsbResult<UsbFuture> {
        // Create the future, and get a copy of it for our inner callback API,
        // because everyone needs to get themselves a copy.
        let future = UsbFuture::new();
        let shared_state = future.clone_state();

        // Convert our inner callback-API into an async API by having our callback just... complete the future.
        let callback = Box::new(move |result| shared_state.lock().unwrap().complete(result));

        // Finally, trigger the actual async control write.
        self.backend.control_write_nonblocking(
            self,
            request_type.into(),
            request_number,
            value,
            index,
            target,
            callback,
            timeout,
        )?;

        Ok(future)
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
    pub fn read_standard_descriptor(
        &mut self,
        descriptor_type: DescriptorType,
        descriptor_index: u8,
    ) -> UsbResult<Vec<u8>> {
        self.read_descriptor(descriptor_type.into(), descriptor_index)
    }

    #[cfg(feature = "async")]
    ///
    /// (Technically, this can get string descriptors, too, but it'll use the Not Strictly Correct
    ///  default language ID, langID '0').
    pub fn read_standard_descriptor_async(
        &mut self,
        descriptor_type: DescriptorType,
        descriptor_index: u8,
        buffer: ReadBuffer,
    ) -> UsbResult<UsbFuture> {
        let value = ((descriptor_type as u16) << 8) | (descriptor_index as u16);

        self.control_read_async(
            STANDARD_IN_FROM_DEVICE,
            StandardDeviceRequest::GetDescriptor.into(),
            value,
            0,
            buffer,
            None,
        )
    }

    /// Performs a read from the provided endpoint.
    /// Usable for bulk and interrupt reads.
    ///
    /// - [endpoint]: The endpoint number (or address) to read from.
    /// - [max_length]: The maximum length we'll try to read. The actual amount read can be anywhere
    ///   from 0 to this length.
    /// - [timeout]: If provided, the maximum amount of time that will be spent performing this
    ///   read. If not provided, this read will be allowed to continue indefinitely until data
    ///   arrives or an error arises.
    ///
    /// Returns the actual amount of data read.
    pub fn read(
        &mut self,
        endpoint: u8,
        buffer: &mut [u8],
        timeout: Option<Duration>,
    ) -> UsbResult<usize> {
        self.backend.read(self, endpoint, buffer, timeout)
    }

    /// Performs an asynchronous write to the provided endpoint.
    /// Usable for bulk and interrupt writes.
    #[cfg(feature = "callbacks")]
    pub fn read_and_call_back(
        &mut self,
        endpoint: u8,
        buffer: ReadBuffer,
        callback: AsyncCallback,
        timeout: Option<Duration>,
    ) -> UsbResult<()> {
        self.backend
            .read_nonblocking(self, endpoint, buffer, callback, timeout)
    }

    /// Performs an asynchronous read to the provided endpoint.
    /// Usable for bulk and interrupt reads.
    #[cfg(feature = "async")]
    pub fn read_async(
        &mut self,
        endpoint: u8,
        buffer: ReadBuffer,
        timeout: Option<Duration>,
    ) -> UsbResult<UsbFuture> {
        // Create the future, and get a copy of it for our inner callback API,
        // because everyone needs to get themselves a copy.
        let future = UsbFuture::new();
        let shared_state = future.clone_state();

        // Convert our inner callback-API into an async API by having our callback just... complete the future.
        let callback = Box::new(move |result| shared_state.lock().unwrap().complete(result));

        // Finally, trigger the actual async read.
        self.backend
            .read_nonblocking(self, endpoint, buffer, callback, timeout)?;

        Ok(future)
    }

    /// Performs a read from the provided endpoint.
    /// Usable for bulk and interrupt reads.
    ///
    /// This convenience variant generates a vector, for ease of use, but may be slower than
    /// e.g. re-using an appropriately sized buffer for multiple reads.
    ///
    /// - [endpoint]: The endpoint number (or address) to read from.
    /// - [max_length]: The maximum length we'll try to read. The actual amount read can be anywhere
    ///   from 0 to this length.
    /// - [timeout]: If provided, the maximum amount of time that will be spent performing this
    ///   read. If not provided, this read will be allowed to continue indefinitely until data
    ///   arrives or an error arises.
    ///
    /// Returns the actual amount of data read.
    pub fn read_to_vec(
        &mut self,
        endpoint: u8,
        max_length: usize,
        timeout: Option<Duration>,
    ) -> UsbResult<Vec<u8>> {
        let mut buffer = vec![0; max_length as usize];

        // Perform our core read...
        let actual_size = self.read(endpoint, &mut buffer, timeout)?;

        // ... clamp it down to the actual length...
        buffer.truncate(actual_size);

        // ... and return it.
        Ok(buffer)
    }

    /// Performs a write to the provided endpoint.
    /// Usable for bulk and interrupt writes.
    pub fn write(&mut self, endpoint: u8, data: &[u8], timeout: Option<Duration>) -> UsbResult<()> {
        self.backend.write(self, endpoint, data, timeout)
    }

    /// Performs an asynchronous write to the provided endpoint.
    /// Usable for bulk and interrupt writes.
    #[cfg(feature = "callbacks")]
    pub fn write_and_call_back(
        &mut self,
        endpoint: u8,
        data: WriteBuffer,
        callback: AsyncCallback,
        timeout: Option<Duration>,
    ) -> UsbResult<()> {
        self.backend
            .write_nonblocking(self, endpoint, data, callback, timeout)
    }

    /// Performs an asynchronous write to the provided endpoint.
    /// Usable for bulk and interrupt writes.
    #[cfg(feature = "async")]
    pub fn write_async(
        &mut self,
        endpoint: u8,
        data: WriteBuffer,
        timeout: Option<Duration>,
    ) -> UsbResult<UsbFuture> {
        // Create the future, and get a copy of it for our inner callback API,
        // because everyone needs to get themselves a copy.
        let future = UsbFuture::new();
        let shared_state = future.clone_state();

        // Convert our inner callback-API into an async API by having our callback just... complete the future.
        let callback = Box::new(move |result| shared_state.lock().unwrap().complete(result));

        // Finally, trigger the actual async write.
        self.backend
            .write_nonblocking(self, endpoint, data, callback, timeout)?;

        Ok(future)
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
