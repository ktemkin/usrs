//! Future definitions; for async support.

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::Context,
    task::{Poll, Waker},
};

use crate::UsbResult;

// Shared state between a UsbFuture and the backend performing its action.
pub(crate) struct UsbFutureState {
    /// Tracks whether the transfer has been completed.
    pending: bool,

    /// The result of the USB transfer. Valid only once the transaction has been completed.
    result: Option<UsbResult<usize>>,

    /// If we've been poll()'d, this contains the waker object used to indicate completion.
    waker: Option<Waker>,
}

impl UsbFutureState {
    /// Creates the inner data of for a UsbFuture.
    pub(crate) fn new() -> UsbFutureState {
        UsbFutureState {
            pending: true,
            result: None,
            waker: None,
        }
    }

    /// Callback to be issued when the USB transfer has been completed.
    pub(crate) fn complete(&mut self, result: UsbResult<usize>) {
        self.result = Some(result);
        self.pending = false;

        // If we've already been poll()'d, we'll have been given a waker,
        // which will let us notify the async executor that our future is complete.
        //
        // If we have one, notify it that we're done.
        if let Some(waker) = self.waker.take() {
            waker.wake()
        }
    }
}

/// Core asynchronous Future that waits on the results of USB operations.
pub struct UsbFuture {
    /// The state shared between the future and the backend.
    state: Arc<Mutex<UsbFutureState>>,
}

impl UsbFuture {
    /// Creates a new UsbFuture, which waits on completion of a USB event.
    pub(crate) fn new() -> UsbFuture {
        UsbFuture {
            state: Arc::new(Mutex::new(UsbFutureState::new())),
        }
    }

    /// Gets an owned handle onto our UsbFutureState.
    pub(crate) fn clone_state(&self) -> Arc<Mutex<UsbFutureState>> {
        Arc::clone(&self.state)
    }
}

impl Future for UsbFuture {
    type Output = UsbResult<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.state.lock().unwrap();

        // If our transaction is still pending, we'll need to capture the waker,
        // and indicate that we're not done.
        if state.pending {
            // Store the waker for later use...
            state.waker = Some(cx.waker().clone());

            // ... and notify our caller that we're not done yet.
            Poll::Pending
        }
        // Otherwise, return our result, since we're done.
        else {
            Poll::Ready(
                state
                    .result
                    .take()
                    .expect("future was complete without result"),
            )
        }
    }
}
