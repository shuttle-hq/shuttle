#! /usr/bin/env sh

set -ue

# Prepare directory
mkdir -p /tmp/qa-$1
cd /tmp/qa-$1

# Init app
cargo shuttle init --name qa-$1 --axum

# Start locally
cargo shuttle run &
sleep 150

echo "Testing local hello endpoint"
output=$(curl --silent localhost:8000/hello)
[ "$output" != "Hello, world!" ] && ( echo "Did not expect output: $output"; exit 1 )

killall cargo-shuttle

cargo shuttle project start

cargo shuttle deploy --allow-dirty

echo "Testing remote hello endpoint"
output=$(curl --silent https://qa-$1.unstable.shuttleapp.rs/hello)
[ "$output" != "Hello, world!" ] && ( echo "Did not expect output: $output"; exit 1 )

cargo shuttle project stop

exit 0
