//! Thread parking and unparking.
//!
//! This is a copy of the
//! [`park()`][`std::thread::park()`]/[`unpark()`][`std::thread::Thread::unpark()`] mechanism from
//! the standard library.
//!
//! # What is parking
//!
//! Conceptually, each [`Parker`] has a token which is initially not present:
//!
//! * The [`Parker::park()`] method blocks the current thread unless or until the token is
//!   available, at which point it automatically consumes the token. It may also return
//!   *spuriously*, without consuming the token.
//!
//! * The [`Parker::park_timeout()`] method works the same as [`Parker::park()`], but blocks until
//!   a timeout is reached.
//!
//! * The [`Parker::park_deadline()`] method works the same as [`Parker::park()`], but blocks until
//!   a deadline is reached.
//!
//! * The [`Parker::unpark()`] and [`Unparker::unpark()`] methods atomically make the token
//!   available if it wasn't already. Because the token is initially absent, [`Unparker::unpark()`]
//!   followed by [`Parker::park()`] will result in the second call returning immediately.
//!
//! # Analogy with channels
//!
//! Another way of thinking about [`Parker`] is as a bounded
//! [channel][`std::sync::mpsc::sync_channel()`] with capacity of 1.
//!
//! Then, [`Parker::park()`] is equivalent to blocking on a
//! [receive][`std::sync::mpsc::Receiver::recv()`] operation, and [`Unparker::unpark()`] is
//! equivalent to a non-blocking [send][`std::sync::mpsc::SyncSender::try_send()`] operation.

#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms)]

use std::fmt;
use std::marker::PhantomData;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// Parks a thread.
///
/// # Examples
///
/// ```
/// use std::thread;
/// use std::time::Duration;
/// use parking::Parker;
///
/// let p = Parker::new();
/// let u = p.unparker();
///
/// // Make the token available.
/// u.unpark();
/// // Wakes up immediately and consumes the token.
/// p.park();
///
/// thread::spawn(move || {
///     thread::sleep(Duration::from_millis(500));
///     u.unpark();
/// });
///
/// // Wakes up when `u.unpark()` provides the token, but may also wake up
/// // spuriously before that without consuming the token.
/// p.park();
/// ```
pub struct Parker {
    unparker: Unparker,
    _marker: PhantomData<*const ()>,
}

unsafe impl Send for Parker {}

impl Parker {
    /// Creates a new [`Parker`].
    ///
    /// # Examples
    ///
    /// ```
    /// use parking::Parker;
    ///
    /// let p = Parker::new();
    /// ```
    ///
    pub fn new() -> Parker {
        Parker {
            unparker: Unparker {
                inner: Arc::new(Inner {
                    state: AtomicUsize::new(EMPTY),
                    lock: Mutex::new(()),
                    cvar: Condvar::new(),
                }),
            },
            _marker: PhantomData,
        }
    }

    /// Blocks the current thread until the token is made available.
    ///
    /// This method may wake up spuriously without consuming the token, and callers should be
    /// prepared for this possibility.
    ///
    /// # Examples
    ///
    /// ```
    /// use parking::Parker;
    ///
    /// let p = Parker::new();
    /// let u = p.unparker();
    ///
    /// // Make the token available.
    /// u.unpark();
    ///
    /// // Wakes up immediately and consumes the token.
    /// p.park();
    /// ```
    pub fn park(&self) {
        self.unparker.inner.park(None);
    }

    /// Blocks the current thread until the token is made available or the timeout is reached.
    ///
    /// Returns `true` if the token was received before the timeout.
    ///
    /// This method may wake up spuriously without consuming the token, and callers should be
    /// prepared for this possibility.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// use parking::Parker;
    ///
    /// let p = Parker::new();
    ///
    /// // Waits for the token to become available, but will not wait longer than 500 ms.
    /// p.park_timeout(Duration::from_millis(500));
    /// ```
    pub fn park_timeout(&self, timeout: Duration) -> bool {
        self.unparker.inner.park(Some(timeout))
    }

    /// Blocks the current thread until the token is made available or the deadline is reached.
    ///
    /// Returns `true` if the token was received before the deadline.
    ///
    /// This method may wake up spuriously without consuming the token, and callers should be
    /// prepared for this possibility.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::{Duration, Instant};
    /// use parking::Parker;
    ///
    /// let p = Parker::new();
    ///
    /// // Waits for the token to become available, but will not wait longer than 500 ms.
    /// p.park_deadline(Instant::now() + Duration::from_millis(500));
    /// ```
    pub fn park_deadline(&self, deadline: Instant) -> bool {
        self.unparker
            .inner
            .park(Some(deadline.saturating_duration_since(Instant::now())))
    }

    /// Atomically makes the token available if it is not already.
    ///
    /// The next time a thread blocks on this [`Parker`], it will wake up immediately.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    /// use std::time::Duration;
    /// use parking::Parker;
    ///
    /// let p = Parker::new();
    /// let u = p.unparker();
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_millis(500));
    ///     u.unpark();
    /// });
    ///
    /// // Wakes up when `u.unpark()` provides the token, but may also wake up
    /// // spuriously before that without consuming the token.
    /// p.park();
    /// ```
    pub fn unpark(&self) {
        self.unparker.unpark()
    }

    /// Returns a handle for unparking.
    ///
    /// The returned [`Unparker`] handle can be cloned and shared among threads.
    ///
    /// # Examples
    ///
    /// ```
    /// use parking::Parker;
    ///
    /// let p = Parker::new();
    /// let u = p.unparker();
    ///
    /// // Make the token available.
    /// u.unpark();
    /// // Wakes up immediately and consumes the token.
    /// p.park();
    /// ```
    pub fn unparker(&self) -> Unparker {
        self.unparker.clone()
    }
}

impl fmt::Debug for Parker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("Parker { .. }")
    }
}

/// Unparks a thread.
pub struct Unparker {
    inner: Arc<Inner>,
}

unsafe impl Send for Unparker {}
unsafe impl Sync for Unparker {}

impl Unparker {
    /// Atomically makes the token available if it is not already.
    ///
    /// This method will wake up the thread blocked on [`Parker::park()`],
    /// [`Parker::park_timeout()`], or [`Parker::park_deadline()`], if there is one.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    /// use std::time::Duration;
    /// use parking::Parker;
    ///
    /// let p = Parker::new();
    /// let u = p.unparker();
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_millis(500));
    ///     u.unpark();
    /// });
    ///
    /// // Wakes up when `u.unpark()` provides the token, but may also wake up
    /// // spuriously before that without consuming the token.
    /// p.park();
    /// ```
    pub fn unpark(&self) {
        self.inner.unpark()
    }
}

impl fmt::Debug for Unparker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("Unparker { .. }")
    }
}

impl Clone for Unparker {
    fn clone(&self) -> Unparker {
        Unparker {
            inner: self.inner.clone(),
        }
    }
}

const EMPTY: usize = 0;
const PARKED: usize = 1;
const NOTIFIED: usize = 2;

struct Inner {
    state: AtomicUsize,
    lock: Mutex<()>,
    cvar: Condvar,
}

impl Inner {
    fn park(&self, timeout: Option<Duration>) -> bool {
        // If we were previously notified then we consume this notification and return quickly.
        if self
            .state
            .compare_exchange(NOTIFIED, EMPTY, SeqCst, SeqCst)
            .is_ok()
        {
            return true;
        }

        // If the timeout is zero, then there is no need to actually block.
        if let Some(dur) = timeout {
            if dur == Duration::from_millis(0) {
                return false;
            }
        }

        // Otherwise we need to coordinate going to sleep.
        let mut m = self.lock.lock().unwrap();

        match self.state.compare_exchange(EMPTY, PARKED, SeqCst, SeqCst) {
            Ok(_) => {}
            // Consume this notification to avoid spurious wakeups in the next park.
            Err(NOTIFIED) => {
                // We must read `state` here, even though we know it will be `NOTIFIED`. This is
                // because `unpark` may have been called again since we read `NOTIFIED` in the
                // `compare_exchange` above. We must perform an acquire operation that synchronizes
                // with that `unpark` to observe any writes it made before the call to `unpark`. To
                // do that we must read from the write it made to `state`.
                let old = self.state.swap(EMPTY, SeqCst);
                assert_eq!(old, NOTIFIED, "park state changed unexpectedly");
                return true;
            }
            Err(n) => panic!("inconsistent park_timeout state: {}", n),
        }

        match timeout {
            None => {
                loop {
                    // Block the current thread on the conditional variable.
                    m = self.cvar.wait(m).unwrap();

                    match self.state.compare_exchange(NOTIFIED, EMPTY, SeqCst, SeqCst) {
                        Ok(_) => return true, // got a notification
                        Err(_) => {}          // spurious wakeup, go back to sleep
                    }
                }
            }
            Some(timeout) => {
                // Wait with a timeout, and if we spuriously wake up or otherwise wake up from a
                // notification we just want to unconditionally set `state` back to `EMPTY`, either
                // consuming a notification or un-flagging ourselves as parked.
                let (_m, _result) = self.cvar.wait_timeout(m, timeout).unwrap();

                match self.state.swap(EMPTY, SeqCst) {
                    NOTIFIED => true, // got a notification
                    PARKED => false,  // no notification
                    n => panic!("inconsistent park_timeout state: {}", n),
                }
            }
        }
    }

    pub fn unpark(&self) {
        // To ensure the unparked thread will observe any writes we made before this call, we must
        // perform a release operation that `park` can synchronize with. To do that we must write
        // `NOTIFIED` even if `state` is already `NOTIFIED`. That is why this must be a swap rather
        // than a compare-and-swap that returns if it reads `NOTIFIED` on failure.
        match self.state.swap(NOTIFIED, SeqCst) {
            EMPTY => return,    // no one was waiting
            NOTIFIED => return, // already unparked
            PARKED => {}        // gotta go wake someone up
            _ => panic!("inconsistent state in unpark"),
        }

        // There is a period between when the parked thread sets `state` to `PARKED` (or last
        // checked `state` in the case of a spurious wakeup) and when it actually waits on `cvar`.
        // If we were to notify during this period it would be ignored and then when the parked
        // thread went to sleep it would never wake up. Fortunately, it has `lock` locked at this
        // stage so we can acquire `lock` to wait until it is ready to receive the notification.
        //
        // Releasing `lock` before the call to `notify_one` means that when the parked thread wakes
        // it doesn't get woken only to have to wait for us to release `lock`.
        drop(self.lock.lock().unwrap());
        self.cvar.notify_one();
    }
}
