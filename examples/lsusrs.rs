//! Enumeration example for USRs.

use usrs::Host;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Create a "usb host" object, which is the top-level interface for working with USB devices.
    let mut host = Host::new()?;

    // Print each device attached to our system.
    for device in host.all_devices()? {
        println!(
            "ID {:04x}:{:04x} {} {}",
            device.vendor_id,
            device.product_id,
            device.vendor.unwrap_or("[Unknown Vendor]".to_owned()),
            device.product.unwrap_or("[Unknown Product]".to_owned())
        );
    }

    Ok(())
}
