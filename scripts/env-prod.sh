export SHUTTLE_API="https://api.shuttle.dev"
unset SHUTTLE_API_KEY
export PS1="(shuttle: Production) $(echo $PS1 | sed -e "s/(shuttle: .*) //")"
