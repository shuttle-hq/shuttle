#!/usr/bin/env bash

export PG_VERSION=${PG_VERSION:-11}

export PG_CLUSTER_NAME=${PG_CLUSTER_NAME:-shuttle}

export PG_DATA=${PG_DATA:-/var/lib/postgresql/$PG_VERSION/$PG_CLUSTER_NAME}

export PG_PORT=${PG_PORT:-5432}

export PG_PASSWORD=${PG_PASSWORD:-postgres}

if [[ "$(pg_lsclusters -h | wc -l)" -ne "1" ]]; then
    set -e
    pg_createcluster -d $PG_DATA $PG_VERSION $PG_CLUSTER_NAME

    conftool() {
        pg_conftool $PG_VERSION $PG_CLUSTER_NAME set $1 $2
    }
    conftool listen_addresses \"*\"
    conftool port $PG_PORT
    conftool log_statement all

    pg_ctlcluster $PG_VERSION $PG_CLUSTER_NAME start
    su postgres -c "psql -c \"ALTER USER postgres PASSWORD '${PG_PASSWORD}'\""
    pg_ctlcluster $PG_VERSION $PG_CLUSTER_NAME stop
    set +e

    network=$(ip route | awk '/scope/{print $1}')
    echo "host    all             all             $network            md5" >> /etc/postgresql/11/shuttle/pg_hba.conf
fi

export PG_LOG=$(pg_lsclusters -h | cut -d' ' -f7)

export PG_HOST=localhost

export PG_URI=postgres://postgres:${PG_PASSWORD}@localhost:${PG_PORT}/postgres

export PROVISIONER_ADDRESS=${PROVISIONER_ADDRESS:-provisioner}

exec supervisord -n -c /usr/share/supervisord/supervisord.conf
