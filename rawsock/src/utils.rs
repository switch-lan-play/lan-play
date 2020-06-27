use errno::errno;
use libc::{c_char, c_int, strerror};
use std::ffi::CStr;

pub fn cstr_to_string(txt: *const c_char) -> String {
    if txt.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(txt) }
        .to_string_lossy()
        .into_owned()
}

pub fn string_from_errno() -> String {
    cstr_to_string(unsafe { strerror(errno().into()) })
}

#[allow(dead_code)]
pub fn string_from_err_code(code: c_int) -> String {
    let corrected = if code < 0 { -code } else { code };
    cstr_to_string(unsafe { strerror(corrected) })
}
