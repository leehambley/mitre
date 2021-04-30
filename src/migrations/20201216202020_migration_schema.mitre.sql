-- Bootstrap the database
CREATE DATABASE IF NOT EXISTS `{{migration_state_database_name}}` CHARACTER SET utf8 COLLATE utf8_bin;

-- Start a transaction to create both, or no tables..
START TRANSACTION;

-- Table name must agree with the constant in the mariadb.rs 
CREATE TABLE `{{migration_state_database_name}}`.`{{migration_state_table_name}}` (

  -- TIMESTMAP is YYYYMMDDHHMMSS just like migration filenames
  -- assumed to be UTC, and stored as such.
  -- BIGINT(14) is just a rendering hint, not a real qualifier
  `version` BIGINT(14) NOT NULL PRIMARY KEY,

  -- Not exactly a property of a migration, but metadata
  -- stored when we store a migration in here via the MigrationStorage
  -- trait.
  `stored_at` DATETIME NOT NULL DEFAULT UTC_TIMESTAMP(),

  -- Flags e.g `sorted,comma,separated,nospaces`
  `flags` TINYTEXT NOT NULL,

  -- Runner Configuration Name (key in the YAML)
  `configuration_name` TINYTEXT NOT NULL,

  -- Was this a built-in migration
  `built_in` BOOLEAN NOT NULL

) ENGINE=InnoDB;
-- ENGINE=InnoDB is the default, but let's be explicit.

CREATE TABLE `{{migration_state_database_name}}`.`{{migration_steps_table_name}}` (

  -- Version must match `{{migration_state_table_name}}`'s column of the 
  -- same name
  `version` BIGINT(14) NOT NULL,

  -- Direction
  `direction` ENUM('up', 'down', 'change') NOT NULL,

  -- Source has a size limit of about 16MiB on most MySQL compatible systems
  -- Empty string is not permitted to prevent stub migration parts being stored
  -- by mistake
  `source` MEDIUMBLOB NOT NULL CHECK (`source` <> ''),

  PRIMARY KEY (`version`, `direction`),

  CONSTRAINT `fk_migration_version`
    FOREIGN KEY (`version`) REFERENCES `{{migration_state_database_name}}`.`{{migration_steps_table_name}}` (`version`)

)

-- Finalize
COMMIT;