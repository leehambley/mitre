var ffi = require("ffi-napi");
var ref = require("ref-napi");
var Struct = require("ref-struct-di")(ref);
var Array = require("ref-array-di")(ref);

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

const RunnerConfigPtr = ref.refType(RunnerConfig);

const Configuration = Struct({
  migrations_directory: ref.types.CString,
  configured_runners: Array(RunnerConfig),
  number_of_configured_runners: ref.types.size_t,
});

const ConfigurationPtr = ref.refType(Configuration);

var libmitre = ffi.Library("./target/debug/libmitre", {
  init_logging: ["int", []],
  config_from_file: [ConfigurationPtr, [ref.types.CString]],
});

// https://github.com/search?q=ffi.Library&type=Code&l=JavaScript
const mitre = {
  parseConfig: (path) => {
    // NOTE String may not be longer than
    // https://doc.rust-lang.org/std/primitive.isize.html#associatedconstant.MAX
    const config = libmitre.config_from_file(path);

    let {
      migrations_directory,
      configured_runners,
      number_of_configured_runners,
    } = config.deref();

    configured_runners.length = number_of_configured_runners

    let cr = {};
    for (let i = 0; i < configured_runners.length; i++) {
      console.log({ i, cri: configured_runners[i]._runner });
    } 
    // cr[runner_one.configuration_name] = {
    //   _runner: runner_one._runner,
    //   database: runner_one.database,
    //   index: runner_one.index,
    //   database_number: runner_one.database_number,
    //   ip_or_hostname: runner_one.ip_or_hostname,
    //   port: runner_one.port,
    //   username: runner_one.username,
    //   password: runner_one.password,
    // };

    // Assume we need something like
    // ffi.Pointer(  )
    // new RunnerConfig(configured_runners.address + 1 * configured_runners.byteLength)

    return {
      migrationsDirectory: migrations_directory,
      configuredRunners: cr,
      numConfiguredRunners: number_of_configured_runners,

      _config_ref_against_gc: config
    };
  },
  diff: (config) => {},
};

libmitre.init_logging();

module.exports = mitre;
