#! /usr/bin/env bash

set -euo pipefail

cat <<EOF
     _           _   _   _      
 ___| |__  _   _| |_| |_| | ___ 
/ __| '_ \\| | | | __| __| |/ _ \\
\__ \\ | | | |_| | |_| |_| |  __/
|___/_| |_|\\__,_|\\__|\\__|_|\\___|
                                
https://www.shuttle.rs
https://github.com/shuttle-hq/shuttle

Please file an issue if you encounter any problems!
===================================================
EOF

if ! command -v curl &>/dev/null; then
  echo "curl not installed. Please install curl."
  exit
fi

REPO_URL="https://github.com/shuttle-hq/shuttle"
LATEST_RELEASE=$(curl -L -s -H 'Accept: application/json' "$REPO_URL/releases/latest")
LATEST_VERSION="${LATEST_RELEASE#*\"tag_name\":\"}"
LATEST_VERSION="${LATEST_VERSION%%\"*}"

_install_linux() {
  echo "Detected Linux!"
  echo "Checking distro..."
  if (uname -a | grep -qi "Microsoft"); then
    OS="ubuntuwsl"
  elif ! command -v lsb_release &>/dev/null; then
    echo "lsb_release could not be found. Falling back to /etc/os-release"
    OS="$(grep -Po '(?<=^ID=).*$' /etc/os-release | tr '[:upper:]' '[:lower:]')" 2>/dev/null
  else
    OS=$(lsb_release -i | awk '{ print $3 }' | tr '[:upper:]' '[:lower:]')
  fi
  case "$OS" in
  "arch" | "manjarolinux" | "endeavouros")
    _install_arch_linux
    ;;
  "ubuntu" | "ubuntuwsl" | "debian" | "linuxmint" | "parrot" | "kali" | "elementary" | "pop")
    # TODO: distribute .deb packages via `cargo-deb` and install them here
    _install_unsupported
    ;;
  *)
    _install_unsupported
    ;;
  esac
}

_install_arch_linux() {
  echo "Arch Linux detected!"
  if command -v pacman &>/dev/null; then
    echo "Installing with pacman"
    sudo pacman -S cargo-shuttle
  else
    echo "Pacman not found"
    exit 1
  fi
}

# TODO: package cargo-shuttle for Homebrew
_install_mac() {
  _install_unsupported
}

_install_binary() {
  echo "Installing pre-built binary..."
  case "$OSTYPE" in
  linux*) target="x86_64-unknown-linux-musl" ;;
  darwin*) target="x86_64-apple-darwin" ;;
  *)
    echo "Cannot determine the target to install"
    exit 1
    ;;
  esac
  temp_dir=$(mktemp -d)
  pushd "$temp_dir" >/dev/null || exit 1
  curl -LO "$REPO_URL/releases/download/$LATEST_VERSION/cargo-shuttle-$LATEST_VERSION-$target.tar.gz"
  tar -xzf "cargo-shuttle-$LATEST_VERSION-$target.tar.gz"
  echo "Installing to $HOME/.cargo/bin/cargo-shuttle"
  mv "cargo-shuttle-$target-$LATEST_VERSION/cargo-shuttle" "$HOME/.cargo/bin/"
  popd >/dev/null || exit 1
  if [[ ":$PATH:" != *":$HOME/.cargo/bin:"* ]]; then
    echo "Add $HOME/.cargo/bin to PATH to run cargo-shuttle"
  fi
}

_install_cargo() {
  echo "Attempting install with cargo"
  if ! command -v cargo &>/dev/null; then
    echo "cargo not found! Attempting to install Rust and cargo via rustup"
    if command -v rustup &>/dev/null; then
      echo "rustup was found, but cargo wasn't. Something is up with your install"
      exit 1
    fi
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -q
    echo "rustup installed! Attempting cargo install"
  fi

  cargo install cargo-shuttle
}

_install_unsupported() {
  echo "Installing with package manager is not supported"

  if command -v cargo-binstall &>/dev/null; then
    echo "Installing with cargo-binstall"
    cargo binstall -y --locked cargo-shuttle
    return 0
  fi

  while true; do
    read -r -p "Do you wish to attempt to install the pre-built binary? [Y/N] " yn
    case $yn in
    [Yy]*)
      _install_binary
      break
      ;;
    [Nn]*)
      read -r -p "Do you wish to attempt an install with 'cargo'? [Y/N] " yn
      case $yn in
      [Yy]*)
        echo "Installing with 'cargo'..."
        _install_cargo
        break
        ;;
      [Nn]*) exit ;;
      *) echo "Please answer yes or no." ;;
      esac
      ;;
    *) echo "Please answer yes or no." ;;
    esac
  done
}

case "$OSTYPE" in
linux*) _install_linux ;;
darwin*) _install_mac ;;
*) _install_unsupported ;;
esac

cat <<EOF
Thanks for installing cargo-shuttle! 🚀

If you have any issues, please open an issue on GitHub or visit our Discord (https://discord.gg/shuttle)!
EOF

# vim: ts=2 sw=2 et:
