//! The five exports of the `.wasm`, and nothing imported.
//!
//! The caller writes a request into memory it asked for with [`wo_alloc`],
//! calls [`wo_call`], and reads the answer at [`wo_result`], which is
//! [`wo_result_len`] bytes of UTF-8 JSON. The answer stays where it is
//! until the next call.
//!
//! A panic stops the module: WebAssembly has no unwinding here, so the call
//! traps and the caller sees an exception. Before it stops, the panic's
//! message is left where the answer would be, so the caller can read it
//! and must then throw the instance away.

use std::cell::RefCell;
use std::sync::Once;

use crate::engine::Engine;

thread_local! {
    static ENGINE: RefCell<Engine> = RefCell::new(Engine::new());
    static RESULT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

static HOOK: Once = Once::new();

fn keep(bytes: Vec<u8>) -> usize {
    RESULT.with(|result| match result.try_borrow_mut() {
        Ok(mut result) => {
            *result = bytes;
            result.len()
        }
        Err(_) => 0,
    })
}

/// Memory for a request of `len` bytes. [`wo_call`] frees it.
#[unsafe(no_mangle)]
pub extern "C" fn wo_alloc(len: usize) -> *mut u8 {
    Box::into_raw(vec![0u8; len].into_boxed_slice()).cast::<u8>()
}

/// Answers the request at `ptr`, which must be `len` bytes from
/// [`wo_alloc`]`(len)`, and returns the length of the answer.
///
/// # Safety
///
/// `ptr` must come from `wo_alloc(len)` and must not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wo_call(ptr: *mut u8, len: usize) -> usize {
    HOOK.call_once(|| {
        std::panic::set_hook(Box::new(|info| {
            keep(info.to_string().into_bytes());
        }));
    });
    // Safety: the caller's promise above, and `wo_alloc` made exactly this.
    let input = unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)) };
    let request = String::from_utf8_lossy(&input);
    let answer = ENGINE.with(|engine| engine.borrow_mut().call(&request));
    keep(answer.into_bytes())
}

/// Where the last answer is.
#[unsafe(no_mangle)]
pub extern "C" fn wo_result() -> *const u8 {
    RESULT.with(|result| result.borrow().as_ptr())
}

/// How long the last answer is, in bytes.
#[unsafe(no_mangle)]
pub extern "C" fn wo_result_len() -> usize {
    RESULT.with(|result| result.borrow().len())
}
