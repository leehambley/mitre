var ffi = require("ffi-napi");
var ref = require("ref-napi");
var StructType = require("ref-struct-di")(ref);
var ArrayType = require("ref-array-di")(ref);

// define the "timeval" struct type
var ReservedWord = StructType({
  word: ref.refType("string"),
  reason: ref.refType("string"),
  kind: ref.refType("string"),
});

var ReservedWords = StructType({
  lenn: ref.refType("uint8"),
  words: ref.refType("pointer"),
});

var libmitre = ffi.Library("./target/debug/libmitre.so", {
  reserved_words: [ReservedWords, []],
  free_reserved_words: ["void", [ReservedWords]],
});

debugger;

console.log(ReservedWords.size);

console.log(libmitre.reserved_words().lenn);
