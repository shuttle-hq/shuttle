# Use this to make cargo-shuttle target the production env.
# Useful when running cargo-shuttle in debug mode, since that targets the local stack by default.
#
# Usage:
#     source scripts/production.sh

export SHUTTLE_API="https://api.shuttle.rs"
