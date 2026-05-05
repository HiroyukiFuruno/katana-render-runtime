set shell := ["bash", "-uc"]

JOBS := env_var_or_default("JOBS", "2")
CARGO := env_var_or_default("CARGO", "cargo")
VERSION := env_var_or_default("VERSION", `awk -F '"' '/^version = / { print $2; exit }' Cargo.toml`)
COVERAGE_MIN_LINES := env_var_or_default("COVERAGE_MIN_LINES", "67")

default: help

# Show available tasks
help:
    @just --list --unsorted

# Apply Rust formatting
fmt:
    {{CARGO}} fmt --all

# Check Rust formatting
fmt-check:
    {{CARGO}} fmt --all -- --check

# Run strict Clippy checks
lint:
    RUSTFLAGS="-D warnings" {{CARGO}} clippy -j {{JOBS}} --workspace --all-targets --all-features -- -D warnings -D clippy::unwrap_used -D clippy::expect_used -D clippy::todo -D clippy::unimplemented -D clippy::dbg_macro -D clippy::panic -D clippy::wildcard_imports

# Run Rust syntax based structural checks
ast-lint:
    {{CARGO}} test -j {{JOBS}} -p kcf-linter ast_linter -- --nocapture

# Run workspace tests
unit-test:
    {{CARGO}} test --workspace --all-targets --all-features

# Run coverage as a required full-check gate
coverage:
    {{CARGO}} llvm-cov --workspace --all-features --locked --summary-only --fail-under-lines {{COVERAGE_MIN_LINES}}

# Run the local quality gate
check: fmt-check lint unit-test ast-lint
    @echo "checks passed"

# Verify package metadata and dry-run the first publishable crate
release-verify: check coverage
    bash scripts/release/verify-version.sh "{{VERSION}}"
    bash scripts/release/verify-internal-dependencies.sh "{{VERSION}}"
    {{CARGO}} package -p katana-canvas-forge --locked --allow-dirty
    {{CARGO}} package -p katana-canvas-forge-cli --locked --allow-dirty --list >/dev/null
    {{CARGO}} publish -p katana-canvas-forge --dry-run --locked --allow-dirty

# Verify release branch readiness before merging
release-check: release-verify
    bash scripts/release/assert-crates-not-published.sh "{{VERSION}}"
