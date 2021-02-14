-- Table name must agree with the constant in the mariadb.rs 
CREATE TABLE {{mariadb_migration_state_table_name}} (

  -- TIMESTMAP is YYYYMMDDHHMMSS just like migration filenames
  `version` TIMESTAMP NOT NULL PRIMARY KEY,

  -- These columns store the up/down/change migrations, so we
  -- know what was applied and can handle turning the database
  -- back in a branch, for e.g by running "downs"
  --
  -- Backticks everywhere for consistency, required here 
  -- because `change` is a reserved word.
  `up` BLOB,
  `down` BLOB,
  `change` BLOB,

  -- Simple metadata columns
  `applied_at` TIMESTAMP NOT NULL,
  `apply_time_sec` BIGINT UNSIGNED NOT NULL,

  -- Environment
  `environment` TINYTEXT NOT NULL

) ENGINE=InnoDB;
-- ENGINE=InnoDB is the default, but let's be explicit.
