#!/usr/bin/env bash
cargo test -p shuttle-resource-recorder --all-features --test '*' -- --skip needs_docker --nocapture
