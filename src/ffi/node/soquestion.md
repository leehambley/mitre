I am trying to return a struct from a Rust function to Node.js, the struct is nested, and contains an array so it is sufficiently complicated that I want to do the allocation in Rust and have Node.JS receive a complete object, here's the Rust code in short:

``` rust
use std::ffi::CStr;
use std::os::raw::c_char;

#[derive(Debug)]
#[repr(C)]
pub struct Config {
    pub some_fields_here: String,
    // pub ...
}

#[derive(Debug)]
#[repr(C)]
pub struct State {
    pub message: String, // Even changing this to *mut c_char doesn't help the immediate problem
    pub configs: *mut RunnerConfiguration,
    pub num_configs: usize,
}

#[no_mangle]
pub extern "C" fn config_from_file(_not_really_used_in_example: *const c_char) -> *mut Configuration {
    let mut configs: Vec<Config> = vec![];
    configs.push(Config {
      some_fields_here: String::from("hello world"), // should maybe be CString::from(...).into_raw()
    });
    Box::into_raw(Box::new(State {
        message: String::from("a message here"),
        configs: Box::into_raw(configs.into_boxed_slice()) as *mut Config,
        num_configs: 1,
    }))
}
```

From the Node side, all the examples and docs we have found only make use of the `StructType` for preparing something to pass into the FFI, or to transparently pass around without ever interrogating it.

What we'd like to do is this:

``` javascript
var ffi = require("ffi-napi");
var ref = require("ref-napi");
var StructType = require("ref-struct-di")(ref);
var ArrayType = require("ref-array-di")(ref);

const Config = StructType({
  some_fields_here: ref.types.CString,
});

const State = StructType({
  message: ref.types.CString,
  configs: ref.refType(ArrayType(Config)),
  num_configs: ref.types.size_t,
});

const StatePtr = ref.refType(StatePtr);

var ourlib = ffi.Library("./target/debug/ourlib", {
  config_from_file: [StatePtr, [ref.types.CString]],
});

const ffiResult = ourlib.config_from_file("a file path, in the real code");
console.log(ffiResult)
// => <Buffer@0x47ce560 20 e2 83 04 00 00 00 00 57 00 00 00 00 00 00 00 57 00 00 00 00 00 00 00, type: { [Function: StructType] defineProperty: [Function: defineProperty], toString: [Function: toString], fields: { message: [Object], configs: [Object], num_configs: [Object] }, size: 24, alignment: 8, indirection: 1, isPacked: false, get: [Function: get], set: [Function: set] }>
```

Here, we need to get a Javscript object something like the struct above, access
to any fields, or see it as an object. We didn't find appropriate examples in the tests or documentation of https://tootallnate.github.io/ref, nor in https://github.com/node-ffi-napi/ref-struct-di

I don't expect to have a clean pass-thru from the C objects backed by `Buffer()` into JavaScript, of course we have to decode and move around things, but I cannot find *any* way to access the information contained within that struct from JavaScript. No combination of any `deref()` or `toObject()` or getting individual fields seems to help at all.

I expected that something like this might do it, but it either/or seg faults or prints garbage depending on some minor tweaks I make one way or another:

```
console.log(ffiResult);
console.log(ref.deref().readCString(ffiResult, 0));
```