let
  moz_overlay = import (builtins.fetchTarball https://github.com/mozilla/nixpkgs-mozilla/archive/master.tar.gz);
  nixpkgs = import <nixpkgs> { overlays = [ moz_overlay ]; };
in
  with nixpkgs;
  stdenv.mkDerivation {
    name = "moz_overlay_shell";
    nativeBuildInputs = with nixpkgs; [
      latest.rustChannels.stable.rust
      openssl
      pkg-config
    ];
    buildInputs = with nixpkgs; [
      latest.rustChannels.stable.rust
      rust-analyzer
      cargo-watch
    ];
  }
