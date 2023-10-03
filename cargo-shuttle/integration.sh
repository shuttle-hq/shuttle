#!/usr/bin/env bash
cargo test -p cargo-shuttle --all-features --test '*' -- --skip needs_docker --nocapture
