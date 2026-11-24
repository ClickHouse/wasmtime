use crate::{wasm_frame_vec_t, wasm_name_t};
use wasmtime::{Error, Result, format_err};

#[repr(C)]
#[cfg_attr(wasmtime_cfi, cfi_encoding = "14wasmtime_error")]
pub struct wasmtime_error_t {
    error: Error,
}

wasmtime_c_api_macros::declare_own!(wasmtime_error_t);

/// Takes ownership of an error returned by a C callback. Null means no error.
pub(crate) unsafe fn error_from_raw(error: *mut wasmtime_error_t) -> Option<Box<wasmtime_error_t>> {
    if error.is_null() {
        None
    } else {
        Some(Box::from_raw(error))
    }
}

impl From<Error> for wasmtime_error_t {
    fn from(error: Error) -> wasmtime_error_t {
        wasmtime_error_t { error }
    }
}

impl From<wasmtime_error_t> for Error {
    fn from(cerr: wasmtime_error_t) -> Error {
        cerr.error
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wasmtime_error_new(
    msg: *const std::ffi::c_char,
) -> Option<Box<wasmtime_error_t>> {
    let msg_bytes = unsafe { std::ffi::CStr::from_ptr(msg).to_bytes() };
    let msg_string = String::from_utf8_lossy(msg_bytes).into_owned();
    Some(Box::new(wasmtime_error_t::from(format_err!(msg_string))))
}

pub(crate) fn handle_result<T>(
    result: Result<T>,
    ok: impl FnOnce(T),
) -> Option<Box<wasmtime_error_t>> {
    match result {
        Ok(value) => {
            ok(value);
            None
        }
        Err(error) => Some(Box::new(wasmtime_error_t { error })),
    }
}

/// Runs `f`, turning a panic into an error: a panic that unwinds out of an `extern "C"` function aborts the process.
pub(crate) fn catch_panic(
    f: impl FnOnce() -> Option<Box<wasmtime_error_t>>,
) -> Option<Box<wasmtime_error_t>> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(result) => result,
        Err(panic) => Some(Box::new(wasmtime_error_t {
            error: error_from_panic(panic),
        })),
    }
}

pub(crate) fn error_from_panic(panic: Box<dyn std::any::Any + Send>) -> Error {
    if let Some(msg) = panic.downcast_ref::<String>() {
        Error::msg(msg.clone())
    } else if let Some(msg) = panic.downcast_ref::<&'static str>() {
        Error::msg(*msg)
    } else {
        Error::msg("rust panic happened")
    }
}

pub(crate) fn bad_utf8() -> Option<Box<wasmtime_error_t>> {
    Some(Box::new(wasmtime_error_t {
        error: format_err!("input was not valid utf-8"),
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn wasmtime_error_message(error: &wasmtime_error_t, message: &mut wasm_name_t) {
    message.set_buffer(format!("{:?}", error.error).into_bytes());
}

#[unsafe(no_mangle)]
pub extern "C" fn wasmtime_error_exit_status(raw: &wasmtime_error_t, status: &mut i32) -> bool {
    #[cfg(feature = "wasi")]
    if let Some(exit) = raw.error.downcast_ref::<wasmtime_wasi::I32Exit>() {
        *status = exit.0;
        return true;
    }

    // Squash unused warnings in wasi-disabled builds.
    drop((raw, status));

    false
}

#[unsafe(no_mangle)]
pub extern "C" fn wasmtime_error_wasm_trace<'a>(
    raw: &'a wasmtime_error_t,
    out: &mut wasm_frame_vec_t<'a>,
) {
    crate::trap::error_trace(&raw.error, out)
}
