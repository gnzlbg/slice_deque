#!/usr/bin/env bash

set -ex

export RUST_TEST_THREADS=1
export RUST_BACKTRACE=1
export RUST_TEST_NOCAPTURE=1

export RUSTFLAGS="-Z sanitizer=${SANITIZER}"
export ASAN_OPTIONS="detect_odr_violation=0:detect_leaks=0"

export OPT="--no-default-features --features=unstable --target=x86_64-unknown-linux-gnu"
export OPT_RELEASE="--release ${OPT}"

if [[ $SANITIZER == "address" ]]; then
    cargo test --lib $OPT
    cargo test --lib $OPT_RELEASE
fi

cargo run --example san $OPT
cargo run --example san $OPT_RELEASE
