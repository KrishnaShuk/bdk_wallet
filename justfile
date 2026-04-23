alias b := build
alias c := check
alias f := fmt
alias t := test
alias p := pre-push

_default:
   @just --list

# Build the project
build:
   cargo build

# Check code: formatting, compilation, linting, and commit signature
check:
   cargo +nightly fmt --all -- --check
   cargo check --all-features --all-targets
   cargo clippy --all-features --all-targets -- -D warnings
   @[ "$(git log --pretty='format:%G?' -1 HEAD)" = "N" ] && \
       echo "\n⚠️  Unsigned commit: BDK requires that commits be signed." || \
       true

# Format all code
fmt:
   cargo +nightly fmt

# Regenerate `Cargo-recent.lock` and `Cargo-minimal.lock`
lock:
    cargo +nightly rbmt lock

# Verify the library builds with MSRV (1.85.0)
msrv:
    cargo rbmt test --toolchain msrv --lock-file minimal

# Run all tests on the workspace with all features
test:
   cargo test --all-features

# Run pre-push suite: format, check, and test
pre-push: fmt check test
