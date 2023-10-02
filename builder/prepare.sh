#!/usr/bin/env sh

##############################################################################################
# This file is run by Containerfile for extra preparation steps for this crate's final image #
##############################################################################################

# Install the nix package manager
curl -L https://nixos.org/nix/install > ./install.sh
chmod +x install.sh
./install.sh --daemon
rm install.sh

# Activate the nix command
echo "experimental-features = nix-command flakes" >> /etc/nix/nix.conf

# Create a symbolic link to the nix binary
ln -s /root/.nix-profile/bin/nix /usr/bin/nix
