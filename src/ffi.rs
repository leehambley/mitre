use std::ffi::{CStr, CString};
use std::os::raw::c_char;
// rust-analyzer has a bug here showing a false warning about unresolved import
// https://github.com/rust-analyzer/rust-analyzer/issues/6038
use std::os::unix::ffi::OsStrExt;

type LoggingFunction = extern "C" fn(*mut c_char);
#[derive(Debug,Copy,Clone)]
#[repr(C)]
struct LogCallbacks {
    trace: LoggingFunction,
    debug: LoggingFunction,
}

impl log::Log for LogCallbacks {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }
    fn log(&self, record: &log::Record) {
        let content = format!(
            "{} : {} -- {}",
            record.level(),
            record.target(),
            record.args()
        );
        let log_fn = match record.level() {
            log::Level::Trace => self.trace,
            _ => self.debug
        };

        log_fn(CString::new(content.as_str()).unwrap().into_raw())
    }
    fn flush(&self) {
        
    }
}



#[no_mangle]
unsafe extern "C" fn init_logger(lc: *mut LogCallbacks) {
    error!("pre init max");
    log::set_max_level(log::max_level());
    error!("pre init");
    match log::set_logger(&*lc) {
       Err(e) => {
           panic!("Error setting logger: {}",e);
       }
       Ok(_) =>{
        trace!("FFI: Initialized logger");
       }
   }
   info!("FFI: Initialized logger");
}

#[derive(Debug)]
#[repr(C)]
pub struct Configuration {
    pub migrations_directory: *mut c_char,
    pub configured_runners: *mut RunnerConfiguration,
    pub number_of_configured_runners: usize,
}

// this derive(Debug) just ensures we generate some boilerplate to hide warnings about
// https://github.com/rust-lang/rust/issues/81658
#[derive(Debug)]
#[repr(C)]
pub struct RunnerConfiguration {
    pub configuration_name: *mut c_char,

    pub _runner: *mut c_char,
    pub database: *mut c_char,
    pub index: *mut c_char,
    pub database_number: u8,
    pub ip_or_hostname: *mut c_char,
    pub port: u16,
    pub username: *mut c_char,
    pub password: *mut c_char,
}

// http://jakegoulding.com/rust-ffi-omnibus/string_arguments/
// http://jakegoulding.com/rust-ffi-omnibus/objects/
// https://github.com/andywer/leakage
// https://michael-f-bryan.github.io/rust-ffi-guide/basic_request.html
#[no_mangle]
pub extern "C" fn config_from_file(p: *const c_char) -> *mut Configuration {
    trace!("FFI: Getting config from file");
    let path_as_str = unsafe {
        match CStr::from_ptr(p).to_str() {
            Ok(s) => s,
            Err(e) => {
                error!("could not create string from pointer: {:?}", e);
                return std::ptr::null_mut(); // invalid pointer?
            }
        }
    };

    let r_path = std::path::Path::new(path_as_str);

    let config = match crate::config::Configuration::from_file(&r_path) {
        Ok(config) => config,
        Err(e) => {
            warn!("Error: {:?}: {:#?}", e, r_path);
            return std::ptr::null_mut();
        }
    };

    // https://dev.to/leehambley/sending-complex-structs-to-ruby-from-rust-4e61
    let mut configured_runners: Vec<RunnerConfiguration> = vec![];
    for (configuration_name, rc) in config.configured_runners.clone() {
        configured_runners.push(RunnerConfiguration {
            configuration_name: CString::new(configuration_name).unwrap().into_raw(),

            _runner: CString::new(rc._runner).unwrap().into_raw(),
            database: CString::new(rc.database.unwrap_or_default())
                .unwrap()
                .into_raw(),
            index: CString::new(rc.index.unwrap_or_default())
                .unwrap()
                .into_raw(),
            database_number: rc.database_number.unwrap_or_default(),
            ip_or_hostname: CString::new(rc.ip_or_hostname.unwrap_or_default())
                .unwrap()
                .into_raw(),
            port: rc.port.unwrap_or_default(),
            username: CString::new(rc.username.unwrap_or_default())
                .unwrap()
                .into_raw(),
            password: CString::new(rc.password.unwrap_or_default())
                .unwrap()
                .into_raw(),
        });
    }

    Box::into_raw(Box::new(Configuration {
        migrations_directory: CString::new(config.migrations_directory.to_str().expect(""))
            .unwrap()
            .into_raw(),
        configured_runners: Box::into_raw(configured_runners.into_boxed_slice())
            as *mut RunnerConfiguration,
        number_of_configured_runners: config.configured_runners.keys().len(),
    }))
}
