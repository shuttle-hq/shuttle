#!/usr/bin/env bash

set -uo pipefail

if [[ "${TERM:-}" = "xterm-256color" ]]; then
  SUPPORTS_COLOR="1"
  echo -e "\
\e[40;38;5;208m       ___                                  \e[0m
\e[40;38;5;208m      /   \\ \e[37m   _           _   _   _        \e[0m
\e[40;38;5;208m   __/    /\e[37m___| |__  _   _| |_| |_| | ___   \e[0m
\e[40;38;5;208m  /_     /\e[37m/ __| '_ \\| | | | __| __| |/ _ \\  \e[0m
\e[40;38;5;208m   _|_  | \e[37m\__ \\ | | | |_| | |_| |_| |  __/  \e[0m
\e[40;38;5;208m  |_| |/  \e[37m|___/_| |_|\\__,_|\\__|\\__|_|\\___|  \e[0m
\e[40m                                            \e[0m
"
else
  SUPPORTS_COLOR="0"
  echo "\
       ___
      /   \\    _           _   _   _
   __/    /___| |__  _   _| |_| |_| | ___
  /_     // __| '_ \\| | | | __| __| |/ _ \\
   _|_  | \__ \\ | | | |_| | |_| |_| |  __/
  |_| |/  |___/_| |_|\\__,_|\\__|\\__|_|\\___|
"
fi
echo "
https://docs.shuttle.dev
https://discord.gg/shuttle
https://github.com/shuttle-hq/shuttle

Please open an issue if you encounter any problems!
===================================================
"

# Anonymous telemetry
TELEMETRY="1"
PLATFORM=""
NEW_INSTALL="true"
INSTALL_METHOD=""
OUTCOME=""
STEP_FAILED="N/A"
STARTED_AT=""
if command -v date &>/dev/null; then
  STARTED_AT="$(date -u -Iseconds)"
fi

# disable telemetry if any opt-out vars are set
if [[ \
    "${DO_NOT_TRACK:-}" == "1" || "${DO_NOT_TRACK:-}" == "true" || \
    "${DISABLE_TELEMETRY:-}" == "1" || "${DISABLE_TELEMETRY:-}" == "true" || \
    "${SHUTTLE_DISABLE_TELEMETRY:-}" == "1" || "${SHUTTLE_DISABLE_TELEMETRY:-}" == "true" || \
    "${CI:-}" == "1" || "${CI:-}" == "true"
  ]]; then
  TELEMETRY=0
fi

if [[ "$TELEMETRY" = "1" ]]; then
  [[ "$SUPPORTS_COLOR" = "1" ]] && echo -en "\e[2m"
  echo "Anonymous telemetry enabled. More info and opt-out:"
  echo "https://docs.shuttle.dev/getting-started/installation#install-script"
  [[ "$SUPPORTS_COLOR" = "1" ]] && echo -en "\e[0m"
  echo
fi

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
    \"new_install\":\"$NEW_INSTALL\",
    \"install_method\":\"$INSTALL_METHOD\",
    \"started_at\":\"$STARTED_AT\",
    \"ended_at\":\"$ENDED_AT\",
    \"outcome\":\"$OUTCOME\",
    \"step_failed\":\"$STEP_FAILED\",
    \"dont_track_ip\":true
  }
}"
    [ -n "${SHUTTLE_DEBUG:-}" ] && echo -e "Sending telemetry data:\n$telemetry_data"
    curl -sL -H 'Content-Type: application/json' -d "$telemetry_data" https://console.shuttle.dev/ingest/i/v0/e > /dev/null
  fi
}

_exit_success() {
  OUTCOME="success"
  _send_telemetry
  echo
  [[ "$SUPPORTS_COLOR" = "1" ]] && echo -en "\e[32m" # green
  echo "Thanks for installing Shuttle CLI! 🚀"
  [[ "$SUPPORTS_COLOR" = "1" ]] && echo -en "\e[0m"
  exit 0
}

_exit_neutral() {
  echo
  echo "If you have any problems, please open an issue on GitHub or visit our Discord!"
  exit 0
}

_exit_failure() {
  STEP_FAILED="$1"
  OUTCOME="fail"
  echo
  [[ "$SUPPORTS_COLOR" = "1" ]] && echo -en "\e[31m" # red
  echo "Shuttle installation script failed with reason: $STEP_FAILED"
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

if command -v cargo-shuttle &>/dev/null; then
  NEW_INSTALL="false"
  if [[ "$(cargo-shuttle -V)" = *"${LATEST_VERSION#v}" ]]; then
    [[ "$SUPPORTS_COLOR" = "1" ]] && echo -en "\e[32m" # green
    echo "Shuttle CLI is already at the latest version!"
    [[ "$SUPPORTS_COLOR" = "1" ]] && echo -en "\e[0m"
    exit 0 # skip telemetry and instantly exit
  else
    echo "Updating Shuttle CLI to $LATEST_VERSION"
  fi
fi

_install_linux() {
  echo "Detected Linux!"
  echo "Checking distro..."
  if (uname -a | grep -qi "Microsoft"); then
    OS="ubuntuwsl"
  elif ! command -v lsb_release &>/dev/null; then
    [ -n "${SHUTTLE_DEBUG:-}" ] && echo "lsb_release could not be found. Falling back to /etc/os-release"
    OS="$(grep -Eo '^ID=.*$' /etc/os-release | cut -d '=' -f 2 | tr '[:upper:]' '[:lower:]')" 2>/dev/null
  else
    OS=$(lsb_release -i | awk '{ print $3 }' | tr '[:upper:]' '[:lower:]')
  fi
  case "$OS" in
  "arch" | "manjarolinux" | "endeavouros")
    _install_arch_linux
    ;;
  "alpine")
    _install_alpine_linux
    ;;
  "ubuntu" | "ubuntuwsl" | "debian" | "linuxmint" | "parrot" | "kali" | "elementary" | "pop")
    # TODO: distribute .deb packages via `cargo-deb` and install them here
    _install_default
    ;;
  *)
    _install_default
    ;;
  esac
}

_install_arch_linux() {
  echo "Arch Linux detected!"
  if command -v pacman &>/dev/null; then
    pacman_version=$(sudo pacman -Si cargo-shuttle | sed -n 's/^Version *: \(.*\)/\1/p')
    if [[ "${pacman_version}" != "${LATEST_VERSION#v}"* ]]; then
      echo "cargo-shuttle is not updated in the repos, ping @orhun!!!"
      _install_default
    else
      echo "Installing with pacman"
      INSTALL_METHOD="pacman"
      sudo pacman -S --noconfirm cargo-shuttle || _exit_failure "pacman-install"
    fi
  else
    echo "Pacman not found"
    _install_default
  fi
}

_install_alpine_linux() {
  echo "Alpine Linux detected!"
  if command -v apk &>/dev/null; then
    if apk search -q cargo-shuttle; then
      echo "cargo-shuttle is not available in the testing repository. Do you want to enable the testing repository? (y/n)"
      read -r enable_testing
      if [ "$enable_testing" = "y" ]; then
        echo "@testing http://dl-cdn.alpinelinux.org/alpine/edge/testing" | tee -a /etc/apk/repositories
        apk update || _exit_failure "apk-update"
      else
        _install_default
        return 0
      fi
    fi
    if ! apk info cargo-shuttle; then
      echo "Installing with apk"
      INSTALL_METHOD="apk"
      apk add cargo-shuttle@testing || _exit_failure "apk-add"
    else
      apk_version=$(apk version cargo-shuttle | awk 'NR==2{print $3}')
      if [[ "${apk_version}" != "${LATEST_VERSION#v}"* ]]; then
        echo "cargo-shuttle is not updated in the testing repository, ping @orhun!!!"
        _install_default
      else
        echo "cargo-shuttle is already up to date."
      fi
    fi
  else
    echo "APK (Alpine Package Keeper) not found"
    _install_default
  fi
}

# TODO: package cargo-shuttle for Homebrew
_install_mac() {
  _install_default
}

_install_binary() {
  INSTALL_METHOD="binary-download"
  case "$OSTYPE" in
  linux*) target="x86_64-unknown-linux-musl" ;;
  darwin*) target="x86_64-apple-darwin" ;;
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
  echo "Installing to $HOME/.cargo/bin/cargo-shuttle"
  mv "cargo-shuttle-$target-$LATEST_VERSION/cargo-shuttle" "$HOME/.cargo/bin/" || _exit_failure "move-binary"
  echo "Installing to $HOME/.cargo/bin/shuttle"
  mv "cargo-shuttle-$target-$LATEST_VERSION/shuttle" "$HOME/.cargo/bin/" || _exit_failure "move-binary"
  popd >/dev/null || _exit_failure "popd"
  if [[ ":$PATH:" != *":$HOME/.cargo/bin:"* ]]; then
    echo "Add $HOME/.cargo/bin to PATH to access the 'shuttle' command"
  fi
}

_install_rust_and_cargo() {
  echo "Rust installation not found!"
  while true; do
    read -r -p "Install Rust and Cargo via rustup? [Y/n] " yn </dev/tty
    case $yn in
    [Yy]*|"")
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y || _exit_failure "install-rust"
      source "$HOME/.cargo/env" # TODO: this only affects this script's env. Print hint to do this manually after installation for the user's shell
      break
      ;;
    [Nn]*)
      _exit_neutral
      ;;
    *) echo "Please answer yes or no." ;;
    esac
  done
}

_install_with_cargo() {
  echo "Installing with cargo..."
  INSTALL_METHOD="cargo"
  cargo install --locked cargo-shuttle || _exit_failure "cargo-install"
}

_install_default() {
  echo "Installing with package manager is not supported"

  if command -v cargo-binstall &>/dev/null; then
    echo "Installing with cargo-binstall"
    INSTALL_METHOD="cargo-binstall"
    cargo-binstall -y --force --locked cargo-shuttle || _exit_failure "cargo-binstall"
    return 0
  fi

  while true; do
    read -r -p "Install the pre-built binary? [Y/n] " yn </dev/tty
    case $yn in
    [Yy]*|"")
      _install_binary
      break
      ;;
    [Nn]*)
      while true; do
        read -r -p "Install from source with cargo? [Y/n] " yn </dev/tty
        case $yn in
        [Yy]*|"")
          _install_with_cargo
          break
          ;;
        [Nn]*)
          _exit_neutral
          ;;
        *) echo "Please answer yes or no." ;;
        esac
      done
      ;;
    *) echo "Please answer yes or no." ;;
    esac
  done
}

# Check Rust installation since it is a required dependency
if ! command -v cargo &>/dev/null; then
  if ! command -v rustup &>/dev/null; then
    _install_rust_and_cargo
    echo
  else
    echo "rustup was found, but cargo wasn't. Something is up with your Rust installation."
    _exit_failure "rustup-found-cargo-not-found"
  fi
fi

case "$OSTYPE" in
linux*)
  PLATFORM="linux"
  _install_linux
  ;;
darwin*)
  PLATFORM="macos"
  _install_mac
  ;;
*)
  PLATFORM="unknown"
  _install_default
  ;;
esac

_exit_success

# vim: ts=2 sw=2 et:
