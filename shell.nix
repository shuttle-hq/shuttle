let
  moz_overlay = import (builtins.fetchTarball https://github.com/mozilla/nixpkgs-mozilla/archive/master.tar.gz);
  # Pin to stable from https://status.nixos.org/
  nixpkgs = import (fetchTarball "https://github.com/NixOS/nixpkgs/archive/3d47bbaa26e7a771059d828eecf3bd8bf28a8b0f.tar.gz") { overlays = [ moz_overlay ]; };
in
  with nixpkgs;
  stdenv.mkDerivation {
    name = "moz_overlay_shell";
    nativeBuildInputs = with nixpkgs; [
      openssl
      pkg-config
    ];
    buildInputs = with nixpkgs; [
      ((rustChannelOf{ channel = "1.63.0"; }).rust.override {
        targets = ["wasm32-wasi"];
      })
      rust-analyzer
      cargo-watch
      terraform
      awscli2
      websocat
      protobuf
      grpcurl
      gh
      docker-compose
      docker
      datadog-agent
      sccache
      sqlite
    ];

    PROTOC = "${protobuf}/bin/protoc";
    PROTOC_INCLUDE="${protobuf}/include";
    RUSTC_WRAPPER="sccache";
  }
