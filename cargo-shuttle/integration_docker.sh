#!/usr/bin/env bash
cargo test -p cargo-shuttle --all-features --test '*' -- needs_docker --nocapture
