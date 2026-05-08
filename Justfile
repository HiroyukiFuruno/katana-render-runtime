set shell := ["bash", "-uc"]

JOBS := env_var_or_default("JOBS", "2")
CARGO := env_var_or_default("CARGO", "cargo")
VERSION := env_var_or_default("VERSION", `awk -F '"' '/^version = / { print $2; exit }' Cargo.toml`)
COVERAGE_MIN_LINES := env_var_or_default("COVERAGE_MIN_LINES", "100")
COVERAGE_MAX_UNCOVERED_LINES := env_var_or_default("COVERAGE_MAX_UNCOVERED_LINES", "0")
MERMAID_JS_VERSION := "3.3.1"
DRAWIO_JS_VERSION := "29.7.10"
MERMAID_JS := env_var_or_default("MERMAID_JS", "crates/katana-canvas-forge/vendor/mermaid/" + MERMAID_JS_VERSION + "/mermaid.min.js")
DRAWIO_JS := env_var_or_default("DRAWIO_JS", "crates/katana-canvas-forge/vendor/drawio/" + DRAWIO_JS_VERSION + "/drawio.min.js")
DRAWIO_RESOURCE_DIR := "crates/katana-canvas-forge/src/markdown/drawio_renderer/js_runtime/resources"
DRAWIO_RESOURCE_MANIFEST := DRAWIO_RESOURCE_DIR + "/drawio-resource-manifest.json"

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
    RUSTFLAGS="-D warnings" {{CARGO}} clippy -j {{JOBS}} --workspace --all-targets --all-features -- -D warnings -D clippy::unwrap_used -D clippy::expect_used -D clippy::todo -D clippy::unimplemented -D clippy::dbg_macro -D clippy::panic -D clippy::wildcard_imports -D clippy::too_many_lines -D clippy::cognitive_complexity

# Run Rust syntax based structural checks
ast-lint:
    {{CARGO}} test -j {{JOBS}} -p kcf-linter ast_linter -- --nocapture

# Check that kcf does not depend on KatanA UI crates
dependency-leak:
    @dependencies="$({{CARGO}} tree --workspace -e normal)"; \
    pattern='(^|[[:space:]])(egui|katana-core|katana-ui|katana-platform|katana-native)([[:space:]]|$)'; \
    if printf '%s\n' "$dependencies" | grep -E "$pattern"; then \
      echo "KatanA UI dependency leaked into katana-canvas-forge." >&2; \
      exit 1; \
    fi

# Run workspace tests
unit-test:
    {{CARGO}} test --workspace --all-targets --all-features

# Run coverage as a required full-check gate
coverage:
    {{CARGO}} llvm-cov --workspace --all-features --locked --summary-only --fail-under-lines {{COVERAGE_MIN_LINES}} --fail-uncovered-lines {{COVERAGE_MAX_UNCOVERED_LINES}}

# Verify pinned runtime asset checksums
runtime-asset-check:
    cd crates/katana-canvas-forge/vendor/mermaid/{{MERMAID_JS_VERSION}} && shasum -a 256 -c mermaid.min.js.sha256
    cd crates/katana-canvas-forge/vendor/drawio/{{DRAWIO_JS_VERSION}} && shasum -a 256 -c drawio.min.js.sha256

# Run TypeScript tests for runtime asset helper scripts
runtime-asset-script-test:
    bun test scripts/runtime-assets/runtime-asset-common_test.ts

# Run the local quality gate
check: fmt-check lint unit-test ast-lint dependency-leak runtime-asset-check
    @echo "checks passed"

# Update dependency crates safely (respects Cargo.toml SemVer)
update-safe:
    {{CARGO}} update

# Upgrade all dependencies to latest versions (including breaking changes)
update:
    {{CARGO}} upgrade -i
    {{CARGO}} update

# Verify package metadata and dry-run the first publishable crate
release-verify: check coverage
    bash scripts/release/verify-version.sh "{{VERSION}}"
    bash scripts/release/verify-internal-dependencies.sh "{{VERSION}}"
    {{CARGO}} package -p katana-canvas-forge --locked --allow-dirty
    {{CARGO}} package -p katana-canvas-forge-cli --locked --allow-dirty --list >/dev/null
    bash scripts/release/verify-crate-size.sh katana-canvas-forge "{{VERSION}}"
    {{CARGO}} publish -p katana-canvas-forge --dry-run --locked --allow-dirty

# Verify release branch readiness before merging
release-check: release-verify
    bash scripts/release/assert-crates-not-published.sh "{{VERSION}}"

# Install Playwright Chromium for official Mermaid / Draw.io reference rendering
browser-install:
    playwright install chromium

# Show latest Mermaid.js and Draw.io versions without changing files
runtime-asset-latest runtime='all':
    bun run scripts/runtime-assets/latest-check.ts "{{runtime}}"

# Show latest Mermaid.js version without changing files
mermaid-latest:
    just runtime-asset-latest mermaid

# Show latest Draw.io version without changing files
drawio-latest:
    just runtime-asset-latest drawio

# Update Mermaid.js runtime asset and refresh references
mermaid-update version:
    bun run scripts/runtime-assets/update.ts mermaid "{{version}}"
    just mermaid-reference-all
    just mermaid-compare-full
    just mermaid-compare-ci

# Update Draw.io runtime asset and refresh references
drawio-update version:
    bun run scripts/runtime-assets/update.ts drawio "{{version}}"
    bun run scripts/drawio/resource-update.ts --resources "{{DRAWIO_RESOURCE_DIR}}" --manifest "{{DRAWIO_RESOURCE_MANIFEST}}"
    just drawio-reference-all
    just drawio-compare-full
    just drawio-compare-ci

# Render kcf Mermaid SVG fixtures
mermaid-render fixtures output='tmp/kcf-mermaid-rendered':
    @rm -rf "{{output}}"
    @mkdir -p "{{output}}"
    @for file in "{{fixtures}}"/*.md; do \
      slug=$(basename "$file" .md); \
      {{CARGO}} run -p katana-canvas-forge-cli -- mermaid render --input "$file" --output "{{output}}/$slug.svg"; \
    done

# Update official Mermaid reference SVG / PNG
mermaid-reference fixtures:
    bun run scripts/mermaid/diagram-update.ts --fixtures "{{fixtures}}" --output tmp/kcf-mermaid-official --markdown-output "{{fixtures}}/official-dark" --theme dark --mermaid-js "{{MERMAID_JS}}" --skip-errors

# Update all committed Mermaid reference SVG / PNG fixtures
mermaid-reference-all:
    just mermaid-reference tests/fixtures/mermaid/en
    just mermaid-reference tests/fixtures/mermaid/ja
    just mermaid-reference tests/fixtures/mermaid/representative

# Compare committed official Mermaid reference with kcf rendering through ImageMagick score
mermaid-compare fixtures min_score='99' output='tmp/kcf-mermaid':
    just mermaid-render "{{fixtures}}" "{{output}}/rendered"
    bun run scripts/mermaid/rasterize-svg-dir.ts --input "{{output}}/rendered" --output "{{output}}/rendered-browser" --theme dark
    bun run scripts/mermaid/reference-compare.ts --official "{{fixtures}}/official-dark" --katana "{{output}}/rendered-browser" --output "{{output}}/comparison" --theme dark --min-score "{{min_score}}"

# Compare representative Mermaid patterns for CI/CD
mermaid-compare-ci min_score='99':
    just mermaid-compare tests/fixtures/mermaid/representative "{{min_score}}" tmp/kcf-mermaid-ci

# Compare full Mermaid fixture sets for local release validation
mermaid-compare-full min_score='99':
    just mermaid-compare tests/fixtures/mermaid/en "{{min_score}}" tmp/kcf-mermaid-full/en
    just mermaid-compare tests/fixtures/mermaid/ja "{{min_score}}" tmp/kcf-mermaid-full/ja

# Render Mermaid fixtures for a timing smoke check
mermaid-bench fixtures:
    @start=$(date +%s); just mermaid-render "{{fixtures}}" tmp/kcf-mermaid-bench; end=$(date +%s); elapsed=$((end - start)); echo "mermaid fixtures rendered in ${elapsed}s"

# Render kcf Draw.io SVG fixtures
drawio-render fixtures output='tmp/kcf-drawio-rendered':
    @rm -rf "{{output}}"
    @mkdir -p "{{output}}"
    @for file in "{{fixtures}}"/*.drawio; do \
      slug=$(basename "$file" .drawio); \
      {{CARGO}} run -p katana-canvas-forge-cli -- drawio render --input "$file" --output "{{output}}/$slug.svg"; \
    done

# Update official Draw.io reference SVG / PNG
drawio-reference fixtures:
    bun run scripts/drawio/diagram-update.ts --fixtures "{{fixtures}}" --output "{{fixtures}}/official" --drawio-js "{{DRAWIO_JS}}" --resources "{{DRAWIO_RESOURCE_DIR}}" --resource-manifest "{{DRAWIO_RESOURCE_MANIFEST}}"

# Update all committed Draw.io reference SVG / PNG fixtures
drawio-reference-all:
    @set -euo pipefail; \
    root="tests/fixtures/drawio"; \
    for fixtures in \
      "$root/basic" \
      "$root/official/diagrams" \
      "$root/official/examples" \
      "$root/official/blog" \
      "$root/official/templates/aws" \
      "$root/official/templates/azure" \
      "$root/official/templates/basic" \
      "$root/official/templates/business" \
      "$root/official/templates/charts" \
      "$root/official/templates/education" \
      "$root/official/templates/engineering" \
      "$root/official/templates/flowcharts" \
      "$root/official/templates/gcp" \
      "$root/official/templates/ibm-cloud" \
      "$root/official/templates/infographic" \
      "$root/official/templates/layout" \
      "$root/official/templates/maps" \
      "$root/official/templates/network" \
      "$root/official/templates/other" \
      "$root/official/templates/plans" \
      "$root/official/templates/software" \
      "$root/official/templates/tables" \
      "$root/official/templates/uml" \
      "$root/official/templates/venn" \
      "$root/official/templates/world" \
      "$root/representative"; do \
        just drawio-reference "$fixtures"; \
      done

# Compare committed official Draw.io reference with kcf rendering through ImageMagick score
drawio-compare fixtures min_score='99' output='tmp/kcf-drawio' baseline='':
    just drawio-render "{{fixtures}}" "{{output}}/rendered"
    bun run scripts/mermaid/rasterize-svg-dir.ts --input "{{output}}/rendered" --output "{{output}}/rendered-browser"
    @if [ -n "{{baseline}}" ]; then \
      bun run scripts/drawio/reference-compare.ts --official "{{fixtures}}/official" --katana "{{output}}/rendered-browser" --output "{{output}}/comparison" --min-score "{{min_score}}" --baseline "{{baseline}}"; \
    else \
      bun run scripts/drawio/reference-compare.ts --official "{{fixtures}}/official" --katana "{{output}}/rendered-browser" --output "{{output}}/comparison" --min-score "{{min_score}}"; \
    fi

# Compare representative Draw.io patterns for CI/CD
drawio-compare-ci min_score='99':
    just drawio-compare tests/fixtures/drawio/representative "{{min_score}}" tmp/kcf-drawio-ci tests/fixtures/drawio/representative/score-baseline.json

# Compare basic Draw.io patterns as a smoke check
drawio-compare-basic min_score='99':
    just drawio-compare tests/fixtures/drawio/basic "{{min_score}}" tmp/kcf-drawio-basic

# Compare full Draw.io fixture sets for local release validation
drawio-compare-full min_score='99':
    @set -euo pipefail; \
    root="tests/fixtures/drawio"; \
    for fixtures in \
      "$root/basic" \
      "$root/official/diagrams" \
      "$root/official/examples" \
      "$root/official/blog" \
      "$root/official/templates/aws" \
      "$root/official/templates/azure" \
      "$root/official/templates/basic" \
      "$root/official/templates/business" \
      "$root/official/templates/charts" \
      "$root/official/templates/education" \
      "$root/official/templates/engineering" \
      "$root/official/templates/flowcharts" \
      "$root/official/templates/gcp" \
      "$root/official/templates/ibm-cloud" \
      "$root/official/templates/infographic" \
      "$root/official/templates/layout" \
      "$root/official/templates/maps" \
      "$root/official/templates/network" \
      "$root/official/templates/other" \
      "$root/official/templates/plans" \
      "$root/official/templates/software" \
      "$root/official/templates/tables" \
      "$root/official/templates/uml" \
      "$root/official/templates/venn" \
      "$root/official/templates/world"; do \
        slug=${fixtures#tests/fixtures/drawio/}; \
        slug=${slug//\//-}; \
        just drawio-compare "$fixtures" "{{min_score}}" "tmp/kcf-drawio-full/$slug"; \
      done

# Render Draw.io fixtures for a timing smoke check
drawio-bench fixtures:
    @start=$(date +%s); just drawio-render "{{fixtures}}" tmp/kcf-drawio-bench; end=$(date +%s); elapsed=$((end - start)); echo "drawio fixtures rendered in ${elapsed}s"
