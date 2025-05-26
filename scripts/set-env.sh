# Usage: source scripts/set-env.sh [env]

if [ -z "$1" ]; then
    echo "provide an env name";
else
    export SHUTTLE_API_ENV="$1"
    unset SHUTTLE_API
    unset SHUTTLE_API_KEY
    export PS1="(shuttle: $SHUTTLE_API_ENV) $(echo $PS1 | sed -e "s/(shuttle: .*) //")"
fi
