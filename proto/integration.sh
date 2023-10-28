#!/usr/bin/env bash
cargo test -p shuttle-proto --all-features --test '*' -- --skip needs_docker --nocapture
