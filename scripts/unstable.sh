# Use this to make cargo-shuttle target the unstable (staging) env.
#
# Usage:
#     source scripts/unstable.sh

export SHUTTLE_API="https://api.unstable.shuttle.rs"
unset SHUTTLE_API_KEY
unset SHUTTLE_BETA
export PS1="(shuttle: unstable) $(echo $PS1 | sed -e "s/(shuttle: .*) //")"
