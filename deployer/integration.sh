#!/usr/bin/env bash
cargo test -p shuttle-deployer --all-features --test '*' -- --skip needs_docker --nocapture
