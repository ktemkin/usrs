//! Example that reads USB descriptors from a specified device.

use usrs::{device, open, DeviceSelector};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Find the device we're interested in working with...
    let t5_headset_info = device(&DeviceSelector {
        vendor_id: Some(0x32f8),
        product_id: Some(0x424c),
        ..Default::default()
    })?;

    // ... open it ...
    let t5_headset = open(&t5_headset_info);
    dbg!(t5_headset);

    Ok(())
}
