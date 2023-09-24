#!/usr/bin/env bash
cargo test -p shuttle-service --all-features --test '*' -- --skip needs_docker --nocapture
