#!/usr/bin/env sh

###############################################################################
# This file is used by our common Containerfile incase the container for this #
# service might need some extra preparation steps for its final image         #
###############################################################################

# Install the nix package manager
curl -L https://nixos.org/nix/install > ./install.sh
chmod +x install.sh
./install.sh --daemon
rm install.sh

# Activate the nix command
echo "experimental-features = nix-command" >> /etc/nix/nix.conf
