extern crate libc;
mod reserved;

#[no_mangle]
extern "C" fn reserved_words() {
    // placeholder, see https://www.reddit.com/r/rust/comments/d3c7be/how_do_i_get_rust_ffi_to_return_array_of_structs/
    // and think long and hard about FFI, Structs and arrays, and also your life choices.
    reserved::words();
}
