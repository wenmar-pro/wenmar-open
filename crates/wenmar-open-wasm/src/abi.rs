//! The five exports of the `.wasm`, and nothing imported.
//!
//! The caller writes a request into memory it asked for with [`wo_alloc`],
//! calls [`wo_call`], and reads the answer at [`wo_result`], which is
//! [`wo_result_len`] bytes of UTF-8 JSON. The answer stays where it is
//! until the next [`wo_alloc`] or [`wo_call`]: each of them empties it
//! first, so a request that stops without a panic leaves nothing behind
//! that could be read as its answer.
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

/// What every export that starts work does first: the panic hook is in
/// place, and the answer of the request before is gone.
///
/// [`wo_alloc`] needs it as much as [`wo_call`]: it is the first export of
/// every request, and an allocation that fails stops the module without
/// running the panic hook.
fn begin() {
    HOOK.call_once(|| {
        std::panic::set_hook(Box::new(|info| {
            keep(info.to_string().into_bytes());
        }));
    });
    RESULT.with(|result| {
        if let Ok(mut result) = result.try_borrow_mut() {
            result.clear();
        }
    });
}

/// Memory for a request of `len` bytes. [`wo_call`] frees it.
#[unsafe(no_mangle)]
pub extern "C" fn wo_alloc(len: usize) -> *mut u8 {
    begin();
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
    begin();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(request: &str) -> usize {
        let pointer = wo_alloc(request.len());
        // Safety: `pointer` is `request.len()` bytes from `wo_alloc`, and it
        // is given to `wo_call` once.
        unsafe {
            std::ptr::copy_nonoverlapping(request.as_ptr(), pointer, request.len());
            wo_call(pointer, request.len())
        }
    }

    #[test]
    fn the_answer_is_gone_as_soon_as_the_next_request_asks_for_memory() {
        assert!(ask(r#"{"op":"version"}"#) > 0);
        assert!(wo_result_len() > 0);
        let pointer = wo_alloc(8);
        assert_eq!(wo_result_len(), 0);
        // Eight zero bytes are not a request: an error, and the memory is freed.
        // Safety: `pointer` is 8 bytes from `wo_alloc(8)`.
        assert!(unsafe { wo_call(pointer, 8) } > 0);
    }
}
