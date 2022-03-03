#!/usr/bin/env bash

export PG_VERSION=${PG_VERSION:-11}

export PG_CLUSTER_NAME=${PG_CLUSTER_NAME:-unveil}

export PG_DATA=${PG_DATA:-/var/lib/postgresql/$PG_VERSION/$PG_CLUSTER_NAME}

export PG_PORT=${PG_PORT:-5432}

if [ ! -f "$PG_DATA/PG_VERSION" ]; then
    # Assume the data directory is not setup
    set -e
    pg_createcluster -d $PG_DATA $PG_VERSION $PG_CLUSTER_NAME

    conftool() {
        pg_conftool $PG_VERSION $PG_CLUSTER_NAME set $1 $2
    }
    conftool port $PG_PORT
    conftool log_statement all
    pg_ctlcluster $PG_VERSION $PG_CLUSTER_NAME start
    set +e
fi

export PG_PASSWORD=${PG_PASSWORD:-postgres}

su postgres -c "psql -c \"ALTER USER postgres PASSWORD '${PG_PASSWORD}'\""

export PG_LOG=$(pg_lsclusters -h | cut -d' ' -f7)

export PG_HOST=localhost

export PG_URI=postgres://postgres:${PG_PASSWORD}@localhost:${PG_PORT}/postgres

export CRATES_PATH=${CRATES_PATH:-/var/lib/unveil/crates}

mkdir -p $CRATES_PATH

export PROXY_PORT=${PROXY_PORT:-8000}

export API_PORT=${API_PORT:-8001}

exec supervisord -n -c /usr/share/supervisord/supervisord.conf
