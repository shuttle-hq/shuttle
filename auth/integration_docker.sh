#!/usr/bin/env bash
cargo test -p shuttle-auth --all-features --test '*' -- needs_docker --nocapture
