#!/usr/bin/env bash

# -e is not set so that we can check for errors ourselves
set -uo pipefail

# Anonymous telemetry
TELEMETRY="1"
PLATFORM=""
ARCH="$(uname -m)"
NEW_INSTALL="true"
INSTALL_METHOD=""
OUTCOME=""
STEP_FAILED="N/A"
STARTED_AT=""
if command -v date &>/dev/null; then
  STARTED_AT="$(date -u -Iseconds)"
fi
case "$OSTYPE" in
linux*) PLATFORM="linux" ;;
darwin*) PLATFORM="macos" ;;
*) PLATFORM="unknown" ;;
esac
BIN_ARCH="$ARCH"
if [ "$ARCH" = "arm64" ]; then
  BIN_ARCH="aarch64"
fi
INSTALL_DIR="$HOME/.local/bin"


# disable telemetry if any opt-out vars are set
if [[ \
    "${DO_NOT_TRACK:-}" == "1" || "${DO_NOT_TRACK:-}" == "true" || \
    "${DISABLE_TELEMETRY:-}" == "1" || "${DISABLE_TELEMETRY:-}" == "true" || \
    "${SHUTTLE_DISABLE_TELEMETRY:-}" == "1" || "${SHUTTLE_DISABLE_TELEMETRY:-}" == "true" || \
    "${CI:-}" == "1" || "${CI:-}" == "true"
  ]]; then
  TELEMETRY=0
fi

# default terminal on mac gives xterm-256color but still doesn't show colors
if [[ "${TERM:-}" = "xterm-256color" && "$PLATFORM" != "macos" ]]; then
  SUPPORTS_COLOR="1"
  # TODO: colored logo
else
  SUPPORTS_COLOR="0"
fi
echo "\
    ____                      __
   /  _/___ ___  ____  __  __/ /_______
   / // __ \`__ \\/ __ \\/ / / / / ___/ _ \\
 _/ // / / / / / /_/ / /_/ / (__  )  __/
/___/_/ /_/ /_/ .___/\\__,_/_/____/\\___/
             /_/              by Shuttle
"
echo "\
https://docs.shuttle.dev
https://discord.gg/shuttle
https://github.com/shuttle-hq/shuttle

Please open an issue if you encounter any problems!"
if [[ "$TELEMETRY" = "1" ]]; then
  [[ "$SUPPORTS_COLOR" = "1" ]] && echo -en "\e[2m"
  echo "Anonymous telemetry enabled. More info and opt-out:"
  echo "https://docs.shuttle.dev/install-script"
  [[ "$SUPPORTS_COLOR" = "1" ]] && echo -en "\e[0m"
fi
echo "==================================================="
echo


_send_telemetry() {
  if [[ "$TELEMETRY" = "1" ]]; then
    ENDED_AT=""
    if command -v date &>/dev/null; then
      ENDED_AT="$(date -u -Iseconds)"
    fi
    telemetry_data="{
  \"api_key\":\"phc_cQMQqF5QmcEzXEaVlrhv3yBSNRyaabXYAyiCV7xKHUH\",
  \"distinct_id\":\"install_script\",
  \"event\":\"install_script_completion\",
  \"properties\":{
    \"platform\":\"$PLATFORM\",
    \"arch\":\"$ARCH\",
    \"new_install\":\"$NEW_INSTALL\",
    \"install_method\":\"$INSTALL_METHOD\",
    \"started_at\":\"$STARTED_AT\",
    \"ended_at\":\"$ENDED_AT\",
    \"outcome\":\"$OUTCOME\",
    \"step_failed\":\"$STEP_FAILED\",
    \"dont_track_ip\":true
  }
}"
    [ -n "${SHUTTLE_DEBUG:-}" ] && echo -e "DEBUG: Sending telemetry data:\n$telemetry_data"
    curl -sL -H 'Content-Type: application/json' -d "$telemetry_data" https://console.shuttle.dev/ingest/i/v0/e > /dev/null
  fi
}

_exit_success() {
  OUTCOME="success"
  _send_telemetry
  echo
  [[ "$SUPPORTS_COLOR" = "1" ]] && echo -en "\e[32m" # green
  echo "Thanks for installing Impulse CLI! 🚀"
  [[ "$SUPPORTS_COLOR" = "1" ]] && echo -en "\e[0m"
  exit 0
}

_exit_neutral() {
  OUTCOME="neutral"
  echo
  echo "If you have any problems, please open an issue on GitHub or visit our Discord!"
  _send_telemetry
  exit 0
}

_exit_failure() {
  STEP_FAILED="$1"
  OUTCOME="failure"
  echo
  [[ "$SUPPORTS_COLOR" = "1" ]] && echo -en "\e[31m" # red
  echo "Impulse installation script failed with reason: $STEP_FAILED"
  [[ "$SUPPORTS_COLOR" = "1" ]] && echo -en "\e[0m"
  echo "If you have any problems, please open an issue on GitHub or visit our Discord!"
  _send_telemetry
  exit 1
}

if ! command -v curl &>/dev/null; then
  echo "curl not installed. Please install curl or use a different install method."
  _exit_failure "curl-not-found"
elif ! command -v sed &>/dev/null; then
  echo "sed not installed. Please install sed or use a different install method."
  _exit_failure "sed-not-found"
fi

REPO_URL="https://github.com/shuttle-hq/shuttle"
LATEST_RELEASE=$(curl -fsL -H 'Accept: application/json' "$REPO_URL/releases/latest")
[ $? -ne 0 ] && _exit_failure "check-latest-release"
# shellcheck disable=SC2001
LATEST_VERSION=$(echo "$LATEST_RELEASE" | sed -e 's/.*"tag_name":"\([^"]*\)".*/\1/')
[ $? -ne 0 ] && _exit_failure "parse-latest-version"

if command -v impulse &>/dev/null; then
  NEW_INSTALL="false"
  if [[ "$(impulse -V)" = *"${LATEST_VERSION#v}" ]]; then
    [[ "$SUPPORTS_COLOR" = "1" ]] && echo -en "\e[32m" # green
    echo "Impulse CLI is already at the latest version!"
    [[ "$SUPPORTS_COLOR" = "1" ]] && echo -en "\e[0m"
    exit 0 # skip telemetry and instantly exit
  else
    echo "Updating Impulse CLI to $LATEST_VERSION"
  fi
fi

_install_binary() {
  INSTALL_METHOD="binary-download"
  case "$OSTYPE" in
  linux*) target="$BIN_ARCH-unknown-linux-musl" ;;
  darwin*) target="$BIN_ARCH-apple-darwin" ;;
  *)
    echo "Cannot determine the target to install"
    _exit_failure "cannot-determine-target"
    ;;
  esac
  temp_dir=$(mktemp -d)
  [ $? -ne 0 ] && _exit_failure "mktemp"
  pushd "$temp_dir" >/dev/null || _exit_failure "pushd"
  curl -LO "$REPO_URL/releases/download/$LATEST_VERSION/cargo-shuttle-$LATEST_VERSION-$target.tar.gz" || _exit_failure "download-binary"
  if ! command -v tar &>/dev/null; then
    _exit_failure "tar-not-found"
  fi
  tar -xzf "cargo-shuttle-$LATEST_VERSION-$target.tar.gz" || _exit_failure "tar-extract-binary"
  mkdir -p "$INSTALL_DIR" || _exit_failure "create-install-dir"
  echo "Installing to $INSTALL_DIR/impulse"
  mv "cargo-shuttle-$target-$LATEST_VERSION/impulse" "$INSTALL_DIR/" || _exit_failure "move-binary"
  popd >/dev/null || _exit_failure "popd"
  if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo "Add $INSTALL_DIR to PATH to access the 'impulse' command"
  fi
}

_install_binary

_exit_success

# vim: ts=2 sw=2 et:
