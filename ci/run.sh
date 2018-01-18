#!/bin/sh

set -ex

export RUST_TEST_THREADS=1
export RUST_BACKTRACE=1

cargo test
cargo test --release
# cargo bench -- --nocapture
cargo doc
