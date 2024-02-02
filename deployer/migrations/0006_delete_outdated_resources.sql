-- delete any old records of these so that they are not sent to r-r
DELETE FROM resources WHERE type = "static_folder";
DELETE FROM resources WHERE type = "turso";
DELETE FROM resources WHERE type = "metadata";
DELETE FROM resources WHERE type = "custom";
