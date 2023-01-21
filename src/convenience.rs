//! Convenience functions to make working with the library easier.

use std::{cell::RefCell, sync::Arc};

use crate::ReadBuffer;

/// Convenience function that creates a read buffer suitable for use with our async functions.
pub fn create_read_buffer(size: usize) -> ReadBuffer {
    Arc::new(RefCell::new(vec![0; size]))
}
