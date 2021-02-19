#!/usr/bin/env ./test/libs/bats/bin/bats
load 'libs/bats-support/load'
load 'libs/bats-assert/load'

@test "requires CI_COMMIT_REF_SLUG environment variable" {
  assert_empty ""
}
