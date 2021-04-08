use std::os::raw::c_char;
use std::ffi::{CStr, OsStr};
use std::path::Path;
use std::os::unix::ffi::OsStrExt;

struct RunnerConfiguration {
  configuration_name: String,
  
  _runner: String,
  database: String,
  index: String,
  database_number: u8,
  ip_or_hostname: String,
  port: u16,
  username: String,
  password: String
}

struct Configuration {
  migrations_directory: String,
  configured_runners: *mut RunnerConfiguration,
  number_of_configured_runners: usize,
}

// http://jakegoulding.com/rust-ffi-omnibus/string_arguments/
// http://jakegoulding.com/rust-ffi-omnibus/objects/
// https://github.com/andywer/leakage
#[no_mangle]
extern "C" fn config_from_file(p: *const c_char) -> *mut Configuration {
    env_logger::Builder::new()
        .filter(None, log::LevelFilter::Trace)
        .parse_env("MITRE_LOG")
        .init();
  trace!("provided path is {:?}", p);
    
    trace!("FFI: config_from_file()");
  let c_str = unsafe {
    assert!(!p.is_null());
    CStr::from_ptr(p)
  };
  trace!("provided c_str is {:?}", c_str);
  let osstr = OsStr::from_bytes(c_str.to_bytes());
  let r_path: &Path = osstr.as_ref();
  let config = match crate::config::Configuration::from_file(&r_path) {
    Ok(config) => config,
    Err(e) =>   {
        warn!("Error: {:?}: {:#?}", e, r_path);
        return std::ptr::null_mut();
    },
  };

  // https://dev.to/leehambley/sending-complex-structs-to-ruby-from-rust-4e61
  let mut configured_runners: Vec<RunnerConfiguration> = vec![];
  for (configuration_name, rc) in config.configured_runners.clone() {
    configured_runners.push(RunnerConfiguration{
      configuration_name,
      _runner: rc._runner,
      database: rc.database.unwrap_or_default(),
      index: rc.index.unwrap_or_default(),
      database_number: rc.database_number.unwrap_or_default(),
      ip_or_hostname: rc.ip_or_hostname.unwrap_or_default(),
      port: rc.port.unwrap_or_default(),
      username: rc.username.unwrap_or_default(),
      password: rc.password.unwrap_or_default(),
    });
  }

  Box::into_raw(Box::new(Configuration {
    migrations_directory: String::from(config.migrations_directory.to_str().unwrap_or_default()),
    configured_runners: Box::into_raw(configured_runners.into_boxed_slice()) as *mut RunnerConfiguration,
    number_of_configured_runners: config.configured_runners.keys().len(),
  }))
}