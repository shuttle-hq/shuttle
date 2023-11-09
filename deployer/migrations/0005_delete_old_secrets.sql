-- Delete any locally cached secrets that remain from old deployer versions
DROP TABLE IF EXISTS secrets;
DELETE FROM resources WHERE type = "secrets";
