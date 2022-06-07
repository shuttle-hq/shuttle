#!/usr/bin/env bash

if [ -z $PROXY_FQDN ]
then
    echo "The variable 'PROXY_FQDN' is missing"
    exit 1
fi

if [ -z $PROVISIONER_ADDRESS ]
then
    echo "The variable 'PROVISIONER_ADDRESS' is missing"
    exit 1
fi

export CRATES_PATH=${CRATES_PATH:-/var/lib/shuttle/crates}

mkdir -p $CRATES_PATH

export PROXY_PORT=${PROXY_PORT:-8000}

export API_PORT=${API_PORT:-8001}

if [[ ! -z "${SHUTTLE_USERS_TOML}" && ! -s "${SHUTTLE_USERS_TOML}" ]]
then
    if [[ -z "${SHUTTLE_INITIAL_KEY}" ]]
    then
        echo "\$SHUTTLE_INITIAL_KEY is not set to create initial user's key"
        exit 1
    fi

    echo "Creating a first user with key '${SHUTTLE_INITIAL_KEY}' at '${SHUTTLE_USERS_TOML}'"
    mkdir -p $(dirname "${SHUTTLE_USERS_TOML}")
    echo -e "[$SHUTTLE_INITIAL_KEY]\nname = 'first-user'\nprojects = []" > "${SHUTTLE_USERS_TOML}"
fi

exec supervisord -n -c /usr/share/supervisord/supervisord.conf
