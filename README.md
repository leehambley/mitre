# Mitre

Mitre is a cross-platform multi-purpose tool for running data and structural
migrations on a variety of databases, data stores and similar.

It is heavily inspired by the Rails migration system, with a directory of
migrations, the filenames prefixed with a timestamp which should be run once,
and only once per environment.

Mitre extends this concept with orthogonal naming of the migration files
(`.curl`, `.sql`, `.pgsql`, etc) which are used to look-up the corresponding
runner engine and configuration.

## Filename Anatomy

```
./config/example.yml
./anylevelofnesting/example/202030303033_do_some_migration_with_our_data_models.
rails

^^^^^ runner type, if the runner

      needs configuration, this

      must be provided in

      ./config/example.yml

                                         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
arbitrary name for

human readability

                            ^^^^^^^^^^^^ Datestamp for ordering across all
files, is
                                         used to determine run-order for all
migrations

                    ^^^^^^^ Uses the configuration in config/example.yml which
may
                            contain configuration for multiple runners

  ^^^^^^^^^^^^^^^^^ Arbitrary, may also just be ./ - useful if you compose
                    your mitre migrations directory from many Git repositories
                    (see below for example)

```

## Example Directory Structure

For example:

```
config/
  ./elasticsearch.yml
  ./postgres.yml
  ./redis.yml
elasticsearch/
  202004102151_create_index.curl
  202006102151_update_index_mapping.curl
postgres/
  202030303033_create_some_table.sql
  202020202020_modify_some_data/
    up.sql
    down.curl
my-project/
  202030303033_do_some_migration_with_our_data_models.rails
  ...
```

In this example `.rails` is executed as a Ruby script using the `bin/rails
runner` as an entrypoint using the configuration from `./config/my-project.yml`.

Various file extensions carry special meanings; `.curl` files are expected to
contain command line flags to complete a `curl ...` command, e.g:

```
# cat 202004102151_create_index.curl
-X POST -d '{...giant data thing here...}'
```

## Bidirectional migrations

Mitre supports separate up-and-down migrations, by replacing the following with
a directory, and two scripts, e.g :

```
rails/
  202030303033_do_some_migration_with_our_data_models.rails
  ...
```

becomes:

```
rails/
  202030303033_do_some_migration_with_our_data_models/
    up.rails
    down.rails
  ...
```

## Templating

Migration files are passed once through the Handlebars library which grants access
to the configuration and some handful of useful variables. This can be useful for
doing runtime reflection.  Handlebars was selected rather than liquid, or similar 
because it is so limited, and is essentially interpolation without too much magic, 
migrations probably shouldn't be Turing-complete.

## Submodule friendliness

The migration directory is allowed to be nested, all files across all
directories within the migration directory will be evaluated after "flattening"
them and associating the relevant configuration.

This allows maintaining a Mitre set-up with migrations from a number of
projects to create a kind of meta-repository that contains the migrations from
a number of projects together.

## Remembering which migrations ran

Mitre tries to remember which migrations have been run, in the case of curl, or
http migrations which are fire-and-forget it's impossible for mitre to store
the result, and avoid firing that migration again.

To resolve that it is required to specify at least one configuration for a
store which is persistent. In case more than one store is available (e.g two
MySQL configurations, or one each MySQL and PostgreSQL) mitre will require that
one is configured as the store for which migrations have and haven't run.

## Tags

Files can be tagged with arbitrary arbitrary flags in the filename. Any dot
separated parts immediately before one of the supported runners will be treated
as a flag. `<timestamp>_name.foo.bar.baz.curl`.

To see how mitre identifies tags in any filename run:

    mitre extract-tags ./path/to.the.file

By default `data`, `risky` and `long` migrations (or combinations including
those tags) are not run.

### Reserved words

Some words cannot be used as a tag, they are used, or are reserved for use for
runners. Examples include `curl`, `rails`, `bash`, `sh` and more. For a
complete list run:

    mitre list-reserved-words

## Other things to know

- config contains config files that correlate with the directories
elasticsearch.yml correlates to 'elasticsearch/'

- the file extension indicates how to run the script

- a .curl extension indicates that the file in question contains params to pass
to an invocation of curl, with connection params as described in the
elasticsearch.yml

- across all directories things run in time order

- configuration has a concept of environments, so each of those .yml files has
a `development`, `prodctuon` or whatever inside, heavily rails inspired

- You could easily do something like .risky.curl to indicate that this
migration is risky, and the default mode is maybe not to run risky migrations
:shrug: but you could force that

- You could support up/down migrations by making a directory
`10101010101_something_reversible/{up/down}.sql`

## The Trouble With Rails Migrations

- At some level of maturity, and table size using the ActiveRecord DSL for
changes is risky, you might want to use a tool such as Percona.

- Being unable to boot the app if there are outstanding migrations is a
constant source of annoyance. Maybe the un-run migrations don't affect the part
of the code you are trying to test.

- Rails' promise of database agnosticity doesn't hold at all, so using the
ActiveRecord migration DSL to define your SQL statements in Ruby is a weak
abstraction if you want to use triggers, functions or custom types (e.g in
PostgreSQL, but also in MySQL in advanced configurations).

- Rails migrations are often used for data migrations, for better or worse, you
may or may not want to run them at deploy time, or later at night, similarly
with adding indexes to databases which may/not need to be run at deploy time.

- The entire concept of deploy-time is weird with containerized applications
and autoscaling.

## TODO:

- Ensure the migrations all have a matching runner defined.
- Warn on unused config for runner.
- Err on on runner with no config.
- Warn on runner with no config when already run.
- https://github.com/rails/rails/blob/161fc87fbf8d0efee2cadc11199fbb4d183ce712/railties/lib/rails/tasks/engine.rake make Rails compatible Gem drop-in
