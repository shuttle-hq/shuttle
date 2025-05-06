export SHUTTLE_API="https://api.dev2.shuttle.dev"
unset SHUTTLE_API_KEY
export PS1="(shuttle: Dev2) $(echo $PS1 | sed -e "s/(shuttle: .*) //")"
