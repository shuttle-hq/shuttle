# Use this to make cargo-shuttle target the unstable (staging) env.
#
# Usage:
#     source scripts/unstable.sh

export SHUTTLE_API="https://api.unstable.shuttle.rs"
unset SHUTTLE_API_KEY
export PS1="(shuttle: unstable) $PS1"
