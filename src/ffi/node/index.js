var ffi = require("ffi-napi");
var ref = require("ref-napi");
var Struct = require("ref-struct-di")(ref);
var Array = require("ref-array-di")(ref);

// Error.stackTraceLimit = Infinity;

const LogCallbacks = Struct({
  // https://github.com/node-ffi-napi/ref-struct-di/blob/master/test/struct.js#L57
  trace: ffi.Function("void", [ref.types.CString]),
  debug: ffi.Function("void", [ref.types.CString]),
});

const LogCallbacksPtr = ref.refType(LogCallbacks);

const MigrationStep = Struct({
  path: ref.types.CString,
  content: ref.types.CString,
  source: ref.types.CString,
});

const Migration = Struct({
  date_time: ref.types.CString,
  steps: Array(MigrationStep),
  built_in: ref.types.bool,
});

const MigrationState = Struct({
  state: ref.types.CString,
  migration: Migration,
});

const MigrationStates = Struct({
  migration_states: Array(Migration),
  num_migration_states: ref.types.size_t,
});

const MigrationStatesPtr = ref.refType(MigrationStates);

const MigrationResult = Struct({
  result: ref.types.CString,
  migration: Migration,
});

const RunnerConfig = Struct({
  configuration_name: ref.types.CString,
  _runner: ref.types.CString,
  database: ref.types.CString,
  index: ref.types.CString,
  database_number: ref.types.uint8,
  ip_or_hostname: ref.types.CString,
  port: ref.types.uint16,
  username: ref.types.CString,
  password: ref.types.CString,
});

const Configuration = Struct({
  migrations_directory: ref.types.CString,
  configured_runners: Array(RunnerConfig),
  number_of_configured_runners: ref.types.size_t,

  // This retains a pointer to Box<Configuration> with a Rust
  // layout, as needed by diff() and up(), etc.
  // _rust_config: "pointer"
});
const ConfigurationPtr = ref.refType(Configuration);

global.libmitre = ffi.Library("./target/debug/libmitre", {
  init_logger: ["void", [LogCallbacksPtr]],
  config_from_file: [ConfigurationPtr, [ref.types.CString]],
  // diff: ["void", ["pointer"]],
});

const fnTrace = function (msg) {
  console.log("trace: fn" + msg);
};

const fnDebug = function (msg) {
  console.log("debug: fn" + msg);
};

function ffiArray(array, length) {
  array.length = length;
  return array;
}

// https://github.com/search?q=ffi.Library&type=Code&l=JavaScript
global.mitre = {
  initLogging: () => {
    let lc = new LogCallbacks({
      trace: fnTrace,
      debug: fnDebug,
    });
    libmitre.init_logger(lc.ref());
  },
  parseConfig: (path) => {
    // NOTE String may not be longer than
    // https://doc.rust-lang.org/std/primitive.isize.html#associatedconstant.MAX
    const config = libmitre.config_from_file(path);

    let {
      migrations_directory,
      configured_runners,
      number_of_configured_runners,
      // _rust_config
    } = config.deref();

    // const runners = ffiArray(configured_runners, number_of_configured_runners);
    configured_runners.length = number_of_configured_runners;

    let cr = {};
    for (let i = 0; i < configured_runners.length; i++) {
      cr[configured_runners[i].configuration_name] = {
        _runner: configured_runners[i]._runner,
        database: configured_runners[i].database,
        index: configured_runners[i].index,
        databaseNumber: configured_runners[i].database_number,
        ipOrHostname: configured_runners[i].ip_or_hostname,
        port: configured_runners[i].port,
        username: configured_runners[i].username,
        password: configured_runners[i].password,
      };
    }

    const result = {
      migrationsDirectory: migrations_directory,
      configuredRunners: cr,
      // _config_ref_against_gc: config,
    };

    // Object.defineProperty(result, "mitre_rust_config", {
    //   configurable: false,
    //   enumerable: false,
    //   value: _rust_config,
    //   writable: false
    // });

    return result;
  },

  diff: (config) => {
    // if (!config) {
    //   throw new Error("diff expects a config")
    // }
    // // const result = libmitre.diff(config.mitre_rust_config).deref();
    // const migrationStates = [];
    // // const migrationStates = ffiArray(result.migration_states, result.num_migration_states);
    // return migrationStates.map(state => Object.assign({}, state,{ migration: {dateTime: new Date(state.migration.date_time), steps: ffiArray(state.migration.steps, state.migration.num_steps), builtIn: state.migration.built_in}}))
  },
};

const mitre = getLibMitre();
mitre.initLogging();

module.exports = mitre;
module.exports.default = mitre;
