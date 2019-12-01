#!/usr/bin/env sh

set -ex

: "${TARGET?The TARGET environment variable must be set.}"

export RUST_TEST_THREADS=1
export RUST_BACKTRACE=1
export RUST_TEST_NOCAPTURE=1

# If the build should not run tests, just check that the code builds:
if [ "${NORUN}" = "1" ]; then
    export CARGO="cargo build"
else
    export CARGO="cargo test"
fi
export CARGO="${CARGO} --target=${TARGET} --no-default-features"

# Simulators:
case "${TARGET}" in
    *ios*)
        export RUSTFLAGS=-Clink-arg=-mios-simulator-version-min=7.0
        rustc ./ci/deploy_and_run_on_ios_simulator.rs -o "${HOME}"/runtest
        export CARGO_TARGET_X86_64_APPLE_IOS_RUNNER="${HOME}"/runtest
        export CARGO_TARGET_I386_APPLE_IOS_RUNNER="${HOME}"/runtest
        ;;
esac

# Make sure that the builds with --no-default-features don't have
# any libstd symbols linked:
cargo clean
cargo build --no-default-features
set +e
if find target/ -name "*.rlib" -exec nm {} \; 2>&1 | grep "std"; then
    exit 1
fi
cargo clean
cargo test --release --no-default-features
if find target/ -name "*.rlib" -exec nm {} \; | grep  "std"; then
    exit 1
fi
set -e

FEATURES="\
use_std \
unstable \
bytes_buf \
use_std,unstable,bytes_buf \
"

for FEATURE in ${FEATURES}; do
    if [ "${SYSV}" = 1 ]; then
        FEATURE="${FEATURE},unix_sysv"
    fi
    "${CARGO} --features=${FEATURE}"
    "${CARGO} --release --features=${FEATURE}"
done

test_asan() {
    RUSTFLAGS_BAK=$RUSTFLAGS
    RUSTFLAGS="${RUSTFLAGS_BAK} -Z sanitizer=address"
    export ASAN_OPTIONS="detect_odr_violation=0:detect_leaks=0"
    "${CARGO}"
    "${CARGO} --release"
    RUSTFLAGS="${RUSTFLAGS_BAK}"
}

# Sanitizers:
case "${TARGET}" in
    x86_64-unknown-linux-gnu*)
        test_asan
        ;;
    x86_64-apple-darwin*)
        test_asan
        ;;
esac
