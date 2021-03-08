var ffi = require("ffi-napi");
var ref = require("ref-napi");
var winston = require("winston");

const logger = winston.createLogger({
  transports: [new winston.transports.Console()],
});
var StructType = require("ref-struct-di")(ref);
var ArrayType = require("ref-array-di")(ref);

var libmitre = ffi.Library("./target/debug/libmitre.dylib", {
  set_logger_fn: ["void", ["pointer"]],
  do_work: ["void", []],
});

var callback = ffi.Callback(
  "void",
  ["string", "string"],
  function (level, message) {
    console.log(message);
    winston.log({ level, message });
  }
);

console.log("registering the callback");

libmitre.set_logger_fn(callback);
libmitre.do_work();
