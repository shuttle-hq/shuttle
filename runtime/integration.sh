#!/usr/bin/env bash
cargo test -p shuttle-runtime --all-features --test '*' -- --skip needs_docker --nocapture
