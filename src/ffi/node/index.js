var ffi = require("ffi-napi");
var ref = require("ref-napi");
var Struct = require("ref-struct-di")(ref);
var Array = require("ref-array-di")(ref);

const LogCallbacks = Struct({
  // https://github.com/node-ffi-napi/ref-struct-di/blob/master/test/struct.js#L57
  trace: ffi.Function("void", [ref.types.CString]),
  debug: ffi.Function("void", [ref.types.CString]),
});

const LogCallbacksPtr = ref.refType(LogCallbacks);

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
});

const ConfigurationPtr = ref.refType(Configuration);

var libmitre = ffi.Library("./target/debug/libmitre", {
  init_logger: ["void", [LogCallbacksPtr]],
  config_from_file: [ConfigurationPtr, [ref.types.CString]],
});

const fnTrace = function (msg) {
  console.log("trace: fn" + msg);
};

const fnDebug = function (msg) {
  console.log("debug: fn" + msg);
};

// https://github.com/search?q=ffi.Library&type=Code&l=JavaScript
const mitre = {
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
    } = config.deref();

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
    return {
      migrationsDirectory: migrations_directory,
      configuredRunners: cr,
      _config_ref_against_gc: config,
    };
  },
  diff: (config) => {},
};

mitre.initLogging();

module.exports = mitre;
