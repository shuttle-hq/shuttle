#!/usr/bin/env bash

/usr/bin/python3 /usr/sbin/watch &

exec /bin/bash /usr/local/bin/docker-entrypoint.sh "$@"
