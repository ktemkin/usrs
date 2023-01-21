//! Example that reads USB descriptors from a specified device.

use std::sync::Arc;

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
    println!("\nOpened a device:");
    dbg!(&t5_headset);

    //
    // Read the device descriptor synchronously.
    //

    let descriptor = t5_headset.read_standard_descriptor(DescriptorType::Device, 0)?;
    println!("\n\nIts device descriptor, read synchronously:");
    dbg!(descriptor);

    //
    // Read the device descriptor asynchronously.
    //

    let buffer = usrs::create_read_buffer(1024);
    let size_read = smol::block_on(t5_headset.read_standard_descriptor_async(
        DescriptorType::Device,
        0,
        Arc::clone(&buffer),
    )?)?;

    // Extract our buffer from its async encapsulation...
    let mut buffer = buffer.borrow_mut();

    // ... and print it.
    println!("\n\nIts device descriptor, read asynchronously:");
    dbg!(&buffer.as_mut()[0..size_read]);

    Ok(())
}
