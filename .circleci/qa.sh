#! /usr/bin/env sh

# Prepare directory
mkdir -p /tmp/qa-linux
cd /tmp/qa-linux

# Init app
cargo shuttle init --name qa-linux --axum

# Start locally
cargo shuttle run &
sleep 60

output=$(curl --silent localhost:8000/hello)
[ "$output" != "Hello, worl" ] && ( echo "Did not expect output: $output"; exit 1 )

exit 0
