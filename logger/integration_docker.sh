#!/usr/bin/env bash
cargo test -p shuttle-logger --all-features --test '*' -- needs_docker --nocapture
