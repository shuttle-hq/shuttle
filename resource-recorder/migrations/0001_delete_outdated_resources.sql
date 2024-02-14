-- all other services should no longer be dealing with these
DELETE FROM resources WHERE type = "static_folder";
DELETE FROM resources WHERE type = "turso";
DELETE FROM resources WHERE type = "metadata";
DELETE FROM resources WHERE type = "custom";
