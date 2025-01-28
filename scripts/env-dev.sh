export SHUTTLE_API="https://api.dev.shuttle.dev"
unset SHUTTLE_API_KEY
export PS1="(shuttle: Dev) $(echo $PS1 | sed -e "s/(shuttle: .*) //")"
