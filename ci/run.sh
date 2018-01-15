#!/bin/sh

set -ex

export RUST_TEST_THREADS=1
export RUST_BACKTRACE=1

cargo test --target=$TARGET
cargo test --target=$TARGET --release
cargo bench
cargo doc
