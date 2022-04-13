let
  moz_overlay = import (builtins.fetchTarball https://github.com/mozilla/nixpkgs-mozilla/archive/master.tar.gz);
  nixpkgs = import (fetchTarball "https://github.com/NixOS/nixpkgs/archive/e7d63bd0d50df412f5a1d8acfa3caae75522e347.tar.gz") { overlays = [ moz_overlay ]; };
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
