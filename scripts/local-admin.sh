# Use this to insert an admin API key in your local stack and
# set that key to be used with cargo-shuttle requests
#
# Usage:
#     source scripts/local-admin.sh

key="dh9z58jttoes3qvt" # arbitrary test key
export SHUTTLE_API_KEY=$key
export SHUTTLE_API="http://localhost:8001"
export PS1="(shuttle: local admin key) $(echo $PS1 | sed -e "s/(shuttle: .*) //")"

docker compose --file docker-compose.rendered.yml --project-name shuttle-dev exec auth /usr/local/bin/shuttle-auth --db-connection-uri=postgres://postgres:postgres@control-db init-admin --user-id admin --key $key
