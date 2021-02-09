use std::ffi::CString;
use std::os::raw::c_char;
extern crate libc;
mod mitre;

#[repr(C)]
pub struct ReservedWords {
    len: usize,
    words: *mut ReservedWord,
}

#[repr(C)]
#[derive(Debug)]
pub struct ReservedWord {
    word: *mut c_char,
    reason: *mut c_char,
    kind: *mut c_char,
}

// #[no_mangle]
// extern "C" fn reserved_word() -> *mut ReservedWord {
//     let w = CString::new("word").expect("boom:word");
//     let r = CString::new("reason").expect("boom:reason");
//     let k = CString::new(reserved::Kind::Runner.to_string()).expect("boom:kind");

//     let word = ReservedWord {
//         word: w.into_raw(),
//         reason: r.into_raw(),
//         kind: k.into_raw(),
//     };
//     // eprintln!("{:?}", word);
//     Box::into_raw(Box::new(word))
// }

#[no_mangle]
extern "C" fn reserved_words() -> *mut ReservedWords {
    let mut v: Vec<ReservedWord> = vec![];

    // placeholder, see https://www.reddit.com/r/rust/comments/d3c7be/how_do_i_get_rust_ffi_to_return_array_of_structs/
    // and think long and hard about FFI, Structs and arrays, and also your life choices.
    // for word in reserved::words() {
    // let w = CString::new(word.word).expect("boom:word");
    // let r = CString::new(word.reason).expect("boom:reason");
    // let k = CString::new(reserved::Kind::Runner.to_string()).expect("boom:kind");

    // v.push(ReservedWord {
    //     word: w.into_raw(),
    //     reason: r.into_raw(),
    //     kind: k.into_raw(),
    // });
    // }

    let rw = ReservedWords {
        len: v.len(),
        words: Box::into_raw(v.into_boxed_slice()) as *mut ReservedWord,
    };

    Box::into_raw(Box::new(rw))
}

#[no_mangle]
extern "C" fn free_reserved_words(ptr: *mut ReservedWords) {
    if ptr.is_null() {
        eprintln!("free_reserved_words() error got NULL ptr!");
        ::std::process::abort();
    }
    unsafe {
        let w: Box<ReservedWords> = Box::from_raw(ptr);
        let words: Vec<ReservedWord> = Vec::from_raw_parts(w.words, w.len, w.len);
        for word in words {
            CString::from_raw(word.kind);
            CString::from_raw(word.reason);
            CString::from_raw(word.word);
        }
    }
}
