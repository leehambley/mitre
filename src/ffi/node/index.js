var ffi = require("ffi-napi");
var ref = require("ref-napi");
var Struct = require("ref-struct-di")(ref);
var Array = require("ref-array-di")(ref);

// Error.stackTraceLimit = Infinity;

const LogCallbacks = Struct({
  // https://github.com/node-ffi-napi/ref-struct-di/blob/master/test/struct.js#L57
  trace: ffi.Function("void", [ref.types.CString]),
  debug: ffi.Function("void", [ref.types.CString]),
  info: ffi.Function("void", [ref.types.CString]),
  warn: ffi.Function("void", [ref.types.CString]),
  error: ffi.Function("void", [ref.types.CString]),
});

const LogCallbacksPtr = ref.refType(LogCallbacks);

const MigrationStep = Struct({
  direction: ref.types.CString,
  path: ref.types.CString,
  source: ref.types.CString,
});

const Migration = Struct({
  date_time: ref.types.CString,
  steps: Array(MigrationStep),
  num_steps: ref.types.size_t,
  built_in: ref.types.uint8, // bool?
});

const MigrationState = Struct({
  state: ref.types.CString,
  migration: ref.refType(Migration),
});

const MigrationStates = Struct({
  migration_states: Array(MigrationState),
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
  configured_drivers: Array(RunnerConfig),
  number_of_configured_drivers: ref.types.size_t,

  // This retains a pointer to Box<Configuration> with a Rust
  // layout, as needed by diff() and up(), etc. Do not try
  // to do anything with this pointer outside Rust.
  _rust_config: "pointer",
});

const ConfigurationPtr = ref.refType(Configuration);

global.libmitre = ffi.Library("./target/debug/libmitre", {
  init_logger: ["void", [LogCallbacksPtr]],
  config_from_file: [ConfigurationPtr, [ref.types.CString]],
  diff: [MigrationStatesPtr, ["pointer"]],
});

// https://github.com/search?q=ffi.Library&type=Code&l=JavaScript
global.mitre = {
  initLogging: () => {
    // Rust defines these:
    //   https://docs.rs/log/0.4.1/log/enum.Level.html
    // Node has these:
    //   https://nodejs.org/api/console.html
    global.loggingInitialized = true;
    if (!global.loggingInitialized) {
      let lc = new LogCallbacks({
        trace: console.log,
        debug: console.log,
        info: console.log,
        warn: console.log,
        error: console.log,
      });
      libmitre.init_logger(lc.ref());
    } else {
      // TODO: Improve this, silently failing or just logging are both horrible ideas.
      // console.warn("Logging has already been configured once, cannot do it again");
    }
  },
  parseConfig: (path) => {
    // NOTE String may not be longer than this, but we're not enforcing that or
    // checking in any way.
    // https://doc.rust-lang.org/std/primitive.isize.html#associatedconstant.MAX
    const config = libmitre.config_from_file(path);

    let {
      migrations_directory,
      configured_drivers,
      number_of_configured_drivers,
      _rust_config,
    } = config.deref();

    // const runners = ffiArray(configured_drivers, number_of_configured_drivers);
    configured_drivers.length = number_of_configured_drivers;

    let cr = {};
    for (let i = 0; i < configured_drivers.length; i++) {
      cr[configured_drivers[i].configuration_name] = {
        _runner: configured_drivers[i]._runner,
        database: configured_drivers[i].database,
        index: configured_drivers[i].index,
        databaseNumber: configured_drivers[i].database_number,
        ipOrHostname: configured_drivers[i].ip_or_hostname,
        port: configured_drivers[i].port,
        username: configured_drivers[i].username,
        password: configured_drivers[i].password,
      };
    }

    const result = {
      migrationsDirectory: migrations_directory,
      configuredRunners: cr,
    };

    // This property holds a pointer to the Rust object containing
    // the Mitre config object. The Rust ABI isn't stable, and Node.js
    // doesn't need access to this pointer, but we do need to hold it
    // to use for subsequent calls to diff(), migrate(), down(), etc.
    Object.defineProperty(result, "_mitre_rust_config", {
      configurable: false,
      enumerable: false,
      value: _rust_config,
      writable: false,
    });

    return result;
  },

  diff: (config) => {
    if (!config) {
      throw new Error("diff expects a config");
    }
    const { migration_states, num_migration_states } = libmitre
      .diff(config._mitre_rust_config)
      .deref();
    const migrationStates = []; // lol const
    migration_states.length = num_migration_states;
    for (let i = 0; i < migration_states.length; i++) {
      let migration = migration_states[i].migration.deref();
      migration.steps.length = migration.num_steps;
      let migration_steps = migration.steps;
      migration_states[migration] = migration_states[i].state;
      const steps = [];
      for (let j = 0; j < migration.num_steps; j++) {
        steps.push({
          [migration_steps[j].direction]: {
            path: migration_steps[j].path,
            source: migration_steps[j].source,
          },
        }); // technique is called ComputedPropertyNames
      }
      migrationStates.push({
        state: migration_states[i].state,
        migration: { steps },
      });
    }
    // Returns an array of objecs with {state: ..., migration: ... } where each
    // migration has a [{direction: step}] tuples, kinda in JavaScript's own
    // special way.
    return migrationStates;
  },
};

mitre.initLogging();

module.exports = mitre;
module.exports.default = mitre;
