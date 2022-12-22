.PHONY: axum

all: axum

axum:
	cd ../tmp/axum-wasm; cargo build --target wasm32-wasi
	cp ../tmp/axum-wasm/target/wasm32-wasi/debug/shuttle_axum.wasm axum.wasm

test: axum
	cargo test --all-features -- --nocapture

runtime:
	cargo build
