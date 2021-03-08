use std::ffi::{c_void, CString};
use std::os::raw::c_char;
pub fn report_status(msg: String) {}

// https://rust-lang.github.io/unsafe-code-guidelines/layout/function-pointers.html

#[no_mangle]
pub unsafe fn set_report_status_callback(func: unsafe extern "C" fn(*mut c_char) -> c_void) {
    let str = CString::new("Hello FFI").expect("boom:reason");
    func(str.into_raw());
}

// #[no_mangle]
// pub unsafe extern "C" fn iterate(
//   node: Option<&Cons>,
//   func: unsafe extern "C" fn(i32, *mut c_void), // note - non-nullable
//   thunk: *mut c_void, // note - this is a thunk, so it's just passed raw
// ) {
//   let mut it = node;
//   while let Some(node) = it {
//     func(node.data, thunk);
//     it = node.next.as_ref().map(|x| &**x);
//   }
// }
