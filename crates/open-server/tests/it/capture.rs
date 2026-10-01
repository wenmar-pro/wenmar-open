//! Reading what the server logs while one test runs.
//!
//! `tracing` decides once for the whole process whether a log statement
//! writes anything: the first thread to reach the statement asks the
//! subscriber it can see, and the answer is kept for every thread. A
//! subscriber set for one test's thread only is therefore not enough. When
//! another test's thread reaches the statement first, it sees no subscriber,
//! the answer kept is "never", and the test that wanted the lines reads
//! nothing.
//!
//! So this test binary has one subscriber for the whole process, set before
//! anything of the server runs, and each thread chooses whether to keep the
//! lines written on it. The tests' runtimes have one thread each, so the
//! lines a test's requests write are written on the test's own thread.

use std::cell::RefCell;
use std::io::Write;
use std::sync::Once;

use tracing_subscriber::fmt::MakeWriter;

thread_local! {
    /// What was logged on this thread since its capture began, if one has.
    static KEPT: RefCell<Option<Vec<u8>>> = const { RefCell::new(None) };
}

/// Keeps what is written on a thread that has a capture, and drops the rest.
struct ThisThread;

impl Write for ThisThread {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        KEPT.with_borrow_mut(|kept| {
            if let Some(kept) = kept {
                kept.extend_from_slice(bytes);
            }
        });
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for ThisThread {
    type Writer = ThisThread;

    fn make_writer(&'a self) -> ThisThread {
        ThisThread
    }
}

/// Sets the subscriber of the whole test binary, once.
///
/// Every way a test reaches the server begins with a data file from
/// `common`, and each of those calls this first, so no log statement is
/// reached before the subscriber is in place.
pub fn install() {
    static INSTALLED: Once = Once::new();
    INSTALLED.call_once(|| {
        let subscriber = tracing_subscriber::fmt()
            .with_writer(ThisThread)
            .with_ansi(false)
            .finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("nothing else sets a subscriber in this test binary");
    });
}

/// The log lines written on this thread from `start` until it is dropped.
pub struct Capture {
    // The lines are this thread's: a capture must not move to another.
    _this_thread: std::marker::PhantomData<*const ()>,
}

impl Capture {
    pub fn start() -> Capture {
        install();
        KEPT.set(Some(Vec::new()));
        Capture {
            _this_thread: std::marker::PhantomData,
        }
    }

    pub fn text(&self) -> String {
        KEPT.with_borrow(|kept| {
            String::from_utf8_lossy(kept.as_deref().unwrap_or_default()).into_owned()
        })
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        KEPT.set(None);
    }
}
