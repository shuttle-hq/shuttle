#! /usr/bin/env sh

set -ue

# cd into the Docker example
cd examples/rocket/postgres

# Start locally
cargo shuttle run &
sleep 300

echo "Testing local docker endpoint"
output=$(curl --silent --request POST --header "Content-Type: application/json" --data '{"note": "test"}' localhost:8000/todo)
[ "$output" != '{"id":1,"note":"test"}' ] && ( echo "Did not expect POST output: $output"; exit 1 )

output=$(curl --silent localhost:8000/todo/1)
[ "$output" != '{"id":1,"note":"test"}' ] && ( echo "Did not expect output: $output"; exit 1 )

killall cargo-shuttle
