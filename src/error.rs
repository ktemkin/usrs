//! Every error that can occur in USRs.

/// Alias to simplify implementing the results of USRs functions.
pub type UsbResult<T> = Result<T, Error>;

#[derive(Debug, PartialEq)]
pub enum Error {
    /// An operation isn't supported; e.g. by this backend or device.
    Unsupported,

    /// Error for when no devices are found that match a given selector.
    DeviceNotFound,

    /// Error for when a device is not yet, or no longer, open.
    DeviceNotOpen,

    /// Error representing a device that has no real USB representation;
    /// generated if we try to open e.g. a billboard device that the OS won't talk to.
    DeviceNotReal,

    /// Error for when the device is reserved by someone who isn't us.
    DeviceReserved,

    /// Error for when a USB stall occurs unexpectedly.
    Stalled,

    /// Targeting a non-existent endpoint.
    InvalidEndpoint,

    /// Targeting a non-existent interface.
    InvalidInterface,

    /// An operation exceeded the timeout interval.
    TimedOut,

    /// An argument was provided with an inalid/non-allowed value.
    InvalidArgument,

    /// A transfer was aborted.
    Aborted,

    /// The response wouldn't fit in the provided buffer.
    Overrun,

    /// The OS won't let us touch this resource.
    PermissionDenied,

    /// An unspecified error, with associated OS error number.
    OsError(i64),

    /// An OS error happened, but we can't get a description from it.
    UnspecifiedOsError,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Error::*;

        match self {
            Unsupported => write!(f, "operation is not supported")?,
            DeviceNotFound => write!(f, "no device found")?,
            DeviceNotOpen => write!(f, "tried to perform an operation on a non-open device")?,
            DeviceNotReal => write!(
                f,
                "tried to work with a device that isn't real to your OS (like a billboard class device)"
            )?,
            DeviceReserved => write!(f, "device reserved by someone else")?,
            Stalled => write!(f, "unexpected transfer stall")?,
            InvalidEndpoint => write!(f, "invalid endpoint")?,
            InvalidInterface => write!(f, "invalid interface")?,
            TimedOut => write!(f, "timed out")?,
            Overrun => write!(f, "buffer overrun")?,
            InvalidArgument => write!(f, "invalid argument")?,
            PermissionDenied => write!(f, "permission denied")?,
            Aborted => write!(f, "aborted")?,
            OsError(errno) => write!(f, "operating system IO error {errno}")?,
            UnspecifiedOsError => write!(
                f,
                "operating system IO error, but the OS doesn't specify which",
            )?,
        }

        Ok(())
    }
}

impl std::error::Error for Error {}
