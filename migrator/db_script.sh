#!/usr/bin/env sh


ssh ubuntu@18.133.52.140 "docker exec pg pg_dumpall -U postgres > dump.sql"
scp ubuntu@18.133.52.140:~/dump.sql dump.sql

scp dump.sql database.shuttle.internal:~/dump.sql

# docker cp dump.sql 123:/dump.sql
# docker exec 123 psql -f dump.sql -U postgres
