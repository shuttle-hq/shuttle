.PHONY: axum

all: axum

axum:
	cd ../tmp/axum-wasm-expanded; cargo build --target wasm32-wasi
	cp ../tmp/axum-wasm-expanded/target/wasm32-wasi/debug/shuttle_axum_expanded.wasm axum.wasm

test: axum
	cargo test --all-features -- --nocapture

runtime:
	cargo build
