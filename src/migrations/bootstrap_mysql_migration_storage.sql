-- Bootstrap the database
CREATE DATABASE IF NOT EXISTS `{{database_name}}` CHARACTER SET utf8 COLLATE utf8_bin;

-- Create the migration storage table (migrations)
CREATE TABLE IF NOT EXISTS `{{database_name}}`.`{{migrations_table}}` (

  -- TIMESTMAP is YYYYMMDDHHMMSS just like migration filenames
  -- assumed to be UTC, and stored as such.
  -- BIGINT(14) is just a rendering hint, not a real qualifier
  `version` BIGINT(14) NOT NULL PRIMARY KEY,

  -- Not exactly a property of a migration, but metadata
  -- stored when we store a migration in here via the MigrationStorage
  -- trait.
  --
  -- Note no default here, using a UTC default *requires* MySQL 8.0.13
  -- or above, or MariaDB, let's just always provide it from Mitre
  -- https://dev.mysql.com/doc/refman/8.0/en/data-type-defaults.html#data-types-defaults-explicit
  `stored_at` DATETIME NOT NULL,

  -- Flags e.g `sorted,comma,separated,nospaces`
  `flags` TINYTEXT NOT NULL,

  -- Runner Configuration Name (key in the YAML)
  `configuration_name` TINYTEXT NOT NULL,

  -- Was this a built-in migration
  `built_in` BOOLEAN NOT NULL

) ENGINE=InnoDB;
-- ENGINE=InnoDB is the default, but let's be explicit.

-- Create the migration storage table (steps)
CREATE TABLE IF NOT EXISTS `{{database_name}}`.`{{migration_steps_table}}` (

  -- Version must match `mitre_migration_steps`'s column of the 
  -- same name
  `version` BIGINT(14) NOT NULL,

  -- Direction
  `direction` ENUM('up', 'down', 'change') NOT NULL,

  -- Source has a size limit of about 16MiB on most MySQL compatible systems
  -- Empty string is not permitted to prevent stub migration parts being stored
  -- by mistake
  `source` MEDIUMBLOB NOT NULL CHECK (`source` <> ''),

  -- The *relative* path to the migrations. Please take every care to ensure
  -- that nothing platform|user|environment specific shows up here.
  `path` BLOB NOT NULL CHECK (`path` <> ''), 

  PRIMARY KEY (`version`, `direction`),

  CONSTRAINT `fk_migration_version`
    FOREIGN KEY (`version`) REFERENCES `{{database_name}}`.`{{migrations_table}}` (`version`)

) ENGINE=InnoDB;
-- ENGINE=InnoDB is the default, but let's be explicit.