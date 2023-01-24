//! Tools for working with USB device requests.

/// Specifies the direction of a request.
#[repr(u8)]
#[derive(Copy, Debug, Clone, PartialEq, Eq)]
pub enum Direction {
    Out = 0,
    In = 1,
}

impl Direction {
    /// Helper that disambiguates the USB "out" direction.
    pub const HOST_TO_DEVICE: Self = Direction::Out;

    /// Helper that disambiguates the USB "in" direction.
    pub const DEVICE_TO_HOST: Self = Direction::In;
}

/// Specifies the direction of a request.
#[repr(u8)]
#[derive(Copy, Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Standard = 0,
    Class = 1,
    Vendor = 2,
}

/// Specifies the "context"/recipient of a request.
#[repr(u8)]
#[derive(Copy, Debug, Clone, PartialEq, Eq)]
pub enum Recipient {
    Device = 0,
    Interface = 1,
    Endpoint = 2,
    Other = 3,
}

/// Helper for working with USB request-type fields.
#[derive(Copy, Debug, Clone, PartialEq, Eq)]
pub struct RequestType {
    /// Whether the given request is to the device (OUT) or to the host (IN).
    pub direction: Direction,

    /// The USB "request type" associated with this request.
    pub request_type: Type,

    /// The context/recipient to which this request will be delivered.
    pub recipient: Recipient,
}

impl From<&RequestType> for u8 {
    fn from(encoded: &RequestType) -> u8 {
        let direction = (encoded.direction as u8) << 7;
        let request_type = (encoded.request_type as u8) << 5;
        let recipient = encoded.recipient as u8;

        direction | request_type | recipient
    }
}

impl From<RequestType> for u8 {
    fn from(encoded: RequestType) -> u8 {
        (&encoded).into()
    }
}

//
// Helper constants for common request types.
//

/// Shorthand for the common case of performing a standard read of e.g. a device descriptor.
pub const STANDARD_IN_FROM_DEVICE: RequestType = RequestType {
    direction: Direction::In,
    request_type: Type::Standard,
    recipient: Recipient::Device,
};

/// Shorthand for the common case of issuing a standard request; e.g. set_interface.
pub const STANDARD_OUT_TO_DEVICE: RequestType = RequestType {
    direction: Direction::Out,
    request_type: Type::Standard,
    recipient: Recipient::Device,
};

/// Shorthand for the most common type of sending vendor-specific data to the device.
pub const VENDOR_IN_FROM_DEVICE: RequestType = RequestType {
    direction: Direction::In,
    request_type: Type::Vendor,
    recipient: Recipient::Device,
};

/// Shorthand for the most common type of receiving vendor-specific data to the device.
pub const VENDOR_OUT_TO_DEVICE: RequestType = RequestType {
    direction: Direction::Out,
    request_type: Type::Vendor,
    recipient: Recipient::Device,
};

/// Shorthand for the somewhat common case of sending class-specific data to the _interface_.
/// Mind that you'll have to provide the interface number in the request's index.
pub const CLASS_OUT_TO_INTERFACE: RequestType = RequestType {
    direction: Direction::Out,
    request_type: Type::Class,
    recipient: Recipient::Interface,
};

/// Shorthand for the somewhat common case of receiving class-specific data from the _interface_.
/// Mind that you'll have to provide the interface number in the request's index.
pub const CLASS_IN_FROM_INTERFACE: RequestType = RequestType {
    direction: Direction::In,
    request_type: Type::Class,
    recipient: Recipient::Interface,
};

//
// Request type helpers.
//

#[repr(u8)]
#[derive(Copy, Debug, Clone, PartialEq, Eq)]
pub enum StandardDeviceRequest {
    GetStatus = 0,
    ClearFeature = 1,
    SetFeature = 3,
    SetAddress = 5,
    GetDescriptor = 6,
    SetDescriptor = 7,
    GetConfiguration = 8,
    SetConfiguration = 9,
}

impl From<&StandardDeviceRequest> for u8 {
    fn from(request: &StandardDeviceRequest) -> u8 {
        *request as u8
    }
}

impl From<StandardDeviceRequest> for u8 {
    fn from(request: StandardDeviceRequest) -> u8 {
        (&request).into()
    }
}

#[repr(u8)]
#[derive(Copy, Debug, Clone, PartialEq, Eq)]
pub enum DescriptorType {
    Device = 1,
    Configuration = 2,
    String = 3,
    Interface = 4,
    Endpoint = 5,
}

impl From<&DescriptorType> for u8 {
    fn from(descriptor: &DescriptorType) -> u8 {
        *descriptor as u8
    }
}

impl From<DescriptorType> for u8 {
    fn from(descriptor: DescriptorType) -> u8 {
        (&descriptor).into()
    }
}
