-- Bootstrap the database
CREATE DATABASE IF NOT EXISTS {{mariadb_migration_state_database_name}} CHARACTER SET utf8 COLLATE utf8_bin;

-- Use the newly created database
USE {{mariadb_migration_state_database_name}};

-- Table name must agree with the constant in the mariadb.rs 
CREATE TABLE {{mariadb_migration_state_table_name}} (

  -- TIMESTMAP is YYYYMMDDHHMMSS just like migration filenames
  -- assumed to be UTC, and stored as such.
  -- BIGINT(14) is just a rendering hint, not a real qualifier
  `version` BIGINT(14) NOT NULL PRIMARY KEY,

  -- These columns store the up/down/change migrations, so we
  -- know what was applied and can handle turning the database
  -- back in a branch, for e.g by running "downs" in environments
  -- that have had the code rolled-back, perhaps.
  --
  -- We don't store the parsed templating code, this is just here
  -- for re-runs and a semblence of independence from code lifecycle
  --
  -- Backticks everywhere for consistency, required here 
  -- because `change` is a reserved word.
  `up` BLOB,
  `down` BLOB,
  `change` BLOB,

  -- Simple metadata columns
  `applied_at_utc` TIMESTAMP NOT NULL,
  `apply_time_ms` BIGINT UNSIGNED NOT NULL,

  -- Was this a built-in migration
  `built_in` BOOLEAN NOT NULL,

  -- Environment
  `environment` TINYTEXT NOT NULL

) ENGINE=InnoDB;
-- ENGINE=InnoDB is the default, but let's be explicit.
