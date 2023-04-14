#! /usr/bin/env sh

set -ue

# Prepare directory
mkdir -p /tmp/qa-linux
cd /tmp/qa-linux

# Init app
cargo shuttle init --name qa-linux --axum

# Start locally
cargo shuttle run &
sleep 60

echo "Testing local hello endpoint"
output=$(curl --silent localhost:8000/hello)
[ "$output" != "Hello, world!" ] && ( echo "Did not expect output: $output"; exit 1 )

killall cargo-shuttle

cargo shuttle project start

cargo shuttle deploy --allow-dirty

echo "Testing remote hello endpoint"
output=$(curl --silent qa-linux.unstable.shuttle.rs/hello)
[ "$output" != "Hello, world!" ] && ( echo "Did not expect output: $output"; exit 1 )

cargo shuttle project stop

exit 0
