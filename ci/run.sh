#!/bin/sh

set -ex

export RUST_TEST_THREADS=1
export RUST_BACKTRACE=1

cargo test
cargo test --features "unstable"
cargo test --features "bytes_buf"
cargo test --release
cargo test --features "unstable" --release
cargo test --features "bytes_buf" --release
cargo doc
