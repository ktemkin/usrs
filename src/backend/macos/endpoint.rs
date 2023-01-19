//! Endpoint related tools for the macOS backend.

/// Helper that annotates that we're working with an OUT address.
/// The function, it does *nothing*.
pub const fn address_for_out_endpoint(number: u8) -> u8 {
    number
}

/// Helper that converts an IN endpoint number to an endpoint address.
pub const fn address_for_in_endpoint(number: u8) -> u8 {
    number | 0x80
}

/// Helper that extracts the endpoint number from an endpoint address.
#[allow(dead_code)]
pub const fn number_for_endpoint_address(address: u8) -> u8 {
    address & 0x7F
}

/// Helper that identifies if an endpoint address refers to an IN endpoint.
#[allow(dead_code)]
pub const fn endpoint_address_is_in(address: u8) -> bool {
    (address & 0x80) != 0
}
