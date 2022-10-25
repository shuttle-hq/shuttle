.PHONY: wasm runtime

all: wasm runtime

wasm:
	cd wasm; cargo build --target wasm32-wasi
	cp ../target/wasm32-wasi/debug/shuttle_serenity.wasm runtime/bot.wasm

test: wasm
	cd runtime; cargo test -- --nocapture

runtime:
	cd runtime; cargo build
