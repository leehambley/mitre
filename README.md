# Mitre

Mitre is a cross-platform multi-purpose tool for running data and structural
migrations on a variety of databases, data stores and similar.

It is heavily inspired by the Rails migration system, with a directory of
migrations, the filenames prefixed with a timestamp which should be run once,
and only once per environment.

Mitre extends this concept with orthogonal naming of the migration files
(`.curl`, `.sql`, `.pgsql`, etc) which are used to look-up the corresponding
runner engine and configuration.

## Prior Art

You may be familiar with the general concept of migrations from frameworks such as Rails (ActiveRecord), Entity Framework Core, Liquibase, FlywayDB, Phinx, typeorm and others.

Mitre is a cross-platform (stand-alone CLI tool, and bindings for Ruby, Node.js) tool to bring the concept in a more portable, more flexible way to ecosystems which may not otherwise overlap.

## Pronunciation

/ˈmaɪtə(r)/, or "my-tuh". In wood-working a mitre is typically a 90⁰ precision cut for joining two pieces. The name feels appropriate for precision work in your database too.

## Filename Anatomy

```
./config/example.yml
./a/b/c/202030303033_create_table.mydb.sql
 ^^^^^^ ➊ 
        ^^^^^^^^^^^^ ➋
                     ^^^^^^^^^^^^ ➌
                                  ^^^^ ➍
                                       ^^^ ➎
```

1. Arbitrary nesting, ideal for composing projects using your SCM's submodule concept.
2. Ordinal integers (datetime stamps) to help run migrations in order. UTC is assumed.
3. Helpful name to give a clue what the migration does.
4. A configuration name, something specifying connection params in the configuration.
5. A runner extension name hint, for example `.sql` expects a configuration for either PostgreSQL, MariaDB or MySQL in the configuration with the name `mydb` and `mydb._driver = "mysql"`.

## Example Configuration & Directory Structure

```
$ cat config.yml
---
migrations_directory: "."
appdb: &mitre
  _driver: "mysql"
  database: "my-awesome-app"
  ip_or_hostname: 127.0.0.1
  port: 3306
  username: "myawesome"
  password: "example"
searchdb:
  _driver: "elasticsearch"
  ip_or_hostname: 127.0.0.1
  port: 9200
  index_name: "my-awesome-app"

mitre:
  <<: *mitre
  database: "mitre" # optional
```

This configuration defines two application databases, and the **required** configuration for `mitre` itself. A core design decision of Mitre is flexibility, so overwriting the name `database` of the mitre configuration would create and maintain the migrations state tables in a database called `mitre` on the same server as `appdb`.

Mitre can run migrations against ElasticSearch, but it cannot store state there, so across the two application database configurations migrations can be applied in both, and the results will be stored in the (shared) configuration `mitre`.

```
$ tree
./
./config.yml
./migrations/
    \- ./202004102151_create_index.searchdb.curl
    \- ./202006102151_update_index_mapping.searchdb.es
    \- ./202030303033_create_some_table.appdb.sql
    \- ./202020202020_modify_some_data.appdb/
              \- up.sql
              \- down.sql
./some-submodule-of-my-project/
    \- ./202030303033_do_some_migration_with_our_data_models.data.appdb.sql
  ...
```

Single files are considered to be "change" migrations, irreversible, and simply applied one-way. Directories with an `up` or `down` file are expected both to be runnable by the same runner defined in their configuration (i.e `.sql` is an allowed extension of the `mysql` specified between the `.appdb` suffix on the directory name, and the `_driver: "mysql` in the configuration.). Migrations are searched in the entire project directory thanks to the `migrations_directory` in the configuration. This allows composition with sub-modules for deploying microliths.

The anatomy of the file and directory names is specified above.

It is vitally important to understand the relationship between the ends of filenames such as `.data.appdb.sql` which can be read as:

- This is a data migration (a kind of tag, applications may permit booting with data migrations un-applied).
- This is migration uses the `appdb` configuration which knows how to handle `.sql` files.

Whether `appdb` is MySQL, MySQL, PostgreSQL or something else, is defined by the `_driver` in the config.

## Bidirectional migrations

Mitre supports separate up-and-down migrations, by replacing the following with
a directory, and two scripts, e.g :

```
202030303033_do_some_migration_with_our_data_models.someconf.rails
...
```

becomes:

```
rails/
  202030303033_do_some_migration_with_our_data_models.someconf/
    up.rails
    down.rails
  ...
```

## Templating

Migration files are passed once through the Mustache library which grants access
to the configuration and some handful of useful variables. This can be useful for
doing runtime reflection.  Mustache was selected rather than liquid, or similar 
because it is so limited, and is essentially interpolation without too much magic, 
migrations probably shouldn't be Turing-complete.

## Submodule friendliness

The migration directory is allowed to be nested, all files across all
directories within the migration directory will be evaluated after "flattening"
them and associating the relevant configuration.

This allows maintaining a Mitre set-up with migrations from a number of
projects to create a kind of meta-repository that contains the migrations from
a number of projects together.

## CLI Usage

### Table Printing

When running on a supported terminal, commands such as `mitre ls` will pretty-print
a table using unicode's box drawing characters. You can get a simple text delimited
output by passing the output through a pipe (so that stdout is not a tty).

If you need to program against the textual output of the Mitre CLI please consider 
very carefully because it's not even a little bit guaranteed to be stable.

You can open an issue against the repostory to suggest some `-0` flag or something we
can begin to accept which would provide null-byte delimted output, or some CSV flavor
or something.

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

- the (last) file extension indicates how to run the script

- a .curl extension indicates that the file in question contains params to pass
to an invocation of curl, with connection params as described in the
elasticsearch.yml

- across all directories things run in time order

- ~~configuration has a concept of environments, so each of those .yml files has
a `development`, `production` or whatever inside, heavily rails inspired~~

- You could easily do something like .risky.curl to indicate that this
migration is risky, and the default mode is maybe not to run risky migrations
:shrug: but you could force that

- You could support up/down migrations by making a directory
`10101010101_something_reversible.someconf/{up/down}.sql`

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

## Blog ideas

- why not having structure.<ext> like Rails
- why no separate "seeds" (hint: flags)

## TODO:

- Ensure the migrations all have a matching runner defined.
- Warn on unused config for runner.
- Err on on runner with no config.
- Warn on runner with no config when already run.
- https://github.com/rails/rails/blob/161fc87fbf8d0efee2cadc11199fbb4d183ce712/railties/lib/rails/tasks/engine.rake make Rails compatible Gem drop-in

## Contributing

Please install [pre-commit](https://pre-commit.com/#install) and run `pre-commit install` in this repo to install the commit checks.

### Testing

1. Start the test databases with `docker-compose up -d` (on Silicon run `docker-compose up -f docker-compose.silicon.yml -d`)
2. Run `cargo test`
