let
  moz_overlay = import (builtins.fetchTarball https://github.com/mozilla/nixpkgs-mozilla/archive/master.tar.gz);
  # Pin to stable from https://status.nixos.org/
  nixpkgs = import (fetchTarball "https://github.com/NixOS/nixpkgs/archive/3c5ae9be1f18c790ea890ef8decbd0946c0b4c04.tar.gz") { overlays = [ moz_overlay ]; };
in
  with nixpkgs;
  stdenv.mkDerivation {
    name = "moz_overlay_shell";
    nativeBuildInputs = with nixpkgs; [
      openssl
      pkg-config
    ];
    buildInputs = with nixpkgs; [
      latest.rustChannels.stable.rust
      rust-analyzer
      cargo-watch
      terraform
      awscli2
    ];
  }
