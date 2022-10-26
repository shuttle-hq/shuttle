.PHONY: wasm next

all: wasm next

wasm:
	cd wasm; cargo build --target wasm32-wasi
	cp target/wasm32-wasi/debug/shuttle_serenity.wasm next/bot.wasm

test: wasm
	cd next; cargo test -- --nocapture

next:
	cd next; cargo build
