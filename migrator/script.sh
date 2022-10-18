#!/usr/bin/env sh

scp ubuntu@18.133.52.140:/opt/shuttle/user-data/users/users.toml users.toml
cargo run -- users.toml > users.sql


scp users.sql controller.shuttle.internal:~/users.sql
ssh controller.shuttle.internal "cat ~/users.sql | sudo sqlite3 /var/lib/docker/volumes/shuttle-dev_gateway-vol/_data/gateway.sqlite"
