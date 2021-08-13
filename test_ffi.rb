require "ffi"
require "pry"

# https://stackoverflow.com/a/53896817/119669

module Mitre

  class RunnerConfiguration < FFI::Struct
    layout :configuration_name, :string,
           :_runner, :string,
           :database, :string,
           :index, :string,
           :database_number, :uint8,
           :ip_or_hostname, :string,
           :port, :uint16, # TODO: check me?
           :username, :string,
           :password, :string
  end
  
  class Configuration < FFI::Struct
    layout :migrations_directory,:string,
           :configured_runner, :pointer,
           :number_of_configured_drivers, :uint8
  end

  extend FFI::Library

  ffi_lib begin
    prefix = Gem.win_platform? ? "" : "lib"
    "#{File.expand_path("./target/debug/", __dir__)}/#{prefix}mitre.#{FFI::Platform::LIBSUFFIX}"
  end

  attach_function :init_logging, [ ], :void
  attach_function :config_from_file, [:string], :void
  
end

Mitre::init_logging()
pewpew = "/home/leehambley/code/mitre/test/fixtures/example-1-simple-mixed-migrations/mitre.yml"

# Simple case, just one RW.
# TODO: `[:kind]` shows :reason not kind, and accessing reason segfaults.. off by one ?
# rw = Mitre::ReservedWord.new(Mitre.reserved_word())

# Yikes, arrays of pointers to structs
# https://github.com/ffi/ffi/wiki/structs#array-of-structs
ffiResult = Mitre.config_from_file(pewpew)
binding.pry
rws = Mitre::Configuration.new(ffiResult)
puts rws
puts rws[:migrations_directory]
# print_rw(conf)

# puts "cleaning up"
# Mitre::free_reserved_words(rws)