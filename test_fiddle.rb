#!/bin/env ruby
#
require "fiddle"
require "fiddle/import"

module Mitre
  extend Fiddle::Importer

  ReservedWord = struct [
    'char *word',
    'char *reason',
    'char *kind'
  ]

  ReservedWords = struct [
    'size_t len',
    'char *anything',
  ]

  dlload "./target/release/libmitre.so"

  reserved_word = Fiddle::Function.new(handler['Reserved_Word'], [], Fiddle::TYPE_INT)
  reserved_words = Fiddle::Function.new(handler['reserved_words'], [], Fiddle::TYPE_INT)

end

rw = Mitre::ReservedWords.new(Mitre::reserved_word.call())

puts rw.word