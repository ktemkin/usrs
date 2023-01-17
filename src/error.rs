//! Every error that can occur in USRs.

/// Alias to simplify implementing the results of USRs functions.
pub type UsbResult<T> = Result<T, Error>;

#[derive(Debug, PartialEq)]
pub enum Error {
    /// Error for when no devices are found that match a given selector.
    DeviceNotFound,

    /// An unspecified error, with associated OS error number.
    OsError(i64),
    UnspecifiedOsError,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Error::*;

        match self {
            DeviceNotFound => write!(f, "no device found")?,
            OsError(errno) => write!(f, "operating system IO error {}", errno)?,
            UnspecifiedOsError => write!(
                f,
                "operating system IO error, but the OS doesn't specify which",
            )?,
        }

        Ok(())
    }
}

impl std::error::Error for Error {}
