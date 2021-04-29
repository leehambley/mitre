use log::{error, trace, warn};
use std::ffi::{CStr, CString};
// rust-analyzer has a bug here showing a false warning about unresolved import
// https://github.com/rust-analyzer/rust-analyzer/issues/6038
use std::os::raw::c_char;

pub use std::os::unix::ffi::OsStrExt;

// Allows use of .diff() on unknown impl StateStore
// result type.
use crate::state_store::StateStore;

type LoggingFunction = extern "C" fn(*mut c_char);
#[derive(Debug, Copy, Clone)]
#[repr(C)]
struct LogCallbacks {
    trace: LoggingFunction,
    debug: LoggingFunction,
    info: LoggingFunction,
    warn: LoggingFunction,
    error: LoggingFunction,
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
            log::Level::Debug => self.debug,
            log::Level::Info => self.info,
            log::Level::Warn => self.warn,
            log::Level::Error => self.error,
        };

        log_fn(CString::new(content.as_str()).unwrap().into_raw())
    }
    fn flush(&self) {}
}

#[no_mangle]
unsafe extern "C" fn init_logger(lc: *mut LogCallbacks) {
    log::set_max_level(log::LevelFilter::Trace);
    match log::set_logger(&*lc) {
        Err(e) => {
            panic!("Error setting logger: {}", e);
        }
        Ok(_) => {
            trace!("Initialized logger");
        }
    }
}

#[derive(Debug)]
#[repr(C)]
struct Configuration {
    migrations_directory: *mut c_char,
    configured_runners: *mut RunnerConfiguration,
    number_of_configured_runners: usize,
    rust_config: *mut crate::config::Configuration,
}

// this derive(Debug) just ensures we generate some boilerplate to hide warnings about
// https://github.com/rust-lang/rust/issues/81658
#[derive(Debug)]
#[repr(C)]
struct RunnerConfiguration {
    configuration_name: *mut c_char,

    _runner: *mut c_char,
    database: *mut c_char,
    index: *mut c_char,
    database_number: u8,
    ip_or_hostname: *mut c_char,
    port: u16,
    username: *mut c_char,
    password: *mut c_char,
}

#[derive(Debug)]
#[repr(C)]
struct MigrationStep {
    direction: *mut c_char,
    path: *mut c_char,
    source: *mut c_char,
}

#[derive(Debug)]
#[repr(C)]
struct Migration {
    date_time: *mut c_char,
    steps: *mut MigrationStep,
    num_steps: usize,
    built_in: u8, // bool
                  // TODO: Flags
}

#[derive(Debug)]
#[repr(C)]
struct MigrationState {
    state: *mut c_char,
    migration: *mut Migration,
}
#[derive(Debug)]
#[repr(C)]
struct MigrationStates {
    migration_state: *mut MigrationState,
    num_migration_states: usize,
}

#[derive(Debug)]
#[repr(C)]
struct MigrationResult {
    result: *mut c_char,
    migration: *mut Migration,
}

// http://jakegoulding.com/rust-ffi-omnibus/string_arguments/
// http://jakegoulding.com/rust-ffi-omnibus/objects/
// https://github.com/andywer/leakage
// https://michael-f-bryan.github.io/rust-ffi-guide/basic_request.html
#[no_mangle]
extern "C" fn config_from_file(p: *const c_char) -> *mut Configuration {
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
        rust_config: Box::into_raw(Box::new(config)),
    }))
}

/// Releases the heap allocated resources created by config_from_file
/// should be called by the FFI host language to avoid a memory leak.
#[no_mangle]
unsafe extern "C" fn free_config_from_file(c: *mut Configuration) {
    assert!(!c.is_null());
    let c: Box<Configuration> = Box::from_raw(c);
    let rcs: Vec<RunnerConfiguration> = Vec::from_raw_parts(
        c.configured_runners,
        c.number_of_configured_runners,
        c.number_of_configured_runners,
    );
    for rc in rcs {
        CString::from_raw(rc.configuration_name);
        CString::from_raw(rc._runner);
        CString::from_raw(rc.database);
        CString::from_raw(rc.index);
        CString::from_raw(rc.ip_or_hostname);
        CString::from_raw(rc.username);
        CString::from_raw(rc.password);
    }
    Box::from_raw(c.rust_config);
    CString::from_raw(c.migrations_directory);
}

/// Takes a pointer to a Box<crate::config::Configuration>, not to the FFI
/// compatible pointer object returned to the host language. The
/// `crate::config::Configuration` pointer is contained within the FFI friendly
/// struct, and freed at the same time. The config will be cloned in
/// `crate::state_store::from_config()`, so can be safely released after calls to
/// this function.
//
// One improvement here might be this, one day:
// https://doc.rust-lang.org/nomicon/ffi.html#representing-opaque-structs
#[no_mangle]
unsafe extern "C" fn diff(c: *mut crate::config::Configuration) -> *mut MigrationStates {
    let rc = Box::from_raw(c);
    let migrations = match crate::migrations::migrations(&rc.clone()) {
        Ok(migrations) => migrations,
        Err(e) => {
            error!("could not list migrations using config {:?}", e);
            return std::ptr::null_mut();
        }
    };

    let migration_states = match StateStore::from_config(&rc.clone()) {
        Ok(mut ss) => match ss.diff(migrations) {
            Ok(diff_results) => {
                let mut num_migration_states: usize = 0;
                let mut migration_states: Vec<MigrationState> = vec![];
                for (migration_state, migration) in diff_results {
                    num_migration_states += 1;
                    let mut num_steps: usize = 0;
                    let mut steps: Vec<MigrationStep> = vec![];
                    for (direction, step) in migration.steps {
                        num_steps += 1;
                        steps.push(MigrationStep {
                            direction: CString::new(format!("{:?}", direction))
                                .unwrap_or_default()
                                .into_raw(),
                            path: CString::new(step.path.to_str().unwrap_or_default())
                                .unwrap_or_default()
                                .into_raw(),
                            source: CString::new(step.source).unwrap().into_raw(),
                        })
                    }
                    migration_states.push(MigrationState {
                        state: CString::new(format!("{:?}", migration_state))
                            .unwrap_or_default()
                            .into_raw(),
                        migration: Box::into_raw(Box::new(Migration {
                            date_time: CString::new(format!(
                                "{}",
                                migration.date_time.format(crate::migrations::FORMAT_STR)
                            ))
                            .unwrap_or_default()
                            .into_raw(),
                            steps: Box::into_raw(steps.into_boxed_slice()) as *mut MigrationStep,
                            built_in: 1,
                            num_steps,
                        })),
                    });
                }
                MigrationStates {
                    migration_state: Box::into_raw(migration_states.into_boxed_slice())
                        as *mut MigrationState,
                    num_migration_states,
                }
            }
            Err(e) => {
                error!("could not run diff() on state store {:?}", e);
                return std::ptr::null_mut();
            }
        },
        Err(e) => {
            error!("could not make state store from config: {:?}", e);
            return std::ptr::null_mut();
        }
    };
    // Box::leak(rc); // don't reclaim this, really, just reference the box contents
    Box::into_raw(Box::new(migration_states))
}

/// Free the results allocated by `diff()`.
/// This function will not free the pointer associated with the configuration as the configuration
/// is cloned into the state store implementation to avoid long-living references, anyway.
#[no_mangle]
unsafe extern "C" fn free_diff(m: *mut MigrationStates) {
    let ms: Box<MigrationStates> = Box::from_raw(m);
    let ms_states: Vec<MigrationState> = Vec::from_raw_parts(
        ms.migration_state,
        ms.num_migration_states,
        ms.num_migration_states,
    );
    for state in ms_states {
        CString::from_raw(state.state);
        let migration = Box::from_raw(state.migration);
        let steps: Vec<MigrationStep> =
            Vec::from_raw_parts(migration.steps, migration.num_steps, migration.num_steps);
        for step in steps {
            CString::from_raw(step.direction);
            CString::from_raw(step.path);
            CString::from_raw(step.source);
        }
    }
}
