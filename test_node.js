var ffi = require("ffi-napi");
var ref = require("ref-napi");
var StructType = require("ref-struct-di")(ref);
var ArrayType = require("ref-array-di")(ref);

var libmitre = ffi.Library("./target/debug/libmitre.dylib", {
  set_report_status_callback: ["void", ["pointer"]],
});

var callback = ffi.Callback("void", ["string"], function (msg) {
  console.log("This came from Rust?!?!?!: ", msg);
});

console.log("registering the callback");

libmitre.set_report_status_callback(callback);
