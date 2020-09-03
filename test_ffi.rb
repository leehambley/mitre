require "ffi"
require "pry"

# https://stackoverflow.com/a/53896817/119669

module Mitre

  class ReservedWord < FFI::Struct
    layout :word, :string,
           :reason, :string,
           :kind, :string
  end
  
  class ReservedWords < FFI::Struct
    layout :len,  :uint8,
           :words, :pointer
  end

  extend FFI::Library

  ffi_lib begin
    prefix = Gem.win_platform? ? "" : "lib"
    "#{File.expand_path("./target/debug/", __dir__)}/#{prefix}mitre.#{FFI::Platform::LIBSUFFIX}"
  end

  attach_function :reserved_words, [ ], :pointer
  attach_function :free_reserved_words, [:pointer], :void
  
end

def print_rw(rw)
  puts "Word: #{rw[:word]} | Kind: #{rw[:kind]} | Reason: #{rw[:reason]}"
end

# Simple case, just one RW.
# TODO: `[:kind]` shows :reason not kind, and accessing reason segfaults.. off by one ?
# rw = Mitre::ReservedWord.new(Mitre.reserved_word())

# Yikes, arrays of pointers to structs
# https://github.com/ffi/ffi/wiki/structs#array-of-structs
rws = Mitre::ReservedWords.new(Mitre.reserved_words())
puts rws.to_ptr
puts "There are #{rws[:len]} reserved words"
0.upto(rws[:len]-1) do |i|
  rw = Mitre::ReservedWord.new(rws[:words] + (i * Mitre::ReservedWord.size))
  print_rw(rw)
end

puts "cleaning up"
Mitre::free_reserved_words(rws)