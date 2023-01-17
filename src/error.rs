//! Every error that can occur in USRs.

/// Alias to simplify implementing the results of USRs functions.
pub type UsbResult<T> = Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    /// Error for when no devices are found that match a given selector.
    DeviceNotFound
}

impl std::fmt::Display for Error {

    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Error::*;

        match self {
            DeviceNotFound => write!(f, "no device found")?,
            _ => write!(f, "Generic USBRs Error")?,
        }

        Ok(())
    }
}

impl std::error::Error for Error {}
