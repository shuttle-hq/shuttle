export SHUTTLE_API="https://api.staging.shuttle.dev"
unset SHUTTLE_API_KEY
export PS1="(shuttle: Staging) $(echo $PS1 | sed -e "s/(shuttle: .*) //")"
