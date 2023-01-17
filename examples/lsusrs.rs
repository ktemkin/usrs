//! Enumeration example for USRs.

use usrs::UsbHost;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a "usb host" object, which is the top-level interface for working with USB devices.
    let host = UsbHost::new()?;

    // Print each device attached to our system.
    for device in host.all_devices()? {
        dbg!(device);
    }

    Ok(())
}
