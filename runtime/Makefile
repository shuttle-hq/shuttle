.PHONY: wasm next

all: wasm next

wasm:
	cd ../tmp/wasm; cargo build --target wasm32-wasi
	cp ../tmp/wasm/target/wasm32-wasi/debug/shuttle_serenity.wasm bot.wasm

test: wasm
	cargo test -- --nocapture

runtime:
	cargo build
