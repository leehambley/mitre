var ffi = require("ffi-napi");
var ref = require("ref-napi");
var StructType = require("ref-struct-di")(ref);
var ArrayType = require("ref-array-di")(ref);

const RunnerConfig = StructType({
  configuration_name: "string",
  _runner: "string",
  database: "string",
  index: "string",
  database_number: "string",
  ip_or_hostname: "string",
  port: "int",
  username: "string",
  password: "string",
});
const Configuration = StructType({
  migrations_directory: "string",
  configured_runners: ArrayType(RunnerConfig),
  number_of_configured_runners: "int",
});

var libmitre = ffi.Library("./target/debug/libmitre.dylib", {
  config_from_file: [Configuration, ["string"]],
});

const mitre = {
  parseConfig: (path) => {
    return libmitre.config_from_file(path);
  },
  diff: (config) => {},
};

module.exports = mitre;

// https://www.reddit.com/r/rust/comments/mkmxi9/

// https://github.com/infinyon/node-bindgen

// https://jvns.ca/blog/2017/12/21/bindgen-is-awesome/

// http://jakegoulding.com/rust-ffi-omnibus/tuples/
