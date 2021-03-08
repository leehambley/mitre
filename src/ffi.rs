use std::ffi::{c_void, CString};
use std::os::raw::c_char;

type LoggingFunction = unsafe extern "C" fn(*mut c_char, *mut c_char) -> c_void;
unsafe extern "C" fn default_logger(_lvl: *mut c_char, _msg: *mut c_char) -> c_void {
    panic!("FFI logger not configured");
}

// https://rust-lang.github.io/unsafe-code-guidelines/layout/function-pointers.html

static mut LOGGER: LoggingFunction = default_logger;

/// https://docs.rs/flexi_logger/0.17.1/flexi_logger/struct.Record.html
struct CLIAndFFILogger;
impl log::Log for CLIAndFFILogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        println!("in enabled");
        true
    }
    fn log(&self, record: &log::Record) {
        println!("Ho");
        // if !self.enabled(record.metadata()) {
        //     return;
        // }

        let r_msg = format!(
            "{}:{} -- {}",
            record.level(),
            record.target(),
            record.args()
        );
        let ffi_lvl = CString::new(format!("{}", record.level()))
            .expect("lvl should be convertable to CString");
        let ffi_msg =
            CString::new(format!("{}", record.args())).expect("msg should convertable to CString");
        println!("{}", r_msg);
        unsafe {
            LOGGER(ffi_lvl.into_raw(), ffi_msg.into_raw());
        }
    }
    fn flush(&self) {
        println!("in flush");
    }
}

#[no_mangle]
pub unsafe fn set_log_level(lvl: *mut c_char) {
    let _level = CString::from_raw(lvl)
        .to_str()
        .expect("level should be set");

    // TODO: implement this
    log::set_max_level(log::LevelFilter::Debug);
}

#[no_mangle]
pub unsafe fn set_logger_fn(func: LoggingFunction) {
    // https://docs.rs/env_logger/0.8.3/src/env_logger/lib.rs.html#799-802
    LOGGER = func;
    let _ = log::set_boxed_logger(Box::new(CLIAndFFILogger {}));
    log!(log::Level::Error, "Received errors");
    println!("Hey")
}

#[no_mangle]
pub unsafe fn do_work() {
    for x in 0..10 {
        info!("Iteration Number: {}", x);
    }
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
