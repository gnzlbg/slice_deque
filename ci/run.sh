#!/usr/bin/env bash

set -ex

export RUST_TEST_THREADS=1
export RUST_BACKTRACE=1

# Select cargo command: use cross by default
export CARGO_CMD=cross

# On Appveyor and Travis native targets we use cargo (no need to cross-compile):
if [[ $TARGET = *"windows"* ]] || [[ $TARGET == "x86_64-unknown-linux-gnu" ]]; then
    export CARGO_CMD=cargo
fi

# Install cross if necessary:
if [[ $CARGO_CMD == "cross" ]]; then
    cargo install cross
fi

$CARGO_CMD test --target=$TARGET -- --nocapture
$CARGO_CMD test --target=$TARGET --release -- --nocapture

$CARGO_CMD test --target=$TARGET --no-default-features -- --nocapture
$CARGO_CMD test --target=$TARGET --no-default-features --release -- --nocapture

$CARGO_CMD test --target=$TARGET --no-default-features --features "std" -- --nocapture
$CARGO_CMD test --target=$TARGET --no-default-features --features "std" --release -- --nocapture

$CARGO_CMD test --target=$TARGET --no-default-features --features "unstable" -- --nocapture
$CARGO_CMD test --target=$TARGET --no-default-features --features "unstable" --release -- --nocapture

$CARGO_CMD test --target=$TARGET --no-default-features --features "bytes_buf" -- --nocapture
$CARGO_CMD test --target=$TARGET --no-default-features --features "bytes_buf" --release -- --nocapture

$CARGO_CMD test --target=$TARGET --no-default-features --features "std,unstable" -- --nocapture
$CARGO_CMD test --target=$TARGET --no-default-features --features "std,unstable" --release -- --nocapture

$CARGO_CMD test --target=$TARGET --no-default-features --features "unstable,bytes_buf" -- --nocapture
$CARGO_CMD test --target=$TARGET --no-default-features --features "unstable,bytes_buf" --release -- --nocapture

if [[ $SYSV == "1" ]]; then
    $CARGO_CMD test --target=$TARGET --no-default-features --features "unix_sysv" -- --nocapture
    $CARGO_CMD test --target=$TARGET --no-default-features --features "unix_sysv" --release -- --nocapture
    $CARGO_CMD test --target=$TARGET --no-default-features --features "std,unstable,unix_sysv" -- --nocapture
    $CARGO_CMD test --target=$TARGET --no-default-features --features "std,unstable,unix_sysv" --release -- --nocapture
fi

if [[ $CARGO_CMD == "cargo" ]]; then
    cargo doc
    cargo install clippy --force
    cargo clippy -- -D clippy-pedantic
fi
