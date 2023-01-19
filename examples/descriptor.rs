//! Example that reads USB descriptors from a specified device.

use usrs::request::DescriptorType;
use usrs::{device, open, DeviceSelector};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Find some device we're interested in working with...
    let t5_headset_info = device(&DeviceSelector {
        vendor_id: Some(0x32f8),
        product_id: Some(0x424c),
        ..Default::default()
    })?;

    // ... open it ...
    let mut t5_headset = open(&t5_headset_info)?;
    dbg!(&t5_headset);

    // ... and ask for its device descriptor.
    let descriptor = t5_headset.read_standard_descriptor(DescriptorType::Device, 0)?;

    dbg!(descriptor);
    Ok(())
}
