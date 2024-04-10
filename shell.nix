let
  moz_overlay = import (builtins.fetchTarball https://github.com/mozilla/nixpkgs-mozilla/archive/master.tar.gz);
  # Pin to stable from https://status.nixos.org/
  nixpkgs = import (fetchTarball "https://github.com/NixOS/nixpkgs/archive/219951b495fc2eac67b1456824cc1ec1fd2ee659.tar.gz") { overlays = [ moz_overlay ]; };
in
  with nixpkgs;
  stdenv.mkDerivation {
    name = "moz_overlay_shell";
    nativeBuildInputs = with nixpkgs; [
      pkg-config
      openssl
    ];
    buildInputs = with nixpkgs; [
      ((rustChannelOf{ channel = "1.77.1"; }).rust.override {
        extensions = ["rust-src"];
      })
      cargo-watch
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
      fastmod
      pebble
      kondo
    ];

    PROTOC = "${protobuf}/bin/protoc";
    PROTOC_INCLUDE="${protobuf}/include";
    RUSTC_WRAPPER="sccache";
  }
