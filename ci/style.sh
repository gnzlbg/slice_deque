#!/usr/bin/env sh

set -ex

if rustup component add rustfmt-preview ; then
    command -v rustfmt
    rustfmt -V
    cargo fmt --all -- --check
fi

if rustup component add clippy-preview ; then
    cargo clippy -V
    cargo clippy --no-default-features -- -D clippy::pedantic
    cargo clippy --features=unstable -- -D clippy::pedantic
fi

if shellcheck --version ; then
    shellcheck ci/*.sh
fi
