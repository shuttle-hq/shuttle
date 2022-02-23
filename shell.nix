{ pkgs ? import <nixpkgs> { } }:
pkgs.mkShell {
  nativeBuildInputs = with pkgs; [ rustc cargo ];
  buildInputs = with pkgs; [ rustfmt clippy rust-analyzer cargo-watch ];

  RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
}
