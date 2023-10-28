#!/usr/bin/env bash
cargo test -p shuttle-provisioner --all-features --test '*' -- needs_docker --nocapture
