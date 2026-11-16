use serde_json::{Value, json};
use std::{
    ffi::{CStr, CString, c_char, c_void},
    panic::{AssertUnwindSafe, catch_unwind},
};

type Callback = extern "C" fn(*mut c_void, *const c_char) -> *mut c_char;
unsafe extern "C" {
    fn swatch_qt_run(mode: *const c_char, callback: Callback, context: *mut c_void) -> i32;
}
extern "C" fn dispatch<F: FnMut(Value) -> Value>(
    context: *mut c_void,
    request: *const c_char,
) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let request = unsafe { CStr::from_ptr(request) }
            .to_str()
            .map_err(|e| e.to_string())?;
        let value = serde_json::from_str(request).map_err(|e| e.to_string())?;
        Ok::<_, String>(unsafe { &mut *(context as *mut F) }(value))
    }));
    let value = match result {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => json!({"error":error}),
        Err(_) => json!({"error":"An internal error occurred"}),
    };
    if value.is_null() {
        return std::ptr::null_mut();
    }
    CString::new(value.to_string())
        .expect("JSON contains no NUL")
        .into_raw()
}
/// Releases a response allocated by the Rust request callback.
///
/// # Safety
/// `value` must be null or an unfreed pointer returned by `dispatch`, and must
/// be released exactly once after the native side finishes reading it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn swatch_qt_free(value: *mut c_char) {
    if !value.is_null() {
        drop(unsafe { CString::from_raw(value) });
    }
}
pub fn run<F: FnMut(Value) -> Value>(mode: &str, mut handler: F) -> i32 {
    let Ok(mode) = CString::new(mode) else {
        return 1;
    };
    unsafe {
        swatch_qt_run(
            mode.as_ptr(),
            dispatch::<F>,
            &mut handler as *mut F as *mut c_void,
        )
    }
}
