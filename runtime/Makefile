.PHONY: axum serenity

all: axum serenity

axum:
	cd ../tmp/axum-wasm; cargo build --target wasm32-wasi
	cp ../tmp/axum-wasm/target/wasm32-wasi/debug/shuttle_axum.wasm axum.wasm

serenity:
	cd ../tmp/wasm; cargo build --target wasm32-wasi
	cp ../tmp/wasm/target/wasm32-wasi/debug/shuttle_serenity.wasm serenity.wasm

test: serenity axum
	cargo test --all-features -- --nocapture

runtime:
	cargo build
