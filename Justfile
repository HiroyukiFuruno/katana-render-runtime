set shell := ["bash", "-uc"]

REPO_ROOT := justfile_directory()
RTK := env_var_or_default("RTK", `command -v rtk 2> /dev/null || true`)
RTK_CMD := if RTK == "" { "" } else { RTK + " " }
RTK_PROXY_CMD := if RTK == "" { "" } else { RTK + " proxy " }
JOBS := env_var_or_default("JOBS", "2")
CHECK_JOBS := env_var_or_default("CHECK_JOBS", "3")
FIXTURE_JOBS := env_var_or_default("FIXTURE_JOBS", JOBS)
TEST_THREADS := env_var_or_default("TEST_THREADS", "2")
TEST_THREAD_ARGS := if TEST_THREADS == "" { "" } else { " -- --test-threads=" + TEST_THREADS }
NEXTEST_THREAD_ARGS := if TEST_THREADS == "" { "" } else { " --test-threads=" + TEST_THREADS }
COVERAGE_TEST_ONLY_IGNORE_FILENAME_REGEX := "(^|[/\\\\])scripts[/\\\\]html[/\\\\]performance-harness[/\\\\]src[/\\\\]([^/\\\\]+[/\\\\])*[^/\\\\]+(_tests|_test_support)\\.rs$"
RUNTIME_UPDATE_LOG_DIR := env_var_or_default("RUNTIME_UPDATE_LOG_DIR", "tmp/runtime-update-logs")
export RUSTFLAGS := env_var_or_default("RUSTFLAGS", "-D warnings")
CARGO := env_var_or_default("CARGO", RTK_PROXY_CMD + "cargo")
HTML84_CARGO := if RTK == "" { "cargo" } else { RTK + " proxy cargo" }
CARGO_BUILD := "python3 " + quote(REPO_ROOT + "/scripts/build/cargo_capacity_guard.py") + " --retain-build-profile debug -- " + CARGO
KRR_BIN := env_var_or_default("KRR_BIN", REPO_ROOT + "/target/debug/krr")
export KRR_BUILD_RECEIPT := env_var_or_default("KRR_BUILD_RECEIPT", "")
export MERMAID_RASTER_SESSION_CONFIG := env_var_or_default("MERMAID_RASTER_SESSION_CONFIG", "")
export MERMAID_RASTER_SESSION_CONFIG_SHA := env_var_or_default("MERMAID_RASTER_SESSION_CONFIG_SHA", "")
export MERMAID_RASTER_BINDING_CONFIG := env_var_or_default("MERMAID_RASTER_BINDING_CONFIG", "")
export MERMAID_RASTER_BINDING_CONFIG_SHA := env_var_or_default("MERMAID_RASTER_BINDING_CONFIG_SHA", "")
MERMAID_RASTER_SESSION_CONFIG_EN := env_var_or_default("MERMAID_RASTER_SESSION_CONFIG_EN", "")
MERMAID_RASTER_SESSION_CONFIG_SHA_EN := env_var_or_default("MERMAID_RASTER_SESSION_CONFIG_SHA_EN", "")
MERMAID_RASTER_BINDING_CONFIG_EN := env_var_or_default("MERMAID_RASTER_BINDING_CONFIG_EN", "")
MERMAID_RASTER_BINDING_CONFIG_SHA_EN := env_var_or_default("MERMAID_RASTER_BINDING_CONFIG_SHA_EN", "")
MERMAID_RASTER_SESSION_CONFIG_JA := env_var_or_default("MERMAID_RASTER_SESSION_CONFIG_JA", "")
MERMAID_RASTER_SESSION_CONFIG_SHA_JA := env_var_or_default("MERMAID_RASTER_SESSION_CONFIG_SHA_JA", "")
MERMAID_RASTER_BINDING_CONFIG_JA := env_var_or_default("MERMAID_RASTER_BINDING_CONFIG_JA", "")
MERMAID_RASTER_BINDING_CONFIG_SHA_JA := env_var_or_default("MERMAID_RASTER_BINDING_CONFIG_SHA_JA", "")
HTML_PROBE_BIN := REPO_ROOT + "/target/release/examples/html_fidelity_probe" + if os() == "windows" { ".exe" } else { "" }
HTML_INTERACTION_REPORT := env_var_or_default("HTML_INTERACTION_REPORT", REPO_ROOT + "/tmp/html-interaction-suite/krr.json")
HTML_INTERACTION_OUTPUT := env_var_or_default("HTML_INTERACTION_OUTPUT", REPO_ROOT + "/tmp/html-interaction-suite/chrome")
VERSION := env_var_or_default("VERSION", `awk -F '"' '/^version = / { print $2; exit }' Cargo.toml`)
VERSION_BARE := replace(VERSION, "v", "")
TAG := "v" + VERSION_BARE
RELEASE_REPO := env_var_or_default("RELEASE_REPO", "HiroyukiFuruno/katana-render-runtime")
COVERAGE_MIN_LINES := env_var_or_default("COVERAGE_MIN_LINES", "100")
COVERAGE_MAX_UNCOVERED_LINES := env_var_or_default("COVERAGE_MAX_UNCOVERED_LINES", "0")
MERMAID_JS_VERSION := "12.2.0-krr.pr6823.1"
MERMAID_ZENUML_JS_VERSION := "1.0.1"
DRAWIO_JS_VERSION := "32.6.1"
MATHJAX_JS_VERSION := "4.1.3"
HTML_PERFORMANCE_SCIPY_VERSION := "1.18.1"
HTML_PERFORMANCE_NUMPY_VERSION := "2.5.4"
HTML_PERFORMANCE_SCIPY_ENV := "KRR_HTML_PERFORMANCE_SCIPY_VERSION=" + HTML_PERFORMANCE_SCIPY_VERSION
HTML_PERFORMANCE_TYPING_PATH := "scripts/release/typings"
HTML_PERFORMANCE_PYTHON_REQUIREMENTS := REPO_ROOT + "/scripts/release/html-performance-requirements.lock"
ZENUML_CORE_JS_VERSION := "4.11.0"
PLANTUML_JAR_VERSION := "1.2026.9"
PLANTUML_JAR_CHECKSUM := "1dbb847646f0f440595fecd5f991e902aae47dc3391a01f64bab548089241a46"
MERMAID_JS := env_var_or_default("MERMAID_JS", "crates/katana-render-runtime/vendor/mermaid/" + MERMAID_JS_VERSION + "/mermaid.min.js")
MERMAID_ZENUML_JS := env_var_or_default("MERMAID_ZENUML_JS", "crates/katana-render-runtime/vendor/mermaid-zenuml/" + MERMAID_ZENUML_JS_VERSION + "/mermaid-zenuml.min.js")
DRAWIO_JS := env_var_or_default("DRAWIO_JS", "crates/katana-render-runtime/vendor/drawio/" + DRAWIO_JS_VERSION + "/drawio.min.js")
MATHJAX_JS := env_var_or_default("MATHJAX_JS", "crates/katana-render-runtime/vendor/mathjax/" + MATHJAX_JS_VERSION + "/tex-svg.js")
PLANTUML_JAR_URL := "https://repo1.maven.org/maven2/net/sourceforge/plantuml/plantuml-lgpl/" + PLANTUML_JAR_VERSION + "/plantuml-lgpl-" + PLANTUML_JAR_VERSION + ".jar"
PLANTUML_CACHE_DIR := env_var_or_default("KRR_PLANTUML_CACHE_DIR", `bash scripts/plantuml/cache-dir.sh`)
PLANTUML_CACHE_JAR := PLANTUML_CACHE_DIR + "/" + PLANTUML_JAR_VERSION + "/plantuml.jar"
DRAWIO_RESOURCE_DIR := "crates/katana-render-runtime/src/markdown/drawio_renderer/js_runtime/resources"
DRAWIO_RESOURCE_MANIFEST := DRAWIO_RESOURCE_DIR + "/drawio-resource-manifest.json"

default: help

# Show available tasks
help:
    @just --list --unsorted

# Apply Rust formatting
fmt:
    {{CARGO}} fmt --all
    {{CARGO}} fmt --manifest-path scripts/html/performance-harness/Cargo.toml --all

# Check Rust formatting
fmt-check:
    {{CARGO}} fmt --all -- --check
    just performance-harness-fmt-check

# 配布と同じ最適化ビルドで初期描画は3秒未満、closeとプロセス全体は60秒上限で検証する。
html-first-frame-check input expected_sha256 report="tmp/html-first-frame.json":
    python3 {{quote(REPO_ROOT + "/scripts/build/cargo_capacity_guard.py")}} --retain-build-profile release -- {{CARGO}} build --release --locked -p katana-render-runtime --example html_initial_frame_probe
    python3 scripts/release/verify_html_first_frame.py --input "{{input}}" --expected-sha256 "{{expected_sha256}}" --binary "{{env_var_or_default('CARGO_TARGET_DIR', 'target')}}/release/examples/html_initial_frame_probe" --report "{{report}}"

# Run strict Clippy checks
# WHY: root Clippyの前後で容量を確認し、後続の計測検査前にCOVを保持して再整理する。
lint:
    @set -euo pipefail; \
      retained_profile="{{REPO_ROOT}}/target/llvm-cov-target/krr-coverage"; \
      retained_profile_arg=""; \
      retained_args=(); \
      if [[ -d "$retained_profile" && ! -L "$retained_profile" ]]; then \
        retained_args=(--retain-profile "$retained_profile"); \
        retained_profile_arg="$retained_profile"; \
      fi; \
      just capacity-admission; \
      RUSTFLAGS="-D warnings" CARGO_TARGET_DIR="{{REPO_ROOT}}/target" python3 scripts/build/cargo_capacity_guard.py "${retained_args[@]}" --check-domain root -- {{CARGO}} clippy -j {{JOBS}} --workspace --all-targets --all-features -- -D warnings -D clippy::unwrap_used -D clippy::expect_used -D clippy::todo -D clippy::unimplemented -D clippy::dbg_macro -D clippy::panic -D clippy::wildcard_imports -D clippy::too_many_lines -D clippy::cognitive_complexity; \
      just capacity-admission; \
      just performance-harness-lint "$retained_profile_arg"

# Run Rust syntax based structural checks
ast-lint:
    {{CARGO}} test -j {{JOBS}} -p kdr-linter ast_linter -- --nocapture

# Check that katana-render-runtime does not depend on KatanA UI crates
dependency-leak:
    @dependencies="$({{CARGO}} tree --workspace -e normal)"; \
    pattern='(^|[[:space:]])(egui|katana-core|katana-ui|katana-platform|katana-native)([[:space:]]|$)'; \
    if printf '%s\n' "$dependencies" | grep -E "$pattern"; then \
      echo "KatanA UI dependency leaked into katana-render-runtime." >&2; \
      exit 1; \
    fi

# Run workspace tests
unit-test: plantuml-install
    CARGO_BUILD_JOBS={{quote(JOBS)}} {{CARGO}} nextest run --workspace --all-targets --all-features --locked --no-fail-fast --retries 0 --user-config-file "{{REPO_ROOT}}/scripts/release/nextest-local-user.toml"{{NEXTEST_THREAD_ARGS}}

# Root unit tests and line coverage share one fresh instrumented test run.
native-coverage-cache-key producer:
    CARGO={{quote(CARGO)}} TEST_THREADS={{quote(TEST_THREADS)}} python3 scripts/release/native_coverage_result_cli.py --repo "{{REPO_ROOT}}" local-key --producer {{quote(producer)}} --moon-fingerprint

native-coverage-cache-diagnose producer:
    CARGO={{quote(CARGO)}} TEST_THREADS={{quote(TEST_THREADS)}} python3 scripts/release/native_coverage_result_cli.py --repo "{{REPO_ROOT}}" local-diagnose --producer {{quote(producer)}}

# WHY: 正式coverageの合格とは分け、既存runner候補の全件実行コストを測る。
native-nextest-diagnostic filter="": plantuml-install runtime-bundle-check
    @set -euo pipefail; \
      diagnostic_run_id="$(bun -e 'process.stdout.write(crypto.randomUUID())')"; \
      retain_profile="{{REPO_ROOT}}/target/llvm-cov-target/krr-coverage"; \
      retain_args=(); \
      if [[ -d "$retain_profile" && ! -L "$retain_profile" ]]; then retain_args=(--retain-profile "$retain_profile"); fi; \
      CARGO_BUILD_JOBS={{quote(JOBS)}} NEXTEST_STATE_DIR="{{REPO_ROOT}}/target/nextest-diagnostic-state" LLVM_PROFILE_FILE_NAME="krr-nextest-diagnostic-${diagnostic_run_id}-%4m.profraw" CARGO_TARGET_DIR="{{REPO_ROOT}}/target" CARGO_LLVM_COV_TARGET_DIR="{{REPO_ROOT}}/target/llvm-cov-target" python3 scripts/build/cargo_capacity_guard.py --coverage-diagnostic-domain root "${retain_args[@]}" -- {{CARGO}} llvm-cov nextest --workspace --all-targets --all-features --locked --cargo-profile krr-coverage --no-report --no-fail-fast --retries 0 --user-config-file "{{REPO_ROOT}}/scripts/release/nextest-local-user.toml"{{NEXTEST_THREAD_ARGS}}{{if filter == "" { "" } else { " -E " + quote(filter) }}}

root-test-and-coverage: plantuml-install
    CARGO={{quote(CARGO)}} CARGO_BUILD_JOBS={{quote(JOBS)}} TEST_THREADS={{quote(TEST_THREADS)}} python3 scripts/release/native_nextest_root_coverage.py --repo "{{REPO_ROOT}}"

# WHY: 未計測行の診断用詳細JSONを正式なroot.jsonと分離し、閾値判定には使わない。
native-root-coverage-detail:
    CARGO_TARGET_DIR="{{REPO_ROOT}}/target" CARGO_LLVM_COV_TARGET_DIR="{{REPO_ROOT}}/target/llvm-cov-target" python3 scripts/build/cargo_capacity_guard.py --coverage-diagnostic-domain root -- {{CARGO}} llvm-cov report --workspace --profile krr-coverage --locked --json --show-missing-lines --output-path "{{REPO_ROOT}}/tmp/quality/root-coverage-detail.json"

# 継続利用するPythonは固定版のLintと厳格型検査を両方通す。
python-lint:
    uvx --from ruff==0.16.10 ruff check --select F,E4,E7,E9,B,UP scripts

python-typecheck:
    MYPYPATH={{HTML_PERFORMANCE_TYPING_PATH}} {{HTML_PERFORMANCE_SCIPY_ENV}} uvx --with-requirements {{quote(HTML_PERFORMANCE_PYTHON_REQUIREMENTS)}} --from mypy==2.4.0 mypy --strict scripts

# 所有コードとテストの未承認重複を必須検査として検出する。
duplication-check:
    bun scripts/quality/duplication-check.ts

# harness lockと明示pinの同期を個別に早期確認する。
html-performance-lock-pin-check:
    python3 -m unittest discover -s scripts/release -p verify_html_performance_evidence_test.py -k test_current_harness_lock_matches_explicit_pin --failfast

# 検証コードの管理・解析範囲・標準導線の登録漏れを拒否する。
html-harness-contract-check:
    bun scripts/html/harness-contract-cli.ts

# 所有ソース全体の解析対象と標準テスト登録の欠落を検出する。
owned-validation-contract-check:
    bun scripts/quality/owned-validation-contract.ts
    python3 -m unittest discover -s scripts/mermaid -p native_cli_build_producer_test.py --failfast
    {{CARGO}} metadata --locked --all-features --format-version 1 --manifest-path "{{REPO_ROOT}}/Cargo.toml" > /dev/null
    {{CARGO}} metadata --locked --no-default-features --features baseline --format-version 1 --manifest-path "{{REPO_ROOT}}/scripts/html/performance-harness/Cargo.toml" > /dev/null
    {{CARGO}} metadata --locked --no-default-features --features candidate --format-version 1 --manifest-path "{{REPO_ROOT}}/scripts/html/performance-harness/Cargo.toml" > /dev/null

# Run coverage as a required full-check gate
coverage:
    just root-test-and-coverage
    just performance-harness-coverage
    just coverage-result-check

# WHY: cacheから復元された結果も毎回検証し、不正な結果は実検査で置き換える。
coverage-result-check:
    python3 scripts/release/native_coverage_local_aggregate.py --repo "{{REPO_ROOT}}"

# 性能producerの限定受入でもMoonと同じ環境で復元結果を認証する。
performance-coverage-result-check:
    python3 scripts/release/native_coverage_local_aggregate.py --repo "{{REPO_ROOT}}" --producer performance

# 独立workspaceの両armを個別に検査し、排他featureを同時選択しない。
performance-harness-fmt-check:
    {{CARGO}} fmt --manifest-path scripts/html/performance-harness/Cargo.toml --all -- --check

performance-harness-lint retained_profile="":
    @set -euo pipefail; \
      retained_args=(); \
      if [[ -n {{quote(retained_profile)}} ]]; then retained_args=(--retain-profile {{quote(retained_profile)}}); fi; \
      RUSTFLAGS="-D warnings" CARGO_TARGET_DIR="{{REPO_ROOT}}/target" python3 scripts/build/cargo_capacity_guard.py "${retained_args[@]}" --check-domain performance-baseline -- {{CARGO}} clippy -j {{JOBS}} --manifest-path scripts/html/performance-harness/Cargo.toml --all-targets --no-default-features --features baseline --locked -- -D warnings -D clippy::unwrap_used -D clippy::expect_used -D clippy::todo -D clippy::unimplemented -D clippy::dbg_macro -D clippy::panic -D clippy::wildcard_imports -D clippy::too_many_lines -D clippy::cognitive_complexity; \
      RUSTFLAGS="-D warnings" CARGO_TARGET_DIR="{{REPO_ROOT}}/target" python3 scripts/build/cargo_capacity_guard.py "${retained_args[@]}" --check-domain performance-candidate -- {{CARGO}} clippy -j {{JOBS}} --manifest-path scripts/html/performance-harness/Cargo.toml --all-targets --no-default-features --features candidate --locked -- -D warnings -D clippy::unwrap_used -D clippy::expect_used -D clippy::todo -D clippy::unimplemented -D clippy::dbg_macro -D clippy::panic -D clippy::wildcard_imports -D clippy::too_many_lines -D clippy::cognitive_complexity

# profileは分離し、全出力は同じroot targetの固定aggregate容量へ含める。
performance-harness-coverage:
    CARGO={{quote(CARGO)}} TEST_THREADS={{quote(TEST_THREADS)}} python3 scripts/hooks/run_locked_command.py --performance-coverage

[private]
performance-harness-coverage-run:
    mkdir -p "{{REPO_ROOT}}/tmp/quality/standalone-performance-coverage"
    python3 scripts/release/native_coverage_result_cli.py --repo "{{REPO_ROOT}}" local-begin --producer performance
    CARGO_TARGET_DIR="{{REPO_ROOT}}/target" CARGO_LLVM_COV_TARGET_DIR="{{REPO_ROOT}}/target/llvm-cov-performance-baseline" python3 scripts/build/cargo_capacity_guard.py --coverage-domain performance-baseline -- {{CARGO}} llvm-cov clean --manifest-path scripts/html/performance-harness/Cargo.toml --workspace --locked
    CARGO_TARGET_DIR="{{REPO_ROOT}}/target" CARGO_LLVM_COV_TARGET_DIR="{{REPO_ROOT}}/target/llvm-cov-performance-baseline" python3 scripts/build/cargo_capacity_guard.py --coverage-collection-domain performance-baseline -- {{CARGO}} llvm-cov --manifest-path scripts/html/performance-harness/Cargo.toml --workspace --all-targets --no-default-features --features baseline --locked --profile krr-coverage --ignore-filename-regex {{quote(COVERAGE_TEST_ONLY_IGNORE_FILENAME_REGEX)}} --json --output-path "{{REPO_ROOT}}/tmp/quality/standalone-performance-coverage/baseline.json"{{TEST_THREAD_ARGS}}
    CARGO_TARGET_DIR="{{REPO_ROOT}}/target" CARGO_LLVM_COV_TARGET_DIR="{{REPO_ROOT}}/target/llvm-cov-performance-candidate" python3 scripts/build/cargo_capacity_guard.py --coverage-domain performance-candidate -- {{CARGO}} llvm-cov clean --manifest-path scripts/html/performance-harness/Cargo.toml --workspace --locked
    CARGO_TARGET_DIR="{{REPO_ROOT}}/target" CARGO_LLVM_COV_TARGET_DIR="{{REPO_ROOT}}/target/llvm-cov-performance-candidate" python3 scripts/build/cargo_capacity_guard.py --coverage-collection-domain performance-candidate -- {{CARGO}} llvm-cov --manifest-path scripts/html/performance-harness/Cargo.toml --workspace --all-targets --no-default-features --features candidate --locked --profile krr-coverage --ignore-filename-regex {{quote(COVERAGE_TEST_ONLY_IGNORE_FILENAME_REGEX)}} --json --output-path "{{REPO_ROOT}}/tmp/quality/standalone-performance-coverage/candidate.json"{{TEST_THREAD_ARGS}}
    python3 scripts/release/native_coverage_result_cli.py --repo "{{REPO_ROOT}}" local-collect-finish --producer performance --producer-exit 0
    python3 scripts/release/native_coverage_result_cli.py --repo "{{REPO_ROOT}}" local-collection-validate --producer performance

# Verify pinned runtime asset checksums
runtime-asset-check:
    cd crates/katana-render-runtime/vendor/mermaid/{{MERMAID_JS_VERSION}} && shasum -a 256 -c mermaid.min.js.sha256
    cd crates/katana-render-runtime/vendor/mermaid-zenuml/{{MERMAID_ZENUML_JS_VERSION}} && shasum -a 256 -c mermaid-zenuml.min.js.sha256
    cd crates/katana-render-runtime/vendor/drawio/{{DRAWIO_JS_VERSION}} && shasum -a 256 -c drawio.min.js.sha256
    cd crates/katana-render-runtime/vendor/mathjax/{{MATHJAX_JS_VERSION}} && shasum -a 256 -c tex-svg.js.sha256
    cd crates/katana-render-runtime/vendor/zenuml-core/{{ZENUML_CORE_JS_VERSION}} && shasum -a 256 -c zenuml.js.sha256
    @grep -qx "{{PLANTUML_JAR_CHECKSUM}}  plantuml.jar" crates/katana-render-runtime/vendor/plantuml/{{PLANTUML_JAR_VERSION}}/plantuml.jar.sha256
    @if [ -f "{{PLANTUML_CACHE_JAR}}" ]; then cd "$(dirname "{{PLANTUML_CACHE_JAR}}")" && shasum -a 256 -c "{{REPO_ROOT}}/crates/katana-render-runtime/vendor/plantuml/{{PLANTUML_JAR_VERSION}}/plantuml.jar.sha256"; fi
    cd crates/katana-render-runtime/src/markdown/diagram_runtime/generated && shasum -a 256 -c runtime-bundles.sha256

# Generate TypeScript-managed diagram runtime bundles
runtime-bundle-build:
    bun run runtime-bundle:build

# Verify generated diagram runtime bundles are synced with TypeScript source
runtime-bundle-check:
    bun run runtime-bundle:check
    bun scripts/runtime-bundles/parser-body-onload-generation.ts --check

# Run TypeScript formatter/linter gate
biome:
    bun run moon exec krr:biome

# 準備と検査を同じJust環境で行い、古いhostを受理しない。
[private]
biome-run:
    bun scripts/runtime-bundles/runtime-rust-facts-host.ts verify {{quote(REPO_ROOT)}} {{quote(REPO_ROOT + "/tmp/quality/rust-facts-host/current/receipt.json")}}
    bun scripts/runtime-bundles/runtime-unused-check.ts --gate

# Run strict TypeScript compiler checks
typecheck:
    bun run typecheck

# Verify generated runtime bundles are included in the library crate package
runtime-bundle-package-check:
    @package_files="$({{CARGO}} package -p katana-render-runtime --locked --allow-dirty --list)"; \
    for file in \
      "src/markdown/diagram_runtime/generated/mermaid-runtime.min.js" \
      "src/markdown/diagram_runtime/generated/drawio-runtime.min.js" \
      "src/markdown/diagram_runtime/generated/mathjax-runtime.min.js" \
      "src/markdown/diagram_runtime/generated/zenuml-runtime.min.js" \
      "src/markdown/diagram_runtime/generated/runtime-bundles.sha256"; do \
        if ! printf '%s\n' "$package_files" | grep -qx "$file"; then \
          echo "missing runtime bundle package file: $file" >&2; \
          exit 1; \
        fi; \
      done; \
    if ! printf '%s\n' "$package_files" | grep -qx "src/markdown/drawio_renderer/generated/drawio-resources.bin.br"; then \
      echo "missing compressed Draw.io resource archive" >&2; \
      exit 1; \
    fi; \
    if ! printf '%s\n' "$package_files" | grep -qx "src/markdown/generated/zenuml-runtime-assets.bin.br"; then \
      echo "missing compressed ZenUML runtime asset archive" >&2; \
      exit 1; \
    fi; \
    if printf '%s\n' "$package_files" | grep -q "src/markdown/drawio_renderer/js_runtime/resources/img/"; then \
      echo "raw Draw.io image resources must not be included in the crates.io package" >&2; \
      exit 1; \
    fi

# Verify the in-process HTML runtime source is included in the library crate package
html-runtime-package-check:
    @package_files="$({{CARGO}} package -p katana-render-runtime --locked --allow-dirty --list)"; \
    for required in \
      "src/renderer/backends/html_runtime/dom_bootstrap.js" \
      "assets/test-fixtures/font-probes/recursive-latin-crsv-normal.ttf" \
      "assets/test-fixtures/font-probes/native-class-ja-probe.svg" \
      "assets/test-fixtures/font-probes/licenses/Recursive-OFL-1.1.txt"; do \
      if ! printf '%s\n' "$package_files" | grep -qx "$required"; then \
        echo "missing HTML runtime package file: $required" >&2; \
        exit 1; \
      fi; \
    done

# Verify the PlantUML checksum manifest is included without packaging the JAR body
plantuml-runtime-package-check:
    @package_files="$({{CARGO}} package -p katana-render-runtime --locked --allow-dirty --list)"; \
    if ! printf '%s\n' "$package_files" | grep -qx "vendor/plantuml/{{PLANTUML_JAR_VERSION}}/plantuml.jar.sha256"; then \
      echo "missing PlantUML checksum manifest package file" >&2; \
      exit 1; \
    fi; \
    if printf '%s\n' "$package_files" | grep -qx "vendor/plantuml/{{PLANTUML_JAR_VERSION}}/plantuml.jar"; then \
      echo "PlantUML JAR body must not be included in the crates.io package" >&2; \
      exit 1; \
    fi

# Run TypeScript tests for runtime asset helper scripts
runtime-asset-script-test:
    bun run moon exec krr:runtime-asset-script-test

runtime-asset-script-aggregate-run:
    bun scripts/runtime-bundles/runtime-unused-check.ts --gate

runtime-asset-script-preflight-run:
    bun scripts/runtime-bundles/runtime-rust-facts-host.ts verify {{ quote(REPO_ROOT) }} {{ quote(REPO_ROOT + "/tmp/quality/rust-facts-host/current/receipt.json") }}
    NODE_ENV=test bun scripts/runtime-bundles/runtime-mathjax-audit-evidence.ts verify

mathjax-import-pin-preflight:
    NODE_ENV=test bun scripts/runtime-bundles/runtime-mathjax-audit-evidence.ts preflight

runtime-classic-generation-evidence:
    bun scripts/runtime-bundles/bundle-runtime.ts --check --classic-generation-evidence

runtime-asset-script-other-run:
    bun test --path-ignore-patterns 'tmp/**' ./scripts/runtime-assets/svg-engine-provenance_test.ts
    bun test --path-ignore-patterns 'tmp/**' ./scripts/runtime-assets/runtime-asset-common_test.ts ./scripts/runtime-assets/update_test.ts ./scripts/runtime-assets/latest-check_test.ts ./scripts/runtime-assets/update_zenuml_test.ts ./scripts/runtime-assets/depends-update-all_test.ts ./scripts/runtime-assets/runtime-package-asset-compressor_test.ts ./scripts/runtime-assets/mermaid-patched-build-recipe_test.ts ./scripts/mermaid/official_renderer_i18n_test.ts ./scripts/mermaid/diagram_update_markdown_assets_test.ts
    bun test --path-ignore-patterns 'tmp/**' ./scripts/runtime-bundles/runtime-rust-facts-host_test.ts ./scripts/runtime-bundles/runtime-rust-facts-host-freshness_test.ts ./scripts/runtime-bundles/runtime-mathjax-audit-evidence_test.ts
    python3 scripts/quality/moon_cache_contract_test.py --moon bun run moon
    bun test --path-ignore-patterns 'tmp/**' ./scripts/drawio/reference_score_test.ts ./scripts/drawio/official-runtime-determinism_test.ts ./scripts/drawio/official-source-fonts_test.ts
    bun test --path-ignore-patterns 'tmp/**' ./scripts/html/reference-capture_test.ts ./scripts/html/fidelity-suite_test.ts ./scripts/html/scroll-coverage_test.ts ./scripts/html/toc-fidelity_test.ts
    bun test --path-ignore-patterns 'tmp/**' ./scripts/html/interaction-suite_test.ts
    bun test --path-ignore-patterns 'tmp/**' ./scripts/html/visual-state-fidelity_test.ts ./scripts/html/dynamic-fidelity_test.ts ./scripts/html/interim-feature-results_test.ts
    bun test --path-ignore-patterns 'tmp/**' ./scripts/html/harness-contract_test.ts ./scripts/html/stage-score_test.ts ./scripts/html/stage-score-suite_test.ts ./scripts/html/catalog-font-equivalence_test.ts ./scripts/html/chromium-font-trace_test.ts ./scripts/html/catalog-capture-producer-context_test.ts ./scripts/html/anonymized-fixture_test.ts ./scripts/html/compare-toc-evidence_test.ts
    bun test --path-ignore-patterns 'tmp/**' ./crates/katana-render-runtime/src/renderer/backends/html_runtime/dom_bootstrap_test.js ./scripts/mermaid/class_reference_gate_test.ts ./scripts/mermaid/class_reference_integrity_test.ts ./scripts/mermaid/diagram_update_fixtures_test.ts ./scripts/mermaid/diagram_update_markdown_reference_test.ts ./scripts/mermaid/dom_css_font_variables_contract_test.ts ./scripts/mermaid/dom_css_priority_contract_test.ts ./scripts/mermaid/dom_plain_text_measure_contract_test.ts ./scripts/mermaid/dom_svg_bbox_visibility_contract_test.ts ./scripts/mermaid/managed_full_compare_test.ts ./scripts/mermaid/mermaid_pair_manifest_test.ts ./scripts/mermaid/native-cli-process_test.ts ./scripts/mermaid/native-source-manifest_test.ts ./scripts/mermaid/native_render_provenance_test.ts ./scripts/mermaid/native_frame_build_proof_test.ts ./scripts/mermaid/native_frame_evidence_test.ts ./scripts/mermaid/native_frame_capture_context_test.ts ./scripts/mermaid/native-render-batch_test.ts ./scripts/mermaid/official_reference_mode_test.ts ./scripts/mermaid/official_raw_metrics_diagnostic_test.ts ./scripts/mermaid/official_renderer_provenance_test.ts ./scripts/mermaid/raster_comparison_binding_test.ts ./scripts/mermaid/raster_producer_observation_test.ts ./scripts/mermaid/raster_production_binding_test.ts ./scripts/mermaid/rasterize_svg_fonts_test.ts ./scripts/mermaid/rasterize_svg_input_test.ts ./scripts/mermaid/reference_pair_repository_test.ts ./scripts/mermaid/reference_pair_strict_provenance_test.ts ./scripts/mermaid/reference_score_policy_test.ts ./scripts/mermaid/reference_svg_geometry_test.ts ./scripts/mermaid/split_markdown_fixtures_test.ts ./scripts/mermaid/strict_svg_external_images_test.ts ./scripts/mermaid/svg_normalizer_general_contract_test.ts ./scripts/mermaid/svg_sequence_activation_fixes_test.ts ./scripts/quality/duplication-check_test.ts ./scripts/mermaid/native_product_score_test.ts ./scripts/runtime-bundles/runtime-bundle-checks_test.ts
    bun test --path-ignore-patterns 'tmp/**' ./scripts/mermaid/native_classic_capture_test.ts
    bun test --path-ignore-patterns 'tmp/**' ./scripts/mermaid/native_cli_managed_build_test.ts
    bun test --path-ignore-patterns 'tmp/**' ./scripts/mermaid/native_cli_managed_build_reader_test.ts
    KRR_CLASSIC_GENERATION_EVIDENCE_PATH={{quote(REPO_ROOT + "/tmp/quality/classic-generation-evidence/current.json")}} bun test --path-ignore-patterns 'tmp/**' ./scripts/runtime-bundles/runtime-unused-classic-capture_test.ts ./scripts/runtime-bundles/runtime-unused-classic-binding_test.ts ./scripts/runtime-bundles/runtime-classic-generation_test.ts ./scripts/runtime-bundles/runtime-unused-semantic-adapter_test.ts
    bun test --path-ignore-patterns 'tmp/**' ./scripts/runtime-bundles/parser-body-onload-generation.test.ts
    just browser-contract-test
    bun test --path-ignore-patterns 'tmp/**' ./scripts/quality/owned-source-inventory_test.ts ./scripts/quality/owned-validation-contract_test.ts

# WHY: 関連leafだけの診断再実行であり、full quality gateの代替ではない。
runtime-unused-script-regression:
    bun run moon exec krr:runtime-unused-script-regression

runtime-unused-script-regression-run:
    bun test --path-ignore-patterns 'tmp/**' ./scripts/runtime-bundles/runtime-fragment-loader_test.ts ./scripts/runtime-bundles/runtime-bundle-definitions_test.ts ./scripts/runtime-bundles/runtime-unused-check_test.ts ./scripts/runtime-bundles/runtime-bundle-tool-fingerprint_test.ts ./scripts/runtime-bundles/runtime-unused-mathjax_test.ts ./scripts/runtime-bundles/runtime-unused-native-width-hash_test.ts ./scripts/runtime-bundles/runtime-unused-rust-classic_test.ts

runtime-script-cas-diagnostic-run:
    python3 -m unittest discover -s scripts/quality -p runtime_script_moon_cas_test.py --failfast
    python3 scripts/quality/runtime_script_moon_cas_diagnostic.py --moon bun run moon

runtime-unused-script-producer-run producer="runtime-unused-script-regression":
    python3 scripts/release/runtime_script_cache_cli.py run --repo {{ quote(REPO_ROOT) }} --producer {{ quote(producer) }}

runtime-unused-script-verify-run producer="runtime-unused-script-regression":
    python3 scripts/release/runtime_script_cache_cli.py ensure --repo {{ quote(REPO_ROOT) }} --producer {{ quote(producer) }}

runtime-unused-script-cache-key producer="runtime-unused-script-regression":
    python3 scripts/release/runtime_script_cache_cli.py key --repo {{ quote(REPO_ROOT) }} --producer {{ quote(producer) }}

runtime-unused-script-final-verify-run:
    python3 scripts/release/runtime_script_cache_cli.py ensure-all --repo {{ quote(REPO_ROOT) }}

runtime-unused-fragment-loader-run:
    bun test --path-ignore-patterns 'tmp/**' ./scripts/runtime-bundles/runtime-fragment-loader_test.ts

runtime-unused-bundle-definitions-run:
    bun test --path-ignore-patterns 'tmp/**' ./scripts/runtime-bundles/runtime-bundle-definitions_test.ts

runtime-unused-check-run:
    bun test --path-ignore-patterns 'tmp/**' ./scripts/runtime-bundles/runtime-unused-check_test.ts

runtime-unused-tool-fingerprint-run:
    bun test --path-ignore-patterns 'tmp/**' ./scripts/runtime-bundles/runtime-bundle-tool-fingerprint_test.ts

runtime-unused-mathjax-run:
    bun test --path-ignore-patterns 'tmp/**' ./scripts/runtime-bundles/runtime-unused-mathjax_test.ts

runtime-unused-native-width-run:
    bun test --path-ignore-patterns 'tmp/**' ./scripts/runtime-bundles/runtime-unused-native-width-hash_test.ts

runtime-unused-rust-classic-run:
    bun test --path-ignore-patterns 'tmp/**' ./scripts/runtime-bundles/runtime-unused-rust-classic_test.ts

# WHY: ブラウザ起動の管理は既存fixtureへ任せ、テスト本体の5秒制限を維持する。
browser-contract-test:
    node node_modules/playwright/cli.js test --config scripts/html/playwright.config.ts

# Verify the finite MathJax Rust consumer route with the pinned syn AST parser.
mathjax-consumer-facts root=REPO_ROOT:
    @CARGO_TARGET_DIR={{quote(root + "/target")}} bun {{quote(root + "/scripts/runtime-bundles/runtime-rust-facts-host.ts")}} run {{quote(root)}} mathjax {{quote(root)}} {{quote(root + "/tmp/quality/rust-facts-host/current/receipt.json")}}

# Verify the finite NativeWidth hash route with the pinned syn AST parser.
native-width-hash-facts root=REPO_ROOT source_root=REPO_ROOT:
    @CARGO_TARGET_DIR={{quote(root + "/target")}} bun {{quote(root + "/scripts/runtime-bundles/runtime-rust-facts-host.ts")}} run {{quote(root)}} native-width {{quote(source_root)}} {{quote(root + "/tmp/quality/rust-facts-host/current/receipt.json")}}

# 同じconsumerの検査は準備済みhostの確認を共有し、両方の実Rust検査を維持する。
native-width-classic-facts root=REPO_ROOT source_root=REPO_ROOT:
    @CARGO_TARGET_DIR={{quote(root + "/target")}} bun {{quote(root + "/scripts/runtime-bundles/runtime-rust-facts-host.ts")}} run {{quote(root)}} native-width-classic {{quote(source_root)}} {{quote(root + "/tmp/quality/rust-facts-host/current/receipt.json")}}

# Verify the finite Mermaid classic-script source route with the pinned syn AST parser.
mermaid-classic-script-facts root=REPO_ROOT source_root=REPO_ROOT:
    @CARGO_TARGET_DIR={{quote(root + "/target")}} bun {{quote(root + "/scripts/runtime-bundles/runtime-rust-facts-host.ts")}} run {{quote(root)}} classic {{quote(source_root)}} {{quote(root + "/tmp/quality/rust-facts-host/current/receipt.json")}}

rust-facts-host-prebuild:
    bun scripts/runtime-bundles/runtime-rust-facts-host.ts prepare {{quote(REPO_ROOT)}} {{quote(REPO_ROOT + "/tmp/quality/rust-facts-host/current/receipt.json")}}

# Admit the canonical Cargo target before the host prebuild without starting a child.
capacity-admission:
    @set -euo pipefail; \
      retained_profile="{{REPO_ROOT}}/target/llvm-cov-target/krr-coverage"; \
      retained_args=(); \
      if [[ -d "$retained_profile" && ! -L "$retained_profile" ]]; then \
        retained_args=(--retain-profile "$retained_profile"); \
      fi; \
      python3 scripts/build/cargo_capacity_guard.py --root "{{REPO_ROOT}}" --target "{{REPO_ROOT}}/target" --state "{{REPO_ROOT}}/tmp/.cargo-capacity-guard.json" --require-capacity --print-json "${retained_args[@]}"

mathjax-owned-evidence:
    NODE_ENV=test bun scripts/runtime-bundles/runtime-mathjax-audit-evidence.ts

rust-facts-host-build root=REPO_ROOT:
    @CARGO_TARGET_DIR={{quote(root + "/target")}} {{CARGO_BUILD}} build --locked -p kdr-linter --bin mathjax_consumer_facts

rust-facts-host-metadata root=REPO_ROOT:
    @CARGO_TARGET_DIR={{quote(root + "/target")}} {{CARGO}} metadata --locked --format-version 1

# Compare every input-state boundary operation against Chrome at the immutable 99 threshold.
html-input-state-fidelity-check:
    mkdir -p tmp/html-fidelity/input-state
    {{RTK_PROXY_CMD}}cargo test --release -p katana-render-runtime --lib --no-run --locked --message-format=json > tmp/html-fidelity/input-state/test-build.jsonl
    bun scripts/html/fidelity-acceptance-cli.ts input-state-final tmp/html-fidelity/input-state/test-build.jsonl

# Interim input-state release gate; final fidelity remains fixed at 99.
html-input-state-fidelity-interim-check:
    mkdir -p tmp/html-fidelity/input-state-interim
    {{RTK_PROXY_CMD}}cargo test --release -p katana-render-runtime --lib --no-run --locked --message-format=json > tmp/html-fidelity/input-state-interim/test-build.jsonl
    bun scripts/html/fidelity-acceptance-cli.ts input-state-interim tmp/html-fidelity/input-state-interim/test-build.jsonl

# Compare every script-state case against Chrome at the immutable 99 threshold.
html-dynamic-fidelity-check:
    mkdir -p tmp/html-fidelity/dynamic
    {{RTK_PROXY_CMD}}cargo test --release -p katana-render-runtime --lib --no-run --locked --message-format=json > tmp/html-fidelity/dynamic/test-build.jsonl
    bun scripts/html/fidelity-acceptance-cli.ts dynamic-final tmp/html-fidelity/dynamic/test-build.jsonl

# Interim dynamic-state release gate; final script-feature results remain fixed at 99.
html-dynamic-fidelity-interim-check:
    mkdir -p tmp/html-fidelity/dynamic-interim
    {{RTK_PROXY_CMD}}cargo test --release -p katana-render-runtime --lib --no-run --locked --message-format=json > tmp/html-fidelity/dynamic-interim/test-build.jsonl
    bun scripts/html/fidelity-acceptance-cli.ts dynamic-interim tmp/html-fidelity/dynamic-interim/test-build.jsonl

html-fidelity-probe-build:
    python3 {{quote(REPO_ROOT + "/scripts/build/cargo_capacity_guard.py")}} --retain-build-profile release -- {{CARGO}} build --release --locked -p katana-render-runtime --example html_fidelity_probe

# Compare independent KRR frames with the installed Google Chrome without resizing.
html-diagram-consumer-check phase output:
    @run_dir={{quote(output)}}/"$(bun -e 'process.stdout.write(crypto.randomUUID())')"; \
      for fixture in mermaid-class-diagram drawio-class-diagram; do \
        bun scripts/html/fidelity-suite.ts --binary {{quote(HTML_PROBE_BIN)}} --input "tests/fixtures/html-diagram-consumer/$fixture.html" --output "$run_dir/$fixture" --phase {{quote(phase)}} --min-score {{if phase == "interim" { "90" } else { "99" }}} || exit "$?"; \
      done

html-fidelity-check: html-fidelity-probe-build
    mkdir -p tmp/html-fidelity
    {{RTK_PROXY_CMD}}cargo test --release -p katana-render-runtime --lib --no-run --locked --message-format=json > tmp/html-fidelity/toc-test-build.jsonl
    @status=0; \
      just browser-contract-test || status=1; \
      bun test --path-ignore-patterns 'tmp/**' ./scripts/html/visual-state-fidelity_test.ts || status=1; \
      bun scripts/html/fidelity-suite.ts --binary {{quote(HTML_PROBE_BIN)}} --toc-build-report tmp/html-fidelity/toc-test-build.jsonl --min-score 99 || status=1; \
      just html-diagram-consumer-check final tmp/html-fidelity/diagram-consumer || status=1; \
      if [[ "$status" -eq 0 ]]; then just html-dynamic-fidelity-check || status=1; fi; \
      if [[ "$status" -eq 0 ]]; then just html-input-state-fidelity-check || status=1; fi; \
      if [[ "$status" -eq 0 ]]; then bun scripts/html/fidelity-acceptance-cli.ts reports-final || status=1; fi; \
      exit "$status"

# Explicit v0.4.24 interim release; the final recipe above retains its 99 gate.
html-fidelity-interim-check: html-fidelity-probe-build
    mkdir -p tmp/html-fidelity/interim
    {{RTK_PROXY_CMD}}cargo test --release -p katana-render-runtime --lib --no-run --locked --message-format=json > tmp/html-fidelity/interim/toc-test-build.jsonl
    @status=0; \
      just browser-contract-test || status=1; \
      bun test --path-ignore-patterns 'tmp/**' ./scripts/html/interim-feature-results_test.ts || status=1; \
      bun scripts/html/fidelity-suite.ts --binary {{quote(HTML_PROBE_BIN)}} --toc-build-report tmp/html-fidelity/interim/toc-test-build.jsonl --output tmp/html-fidelity/interim --phase interim --min-score 90 || status=1; \
      just html-diagram-consumer-check interim tmp/html-fidelity/interim/diagram-consumer || status=1; \
      if [[ "$status" -eq 0 ]]; then just html-dynamic-fidelity-interim-check || status=1; fi; \
      if [[ "$status" -eq 0 ]]; then just html-input-state-fidelity-interim-check || status=1; fi; \
      if [[ "$status" -eq 0 ]]; then bun scripts/html/fidelity-acceptance-cli.ts reports-interim || status=1; fi; \
      exit "$status"

# Select from the committed release authorization and package version; no threshold override.
html-release-fidelity-check:
    @recipe="$(python3 scripts/release/html_fidelity_policy.py --format recipe)" && just "$recipe"

# Run the independent KRR snapshot interaction probe and compare it with real Google Chrome.
# Interaction evidence supplements the final 99-point visual and performance contract.
html-interaction-check:
    mkdir -p "$(dirname '{{HTML_INTERACTION_REPORT}}')" "{{HTML_INTERACTION_OUTPUT}}"
    KRR_HTML_INTERACTION_REPORT="{{HTML_INTERACTION_REPORT}}" {{CARGO}} test --release --locked -p katana-render-runtime --lib interaction_state_is_derived_from_the_runtime_snapshot -- --nocapture
    bun scripts/html/interaction-suite.ts --html "{{REPO_ROOT}}/tests/fixtures/html/interaction-fidelity.html" --krr-report "{{HTML_INTERACTION_REPORT}}" --output "{{HTML_INTERACTION_OUTPUT}}" --max-performance-ms 3000
    @echo "HTML interaction evidence completed; the final 99-point visual and performance gates still apply."

[private]
runtime-package-asset-check:
    bun run scripts/runtime-assets/svg-engine-provenance.ts "{{REPO_ROOT}}"
    bun run scripts/runtime-assets/runtime-package-asset-compressor.ts --check
    @package_files="$({{CARGO}} package -p katana-render-runtime --locked --allow-dirty --list)"; \
    for kind_file in "mermaid/{{MERMAID_JS_VERSION}}/mermaid.min.js"; do \
        if ! printf '%s\n' "$package_files" | grep -qx "vendor/$kind_file.br"; then \
          echo "missing compressed runtime package asset: vendor/$kind_file.br" >&2; \
          exit 1; \
        fi; \
        if printf '%s\n' "$package_files" | grep -qx "vendor/$kind_file"; then \
          echo "raw runtime asset must not be included in the crates.io package: vendor/$kind_file" >&2; \
          exit 1; \
        fi; \
      done
    @runtime_package_files="$({{CARGO}} package -p katana-render-runtime --locked --allow-dirty --list)"; \
    asset_package_files="$({{CARGO}} package -p katana-render-runtime-assets --locked --allow-dirty --list)"; \
    if ! printf '%s\n' "$runtime_package_files" | grep -qx "vendor/drawio/{{DRAWIO_JS_VERSION}}/LICENSE"; then \
      echo "missing Draw.io license from runtime crate package" >&2; exit 1; \
    fi; \
    if ! printf '%s\n' "$asset_package_files" | grep -qx "assets/drawio.min.js.br"; then \
      echo "missing compressed Draw.io asset package file" >&2; exit 1; \
    fi; \
    if ! printf '%s\n' "$asset_package_files" | grep -qx "assets/licenses/drawio/LICENSE"; then \
      echo "missing Draw.io license from assets crate package" >&2; exit 1; \
    fi; \
    if printf '%s\n' "$runtime_package_files" | grep -Eq '^vendor/drawio/{{DRAWIO_JS_VERSION}}/drawio\.min\.js(\.br)?$'; then \
      echo "Draw.io source and compressed asset belong to the assets package" >&2; exit 1; \
    fi

# Verify repository hook and release cleanup contracts
automation-contract-test:
    python3 -m unittest discover -s scripts/hooks -p '*_test.py' --failfast
    {{HTML_PERFORMANCE_SCIPY_ENV}} uv run --isolated --with-requirements {{quote(HTML_PERFORMANCE_PYTHON_REQUIREMENTS)}} python scripts/release/run_html_performance.py test
    python3 -m unittest discover -s scripts/quality -p '*_test.py' --failfast
    python3 -m unittest discover -s scripts/mermaid -p '*_test.py' --failfast
    python3 -m unittest discover -s scripts/validation-timing -p test_measure.py --failfast

# 必要時の要求適合レビューを明示実行し、DraftのPRレビューとの重複を避ける。
local-review:
    @export COVERAGE_MIN_LINES={{quote(COVERAGE_MIN_LINES)}} COVERAGE_MAX_UNCOVERED_LINES={{quote(COVERAGE_MAX_UNCOVERED_LINES)}} \
      TEST_THREADS={{quote(TEST_THREADS)}} RUSTFLAGS={{quote(RUSTFLAGS)}} CARGO={{quote(CARGO)}} JOBS={{quote(JOBS)}} CHECK_JOBS={{quote(CHECK_JOBS)}}; \
      python3 scripts/hooks/local_review.py

# Draft前後の品質確認は維持し、初回AIレビューはDraftのPR上で実行する。
draft-review: check

# Run independent local quality-gate lanes concurrently. Cargo and PlantUML
# work stay in one lane because they share build/cache outputs.
# 指摘修正ごとの重複を避け、AIレビューは専用targetから明示実行する。
check:
    @export COVERAGE_MIN_LINES={{quote(COVERAGE_MIN_LINES)}} COVERAGE_MAX_UNCOVERED_LINES={{quote(COVERAGE_MAX_UNCOVERED_LINES)}} \
      TEST_THREADS={{quote(TEST_THREADS)}} RUSTFLAGS={{quote(RUSTFLAGS)}} CARGO={{quote(CARGO)}} JOBS={{quote(JOBS)}} CHECK_JOBS={{quote(CHECK_JOBS)}}; \
      python3 scripts/hooks/run_parallel_checks.py --jobs "$CHECK_JOBS"
    @echo "checks passed"

# Cargo operations share target/ and the PlantUML cache, so keep them ordered.
check-rust:
    just fmt-check
    just ast-lint
    just lint
    just unit-test
    just dependency-leak
    just runtime-asset-check
    just runtime-package-asset-check
    just runtime-bundle-package-check
    just html-runtime-package-check
    just plantuml-runtime-package-check

# 実解析の準備も標準Moon経路に通し、検査中の暗黙ビルドを防ぐ。
check-assets:
    just runtime-bundle-check
    just runtime-asset-script-test
    just biome
    just typecheck

# Python contract suites use isolated temporary repositories.
check-contracts:
    just automation-contract-test
    just build-capacity-check

# 容量超過・使用中lock・実Cargo targetの安全条件を独立projectで検証する。
build-capacity-check:
    {{RTK_PROXY_CMD}}python3 -m unittest discover -s scripts/build -p 'cargo_capacity_guard*test.py' -v

# Sweep old build artifacts locally (older than 7 days)
sweep:
    python3 scripts/hooks/run_locked_command.py --protect-root-producer -- {{CARGO}} sweep --time 7

# Remove build artifacts
clean:
    python3 scripts/hooks/run_locked_command.py --protect-root-producer -- {{CARGO}} clean

# Verify VERSION follows the remote release line
release-target-check:
    bash scripts/release/verify-version.sh "{{VERSION}}"
    python3 scripts/release/verify_dependency_freshness.py
    python3 scripts/release/verify-release-target.py --target-version "{{VERSION}}" --repo "{{RELEASE_REPO}}"
    bash scripts/release/assert-tag-safe.sh "{{TAG}}" origin
    bash scripts/release/assert-crates-not-published.sh "{{VERSION}}"

# Show or explicitly synchronize the checked-in immutable release-content manifest.
# The normal release target check continues to reject a mismatch until this source
# change is reviewed and committed.
release-target-manifest:
    python3 scripts/release/verify-release-target.py --target-version "{{VERSION}}" --head-ref HEAD --print-release-manifest

release-target-manifest-update:
    python3 scripts/release/verify-release-target.py --target-version "{{VERSION}}" --head-ref HEAD --update-release-manifest

# 未公開のworkspace依存を含め、Cargoの検証を通した配布packageを揃える。
release-verify: html-performance-nonregression-check release-target-check
    bash scripts/release/verify-version.sh "{{VERSION}}"
    bash scripts/release/verify-internal-dependencies.sh "{{VERSION}}"
    {{CARGO}} package -p katana-render-runtime-assets -p katana-render-runtime -p katana-render-runtime-cli --locked --allow-dirty
    python3 scripts/release/test_packaged_runtime.py --version "{{VERSION_BARE}}" --cargo {{quote(CARGO)}} {{if TEST_THREADS == "" { "" } else { "--test-threads " + TEST_THREADS }}}
    @asset_files="$({{CARGO}} package -p katana-render-runtime-assets --locked --allow-dirty --list)"; \
    if ! printf '%s\n' "$asset_files" | grep -qx "assets/drawio.min.js.br"; then \
      echo "missing compressed Draw.io asset from assets crate package" >&2; exit 1; \
    fi; \
    if ! printf '%s\n' "$asset_files" | grep -qx "assets/licenses/drawio/LICENSE"; then \
      echo "missing Draw.io license from assets crate package" >&2; exit 1; \
    fi
    @runtime_files="$({{CARGO}} package -p katana-render-runtime --locked --allow-dirty --list)"; \
    for file in "vendor/drawio/{{DRAWIO_JS_VERSION}}/drawio.min.js.sha256" "vendor/drawio/{{DRAWIO_JS_VERSION}}/LICENSE"; do \
      if ! printf '%s\n' "$runtime_files" | grep -qx "$file"; then echo "missing runtime source/checksum package file: $file" >&2; exit 1; fi; \
    done; \
    if printf '%s\n' "$runtime_files" | grep -Eq '^vendor/drawio/{{DRAWIO_JS_VERSION}}/drawio\.min\.js(\.br)?$'; then \
      echo "Draw.io source and compressed asset must not be duplicated in the runtime crate" >&2; exit 1; \
    fi
    bash scripts/release/verify-crate-size.sh katana-render-runtime "{{VERSION}}"
    bash scripts/release/verify-crate-size.sh katana-render-runtime-assets "{{VERSION}}"
    bash scripts/release/verify-crate-size.sh katana-render-runtime-cli "{{VERSION}}"

# Verify completed OpenSpec changes are archived before release PRs
release-openspec-archive:
    bash scripts/release/check-openspec-release-archive.sh --self-test
    bash scripts/release/check-openspec-release-archive.sh "{{VERSION}}"

# 同一committed sourceの3幅・実測時間/RSSを再集計し、劣化時は公開を止める。
html-performance-nonregression-check:
    {{HTML_PERFORMANCE_SCIPY_ENV}} uv run --isolated --with-requirements {{quote(HTML_PERFORMANCE_PYTHON_REQUIREMENTS)}} python scripts/release/run_html_performance.py verify

# 同一SHAの完了した実測CIからimmutableな生計測証跡を取得する。
html-performance-evidence-acquire:
    {{HTML_PERFORMANCE_SCIPY_ENV}} uv run --isolated --with-requirements {{quote(HTML_PERFORMANCE_PYTHON_REQUIREMENTS)}} python scripts/release/run_html_performance.py verify --acquire

# 既存catalog CLIで84条件のstage90 raw capture manifestを作る。
html-performance-catalog-captures output_root prior_observation='' prior_finalized='' chrome_executable='' chrome_sha256='' font_observer='false':
    @set -euo pipefail; \
      output_relative={{quote(output_root)}}; \
      prior_observation={{quote(prior_observation)}}; \
      prior_finalized={{quote(prior_finalized)}}; \
      chrome_executable={{quote(chrome_executable)}}; \
      chrome_sha256={{quote(chrome_sha256)}}; \
      font_observer={{quote(font_observer)}}; \
      if [[ "$font_observer" != true && "$font_observer" != false ]]; then echo "font observer must be true or false" >&2; exit 2; fi; \
      if [[ -n "$chrome_executable" || -n "$chrome_sha256" ]]; then \
        if [[ "$chrome_executable" != /* || ! -f "$chrome_executable" || ! "$chrome_sha256" =~ ^[a-f0-9]{64}$ ]]; then echo "Chrome requires an absolute executable path and SHA-256 pin" >&2; exit 2; fi; \
      elif [[ "$font_observer" == true ]]; then echo "font observer requires a pinned Chrome executable" >&2; exit 2; fi; \
      if [[ -n "$prior_observation" || -n "$prior_finalized" ]]; then \
        if [[ -z "$prior_observation" || -z "$prior_finalized" || ! -f "$prior_observation" || ! -f "$prior_finalized" || -L "$prior_observation" || -L "$prior_finalized" ]]; then echo "resume requires both regular prior observation and finalized manifests" >&2; exit 2; fi; \
      fi; \
      if [[ ! "$output_relative" =~ ^tmp/html-performance-stage90/[A-Za-z0-9][A-Za-z0-9._-]*$ ]]; then echo "output root must be tmp/html-performance-stage90/<run-id>" >&2; exit 2; fi; \
      run_root={{quote(REPO_ROOT)}}/"$output_relative"; \
      tmp_root={{quote(REPO_ROOT + "/tmp")}}; \
      stage_root={{quote(REPO_ROOT + "/tmp/html-performance-stage90")}}; \
      if [[ -L "$tmp_root" || -L "$stage_root" ]]; then echo "refusing symlinked HTML stage90 output parent" >&2; exit 1; fi; \
      if [[ -e "$tmp_root" && ! -d "$tmp_root" || -e "$stage_root" && ! -d "$stage_root" ]]; then echo "HTML stage90 output parent is not a directory" >&2; exit 1; fi; \
      if [[ -e "$run_root" || -L "$run_root" ]]; then echo "refusing existing HTML stage90 output root: $run_root" >&2; exit 1; fi; \
      if [[ ! -e "$tmp_root" ]]; then mkdir "$tmp_root"; fi; \
      if [[ -L "$tmp_root" || ! -d "$tmp_root" ]]; then echo "HTML stage90 tmp root changed during creation" >&2; exit 1; fi; \
      if [[ ! -e "$stage_root" ]]; then mkdir "$stage_root"; fi; \
      if [[ -L "$stage_root" || ! -d "$stage_root" || -e "$run_root" || -L "$run_root" ]]; then echo "HTML stage90 output path changed during creation" >&2; exit 1; fi; \
      mkdir "$run_root"; \
      bun scripts/html/fidelity-acceptance-cli.ts catalog-stage90-plan "$run_root/plan.json"; \
      just capacity-admission; \
      python3 scripts/build/cargo_capacity_guard.py --root {{quote(REPO_ROOT)}} --target {{quote(REPO_ROOT + "/target")}} --state {{quote(REPO_ROOT + "/tmp/.cargo-capacity-guard.json")}} -- {{HTML84_CARGO}} test --release -p katana-render-runtime --lib --no-run --locked --message-format=json > "$run_root/test-build.jsonl"; \
      capture_args=(catalog-stage90-capture "$run_root/test-build.jsonl" "$run_root/plan.json" "$run_root/captures"); \
      if [[ -n "$prior_observation" ]]; then capture_args+=("$prior_observation" "$prior_finalized"); fi; \
      if [[ -n "$chrome_executable" ]]; then capture_args+=(--chrome-executable "$chrome_executable" --chrome-executable-sha256 "$chrome_sha256"); fi; \
      if [[ "$font_observer" == true ]]; then capture_args+=(--font-observer); fi; \
      bun scripts/html/fidelity-acceptance-cli.ts "${capture_args[@]}"; \
      test -s "$run_root/captures/catalog-stage90-captures.json"

# stage90 capture manifestなしの正式paired performanceを拒否する。
html-performance-evidence-produce catalog_manifest:
    @set -euo pipefail; \
      manifest={{quote(catalog_manifest)}}; \
      if [[ ! -f "$manifest" || -L "$manifest" ]]; then echo "catalog stage90 capture manifest is missing or not a regular file: $manifest" >&2; exit 2; fi; \
      binding_path="{{REPO_ROOT}}/tmp/html-performance-capacity/binding.json"; \
      binding_sha256="$(python3 scripts/build/initialize_ci_capacity_state.py --root "{{REPO_ROOT}}" --interaction-report "{{REPO_ROOT}}/tmp/html-interaction-suite/krr.json")"; \
      {{HTML_PERFORMANCE_SCIPY_ENV}} uv run --isolated --with-requirements {{quote(HTML_PERFORMANCE_PYTHON_REQUIREMENTS)}} python scripts/release/run_html_performance.py produce --repo "{{REPO_ROOT}}" --capacity-binding "$binding_path" --capacity-binding-sha256 "$binding_sha256" --catalog-plan "$manifest"

# HTML受入れとは独立にcandidate build proofだけを生成・検証する。
html-performance-candidate-build-proof output capacity_binding capacity_binding_sha256:
    python3 scripts/release/produce_html_performance_evidence.py --repo {{quote(REPO_ROOT)}} --output {{quote(output)}} --capacity-binding {{quote(capacity_binding)}} --capacity-binding-sha256 {{quote(capacity_binding_sha256)}} --candidate-build-only

# Run the complete quality gate before release-specific verification.
release-quality: check html-performance-nonregression-check

# Verify the concrete release version and packaged artifact.
release-specific: release-openspec-archive release-verify

# Verify release branch readiness before merging
release-check: release-quality release-specific

# Verify release-only checks after PR CI quality evidence has been verified
release-preflight-check: release-openspec-archive release-verify


# Install Playwright Chromium for official Mermaid / Draw.io reference rendering
browser-install:
    @if [[ "$(uname -s)" == "Linux" ]]; then bunx playwright install --with-deps chromium; else bunx playwright install chromium; fi

# Build the local krr CLI once before parallel fixture compares
krr-build:
    {{CARGO_BUILD}} build -p katana-render-runtime-cli

# Force-update all Rust and JavaScript dependencies plus pinned runtime assets, then run required checks
depends-update-all:
    python3 scripts/hooks/run_locked_command.py --protect-root-producer -- just depends-update-mutations
    just check
    just mermaid-reference-all
    just mermaid-compare-full
    just mermaid-compare-ci
    just drawio-reference-all
    just drawio-compare-full
    just drawio-compare-ci

[private]
depends-update-mutations:
    {{CARGO}} upgrade -i allow --pinned allow --exclude skrifa --exclude harfrust
    python3 scripts/release/update_html5ever_pair.py --cargo "{{CARGO}}"
    {{CARGO}} update
    python3 scripts/release/update_html_performance_harness_lock.py --cargo "{{CARGO}}"
    bun update --latest '!@moonrepo/*'
    bunx --no-install biome migrate --write
    bun run scripts/runtime-assets/depends-update-all.ts
    bun run scripts/drawio/resource-update.ts --resources "{{DRAWIO_RESOURCE_DIR}}" --manifest "{{DRAWIO_RESOURCE_MANIFEST}}"
    bun run scripts/runtime-assets/runtime-package-asset-compressor.ts --write
    just fmt
    just runtime-bundle-build

# Install pinned PlantUML LGPL JAR into the PlantUML cache
plantuml-install version=PLANTUML_JAR_VERSION output=PLANTUML_CACHE_JAR:
    @set -euo pipefail; \
    url="https://repo1.maven.org/maven2/net/sourceforge/plantuml/plantuml-lgpl/{{version}}/plantuml-lgpl-{{version}}.jar"; \
    target="{{output}}"; \
    max_attempts=3; \
    retry_delay="${KRR_PLANTUML_DOWNLOAD_RETRY_DELAY_SECONDS:-1}"; \
    case "$retry_delay" in ''|*[!0-9]*) echo "PlantUML retry delay must be a non-negative integer: $retry_delay" >&2; exit 1;; esac; \
    mkdir -p "$(dirname "$target")"; \
    tmp="$target.tmp"; \
    downloaded=false; \
    for attempt in $(seq 1 "$max_attempts"); do \
      if curl --fail --silent --show-error --location "$url" --output "$tmp"; then downloaded=true; break; fi; \
      if [ "$attempt" -lt "$max_attempts" ]; then echo "PlantUML download failed (attempt $attempt/$max_attempts); retrying" >&2; sleep "$retry_delay"; fi; \
    done; \
    if [ "$downloaded" != true ]; then echo "PlantUML download failed after $max_attempts attempts" >&2; exit 1; fi; \
    expected="{{PLANTUML_JAR_CHECKSUM}}"; \
    actual="$(bash scripts/plantuml/sha256-file.sh "$tmp")"; \
    if [ "$actual" != "$expected" ]; then \
      echo "PlantUML checksum mismatch: expected=$expected actual=$actual" >&2; \
      rm -f "$tmp"; \
      exit 1; \
    fi; \
    mv "$tmp" "$target"; \
    echo "installed PlantUML {{version}} to $target"

# Render krr Mermaid SVG fixtures
mermaid-render fixtures output='tmp/krr-mermaid-rendered' fixture_glob='*.md':
    @rm -rf "{{output}}"
    @mkdir -p "{{output}}"
    @for file in "{{fixtures}}"/{{fixture_glob}}; do \
      slug=$(basename "$file" .md); \
      {{CARGO}} run -p katana-render-runtime-cli -- mermaid render --input "$file" --output "{{output}}/$slug.svg"; \
    done

# Render krr Mermaid SVG fixtures with the prebuilt krr binary
mermaid-render-prebuilt fixtures output='tmp/krr-mermaid-rendered' fixture_glob='*.md':
    @rm -rf "{{output}}"
    @mkdir -p "{{output}}"
    @for file in "{{fixtures}}"/{{fixture_glob}}; do \
      slug=$(basename "$file" .md); \
      "{{KRR_BIN}}" mermaid render --input "$file" --output "{{output}}/$slug.svg"; \
    done

# 独立承認した入力と保護対象を使い、管理された実ビルド証跡を取得する。
native-cli-build-receipt expected_source expected_source_sha256 protected_registry protected_registry_sha256 cargo_bin rustc_bin runtime=MERMAID_JS:
    python3 scripts/mermaid/produce_native_cli_build_receipt.py --root {{quote(REPO_ROOT)}} --expected-source {{quote(expected_source)}} --expected-source-sha256 {{quote(expected_source_sha256)}} --protected-registry {{quote(protected_registry)}} --protected-registry-sha256 {{quote(protected_registry_sha256)}} --cargo {{quote(cargo_bin)}} --rustc {{quote(rustc_bin)}} --runtime {{quote(runtime)}}

# 既存guardの物理identity検証結果を、そのまま保護ref環境値へ使う。
protected-capacity-admission protected_registry:
    python3 scripts/mermaid/native_cli_build_bindings.py --repo {{quote(REPO_ROOT)}} --protected-registry {{quote(protected_registry)}}

# WHY: caller が監査済み producer の receipt を供給するまで正式比較を開始しない。
[private]
mermaid-build-receipt-required:
    @receipt={{quote(KRR_BUILD_RECEIPT)}}; host_os=$(uname -s); \
    case "$host_os" in \
      MINGW*|MSYS*|CYGWIN*) \
        case "$receipt" in [A-Za-z]:/*|[A-Za-z]:\\*|\\*|/*) ;; \
          *) printf '%s\n' "KRR_BUILD_RECEIPT must be an absolute caller-supplied receipt path" >&2; exit 1 ;; \
        esac ;; \
      *) \
        case "$receipt" in /*) ;; \
          *) printf '%s\n' "KRR_BUILD_RECEIPT must be an absolute caller-supplied receipt path" >&2; exit 1 ;; \
        esac ;; \
    esac; \
    test -f "$receipt" && test -s "$receipt" && ! test -L "$receipt" || { printf '%s\n' "KRR_BUILD_RECEIPT must name an existing regular nonempty receipt" >&2; exit 1; }

# Render Mermaid fixtures through the native renderer while retaining exact source and binary bindings
mermaid-render-strict fixtures output='tmp/krr-mermaid-rendered' fixture_glob='*.md': mermaid-build-receipt-required
    bun run scripts/mermaid/native-render-provenance.ts --fixtures-dir "{{fixtures}}" --fixture-glob "{{fixture_glob}}" --output-dir "{{output}}" --binary "{{KRR_BIN}}" --runtime "{{MERMAID_JS}}" --build-receipt "{{KRR_BUILD_RECEIPT}}"

# Update official Mermaid reference SVG / PNG
mermaid-reference fixtures output='tmp/krr-mermaid-official':
    bun run scripts/mermaid/diagram-update.ts --fixtures "{{fixtures}}" --output "{{output}}" --markdown-output "{{fixtures}}/official-dark" --theme dark --reference-mode legacyNormalized --reference-scope diagnostic --mermaid-js "{{MERMAID_JS}}" --mermaid-zenuml-js "{{MERMAID_ZENUML_JS}}" --skip-errors

# Generate a strict vendor-original diagnostic reference without rewriting fixture markdown
mermaid-reference-strict fixtures output=('tmp/krr-mermaid-strict-official/' + file_name(fixtures)):
    bun run scripts/mermaid/diagram-update.ts --fixtures "{{fixtures}}" --output "{{output}}" --theme dark --reference-mode strictVendorOriginal --reference-scope formal --mermaid-js "{{MERMAID_JS}}" --mermaid-zenuml-js "{{MERMAID_ZENUML_JS}}" --raster-session-config {{quote(MERMAID_RASTER_SESSION_CONFIG)}} --raster-session-config-sha {{quote(MERMAID_RASTER_SESSION_CONFIG_SHA)}}

# Update all committed Mermaid reference SVG / PNG fixtures
mermaid-reference-all:
    @set -euo pipefail; \
    mkdir -p "{{RUNTIME_UPDATE_LOG_DIR}}"; \
    printf '%s\n' \
      "tests/fixtures/mermaid/en" \
      "tests/fixtures/mermaid/ja" \
      "tests/fixtures/mermaid/representative" \
      | xargs -P "{{FIXTURE_JOBS}}" -I {} bash -c 'slug=${1#tests/fixtures/mermaid/}; log="{{RUNTIME_UPDATE_LOG_DIR}}/mermaid-reference-${slug//\//-}.log"; if just mermaid-reference-strict "$1" "tmp/krr-mermaid-strict-official/$slug" >"$log" 2>&1; then echo "strict mermaid reference passed: $slug (log: $log)"; else echo "strict mermaid reference failed: $slug (log: $log)" >&2; tail -n 80 "$log" >&2; exit 1; fi' _ {}

# Compare committed official Mermaid reference with krr rendering through ImageMagick score
mermaid-compare fixtures min_score='99' output='tmp/krr-mermaid' fixture_glob='*.md' official=('tmp/krr-mermaid-strict-official/' + file_name(fixtures)): mermaid-build-receipt-required
    just mermaid-render-strict "{{fixtures}}" "{{output}}/rendered" "{{fixture_glob}}"
    bun run scripts/mermaid/rasterize-svg-dir.ts --input "{{output}}/rendered" --output "{{output}}/rendered-browser" --theme dark --reference-mode strictVendorOriginal --reference-scope formal --fixtures-dir "{{fixtures}}" --fixture-glob "{{fixture_glob}}" --raster-session-config {{quote(MERMAID_RASTER_SESSION_CONFIG)}} --raster-session-config-sha {{quote(MERMAID_RASTER_SESSION_CONFIG_SHA)}}
    bun run scripts/mermaid/reference-compare.ts --official "{{official}}" --katana "{{output}}/rendered-browser" --katana-svg-dir "{{output}}/rendered" --fixtures-dir "{{fixtures}}" --fixture-glob "{{fixture_glob}}" --output "{{output}}/comparison" --theme dark --min-score "{{min_score}}" --reference-mode strictVendorOriginal --reference-scope formal --raster-binding-config {{quote(MERMAID_RASTER_BINDING_CONFIG)}} --raster-binding-config-sha {{quote(MERMAID_RASTER_BINDING_CONFIG_SHA)}}

# Compare Mermaid fixtures using the prebuilt krr binary
mermaid-compare-prebuilt fixtures min_score='99' output='tmp/krr-mermaid' fixture_glob='*.md' official=('tmp/krr-mermaid-strict-official/' + file_name(fixtures)): mermaid-build-receipt-required
    just mermaid-render-strict "{{fixtures}}" "{{output}}/rendered" "{{fixture_glob}}"
    bun run scripts/mermaid/rasterize-svg-dir.ts --input "{{output}}/rendered" --output "{{output}}/rendered-browser" --theme dark --reference-mode strictVendorOriginal --reference-scope formal --fixtures-dir "{{fixtures}}" --fixture-glob "{{fixture_glob}}" --raster-session-config {{quote(MERMAID_RASTER_SESSION_CONFIG)}} --raster-session-config-sha {{quote(MERMAID_RASTER_SESSION_CONFIG_SHA)}}
    bun run scripts/mermaid/reference-compare.ts --official "{{official}}" --katana "{{output}}/rendered-browser" --katana-svg-dir "{{output}}/rendered" --fixtures-dir "{{fixtures}}" --fixture-glob "{{fixture_glob}}" --output "{{output}}/comparison" --theme dark --min-score "{{min_score}}" --reference-mode strictVendorOriginal --reference-scope formal --raster-binding-config {{quote(MERMAID_RASTER_BINDING_CONFIG)}} --raster-binding-config-sha {{quote(MERMAID_RASTER_BINDING_CONFIG_SHA)}}

# WHY: 実 capture と外部 bindings の固定を score より先に完了できる入口を分ける。
mermaid-capture-prebuilt fixtures output='tmp/krr-mermaid' fixture_glob='*.md': mermaid-build-receipt-required
    just mermaid-render-strict "{{fixtures}}" "{{output}}/rendered" "{{fixture_glob}}"
    bun run scripts/mermaid/rasterize-svg-dir.ts --input "{{output}}/rendered" --output "{{output}}/rendered-browser" --theme dark --reference-mode strictVendorOriginal --reference-scope formal --fixtures-dir "{{fixtures}}" --fixture-glob "{{fixture_glob}}" --raster-session-config {{quote(MERMAID_RASTER_SESSION_CONFIG)}} --raster-session-config-sha {{quote(MERMAID_RASTER_SESSION_CONFIG_SHA)}}

# WHY: 完了した二 producer の外部 pins を検査し、再 capture せず各99で比較する。
mermaid-score-strict fixtures output='tmp/krr-mermaid' fixture_glob='*.md' official=('tmp/krr-mermaid-strict-official/' + file_name(fixtures)): mermaid-build-receipt-required
    bun run scripts/mermaid/reference-compare.ts --official "{{official}}" --katana "{{output}}/rendered-browser" --katana-svg-dir "{{output}}/rendered" --fixtures-dir "{{fixtures}}" --fixture-glob "{{fixture_glob}}" --output "{{output}}/comparison" --theme dark --min-score "99" --reference-mode strictVendorOriginal --reference-scope formal --raster-binding-config {{quote(MERMAID_RASTER_BINDING_CONFIG)}} --raster-binding-config-sha {{quote(MERMAID_RASTER_BINDING_CONFIG_SHA)}}

# Compare representative Mermaid patterns for CI/CD
mermaid-compare-ci min_score='99': mermaid-build-receipt-required
    just mermaid-compare-prebuilt tests/fixtures/mermaid/representative "{{min_score}}" tmp/krr-mermaid-ci

# Run the managed current-source EN/JA full comparison and aggregate exact 117 score evidence.
mermaid-compare-full-managed request request_sha256:
    bun run scripts/mermaid/managed-full-compare.ts --request "{{request}}" --request-sha256 "{{request_sha256}}"

# Compare full Mermaid fixture sets for local release validation
mermaid-compare-full min_score='99' output='tmp/krr-mermaid-full' official='tmp/krr-mermaid-strict-official': mermaid-build-receipt-required
    @set -euo pipefail; \
    compare_output={{quote(output)}}; \
    official_root={{quote(official)}}; \
    session_config_en={{quote(MERMAID_RASTER_SESSION_CONFIG_EN)}}; \
    session_sha_en={{quote(MERMAID_RASTER_SESSION_CONFIG_SHA_EN)}}; \
    binding_config_en={{quote(MERMAID_RASTER_BINDING_CONFIG_EN)}}; \
    binding_sha_en={{quote(MERMAID_RASTER_BINDING_CONFIG_SHA_EN)}}; \
    session_config_ja={{quote(MERMAID_RASTER_SESSION_CONFIG_JA)}}; \
    session_sha_ja={{quote(MERMAID_RASTER_SESSION_CONFIG_SHA_JA)}}; \
    binding_config_ja={{quote(MERMAID_RASTER_BINDING_CONFIG_JA)}}; \
    binding_sha_ja={{quote(MERMAID_RASTER_BINDING_CONFIG_SHA_JA)}}; \
    test -n "$session_config_en" && test -n "$session_sha_en" && test -n "$binding_config_en" && test -n "$binding_sha_en" || { printf '%s\n' "EN strict raster session and binding config/SHA are required" >&2; exit 1; }; \
    test -n "$session_config_ja" && test -n "$session_sha_ja" && test -n "$binding_config_ja" && test -n "$binding_sha_ja" || { printf '%s\n' "JA strict raster session and binding config/SHA are required" >&2; exit 1; }; \
    [[ "$session_sha_en" =~ ^[a-f0-9]{64}$ && "$binding_sha_en" =~ ^[a-f0-9]{64}$ && "$session_sha_ja" =~ ^[a-f0-9]{64}$ && "$binding_sha_ja" =~ ^[a-f0-9]{64}$ ]] || { printf '%s\n' "Per-language strict raster config SHA must be lowercase SHA-256" >&2; exit 1; }; \
    mkdir -p "{{RUNTIME_UPDATE_LOG_DIR}}"; \
    printf '%s\n' \
      "tests/fixtures/mermaid/en" \
      "tests/fixtures/mermaid/ja" \
      | xargs -P "{{FIXTURE_JOBS}}" -I {} bash -c 'set -euo pipefail; slug=${1#tests/fixtures/mermaid/}; case "$slug" in en) session_config="$3"; session_sha="$4"; binding_config="$5"; binding_sha="$6";; ja) session_config="$7"; session_sha="$8"; binding_config="$9"; binding_sha="${10}";; *) printf "%s\n" "Unsupported Mermaid full-compare domain: $slug" >&2; exit 1;; esac; unset MERMAID_RASTER_SESSION_CONFIG_EN MERMAID_RASTER_SESSION_CONFIG_SHA_EN MERMAID_RASTER_BINDING_CONFIG_EN MERMAID_RASTER_BINDING_CONFIG_SHA_EN MERMAID_RASTER_SESSION_CONFIG_JA MERMAID_RASTER_SESSION_CONFIG_SHA_JA MERMAID_RASTER_BINDING_CONFIG_JA MERMAID_RASTER_BINDING_CONFIG_SHA_JA; log="{{RUNTIME_UPDATE_LOG_DIR}}/mermaid-compare-${slug//\//-}.log"; if MERMAID_RASTER_SESSION_CONFIG="$session_config" MERMAID_RASTER_SESSION_CONFIG_SHA="$session_sha" MERMAID_RASTER_BINDING_CONFIG="$binding_config" MERMAID_RASTER_BINDING_CONFIG_SHA="$binding_sha" just mermaid-compare-prebuilt "$1" "$2" "${11}/$slug" "*.md" "${12}/$slug" >"$log" 2>&1; then echo "mermaid compare passed: $slug (log: $log)"; else echo "mermaid compare failed: $slug (log: $log)" >&2; tail -n 80 "$log" >&2; exit 1; fi' _ {} "{{min_score}}" "$session_config_en" "$session_sha_en" "$binding_config_en" "$binding_sha_en" "$session_config_ja" "$session_sha_ja" "$binding_config_ja" "$binding_sha_ja" "$compare_output" "$official_root"

# Render Mermaid fixtures for a timing smoke check
mermaid-bench fixtures:
    @start=$(date +%s); just mermaid-render "{{fixtures}}" tmp/krr-mermaid-bench; end=$(date +%s); elapsed=$((end - start)); echo "mermaid fixtures rendered in ${elapsed}s"

# Render krr Draw.io SVG fixtures
drawio-render fixtures output='tmp/krr-drawio-rendered':
    @rm -rf "{{output}}"
    @mkdir -p "{{output}}"
    @for file in "{{fixtures}}"/*.drawio; do \
      slug=$(basename "$file" .drawio); \
      {{CARGO}} run -p katana-render-runtime-cli -- drawio render --input "$file" --output "{{output}}/$slug.svg"; \
    done

# Render krr Draw.io SVG fixtures with the prebuilt krr binary
drawio-render-prebuilt fixtures output='tmp/krr-drawio-rendered':
    @rm -rf "{{output}}"
    @mkdir -p "{{output}}"
    @for file in "{{fixtures}}"/*.drawio; do \
      slug=$(basename "$file" .drawio); \
      "{{KRR_BIN}}" drawio render --input "$file" --output "{{output}}/$slug.svg"; \
    done

# Update official Draw.io reference SVG / PNG
drawio-reference fixtures:
    bun run scripts/drawio/diagram-update.ts --fixtures "{{fixtures}}" --output "{{fixtures}}/official" --drawio-js "{{DRAWIO_JS}}" --resources "{{DRAWIO_RESOURCE_DIR}}" --resource-manifest "{{DRAWIO_RESOURCE_MANIFEST}}"

# Update all committed Draw.io reference SVG / PNG fixtures
drawio-reference-all:
    @set -euo pipefail; \
    mkdir -p "{{RUNTIME_UPDATE_LOG_DIR}}"; \
    root="tests/fixtures/drawio"; \
    printf '%s\n' \
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
      "$root/representative" \
      | xargs -P "{{FIXTURE_JOBS}}" -I {} bash -c 'slug=${1#tests/fixtures/drawio/}; log="{{RUNTIME_UPDATE_LOG_DIR}}/drawio-reference-${slug//\//-}.log"; if just drawio-reference "$1" >"$log" 2>&1; then echo "drawio reference passed: $slug (log: $log)"; else echo "drawio reference failed: $slug (log: $log)" >&2; tail -n 80 "$log" >&2; exit 1; fi' _ {}

# Compare committed official Draw.io reference with krr rendering through ImageMagick score
drawio-compare fixtures min_score='99' output='tmp/krr-drawio' baseline='':
    just drawio-render "{{fixtures}}" "{{output}}/rendered"
    bun run scripts/mermaid/rasterize-svg-dir.ts --input "{{output}}/rendered" --output "{{output}}/rendered-browser"
    @if [ -n "{{baseline}}" ]; then \
      bun run scripts/drawio/reference-compare.ts --official "{{fixtures}}/official" --katana "{{output}}/rendered-browser" --output "{{output}}/comparison" --min-score "{{min_score}}" --baseline "{{baseline}}"; \
    else \
      bun run scripts/drawio/reference-compare.ts --official "{{fixtures}}/official" --katana "{{output}}/rendered-browser" --output "{{output}}/comparison" --min-score "{{min_score}}"; \
    fi

# Compare Draw.io fixtures using the prebuilt krr binary
drawio-compare-prebuilt fixtures min_score='99' output='tmp/krr-drawio' baseline='':
    just drawio-render-prebuilt "{{fixtures}}" "{{output}}/rendered"
    bun run scripts/mermaid/rasterize-svg-dir.ts --input "{{output}}/rendered" --output "{{output}}/rendered-browser"
    @if [ -n "{{baseline}}" ]; then \
      bun run scripts/drawio/reference-compare.ts --official "{{fixtures}}/official" --katana "{{output}}/rendered-browser" --output "{{output}}/comparison" --min-score "{{min_score}}" --baseline "{{baseline}}"; \
    else \
      bun run scripts/drawio/reference-compare.ts --official "{{fixtures}}/official" --katana "{{output}}/rendered-browser" --output "{{output}}/comparison" --min-score "{{min_score}}"; \
    fi

# Compare representative Draw.io patterns for CI/CD
drawio-compare-ci min_score='99':
    just krr-build
    just drawio-compare-prebuilt tests/fixtures/drawio/representative "{{min_score}}" tmp/krr-drawio-ci tests/fixtures/drawio/representative/score-baseline.json

# Compare basic Draw.io patterns as a smoke check
drawio-compare-basic min_score='99':
    just krr-build
    just drawio-compare-prebuilt tests/fixtures/drawio/basic "{{min_score}}" tmp/krr-drawio-basic

# Compare full Draw.io fixture sets for local release validation
drawio-compare-full min_score='99':
    just krr-build
    @set -euo pipefail; \
    mkdir -p "{{RUNTIME_UPDATE_LOG_DIR}}"; \
    root="tests/fixtures/drawio"; \
    printf '%s\n' \
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
      | xargs -P "{{FIXTURE_JOBS}}" -I {} bash -c 'slug=${1#tests/fixtures/drawio/}; output_slug=${slug//\//-}; log="{{RUNTIME_UPDATE_LOG_DIR}}/drawio-compare-$output_slug.log"; if just drawio-compare-prebuilt "$1" "$2" "tmp/krr-drawio-full/$output_slug" >"$log" 2>&1; then echo "drawio compare passed: $slug (log: $log)"; else echo "drawio compare failed: $slug (log: $log)" >&2; tail -n 80 "$log" >&2; exit 1; fi' _ {} "{{min_score}}"

# Render Draw.io fixtures for a timing smoke check
drawio-bench fixtures:
    @start=$(date +%s); just drawio-render "{{fixtures}}" tmp/krr-drawio-bench; end=$(date +%s); elapsed=$((end - start)); echo "drawio fixtures rendered in ${elapsed}s"

# Render krr PlantUML SVG fixtures
plantuml-render fixtures output='tmp/krr-plantuml-rendered':
    @rm -rf "{{output}}"
    @mkdir -p "{{output}}"
    @for file in "{{fixtures}}"/*.puml "{{fixtures}}"/*.md; do \
      [ -e "$file" ] || continue; \
      [ "$(basename "$file")" = "README.md" ] && continue; \
      slug=$(basename "$file"); \
      slug="${slug%.*}"; \
      {{CARGO}} run -p katana-render-runtime-cli -- plantuml render --input "$file" --output "{{output}}/$slug.svg"; \
    done

# Render krr PlantUML SVG fixtures with the prebuilt krr binary
plantuml-render-prebuilt fixtures output='tmp/krr-plantuml-rendered':
    @rm -rf "{{output}}"
    @mkdir -p "{{output}}"
    @for file in "{{fixtures}}"/*.puml "{{fixtures}}"/*.md; do \
      [ -e "$file" ] || continue; \
      [ "$(basename "$file")" = "README.md" ] && continue; \
      slug=$(basename "$file"); \
      slug="${slug%.*}"; \
      "{{KRR_BIN}}" plantuml render --input "$file" --output "{{output}}/$slug.svg"; \
    done

# Update PlantUML reference SVG fixtures
plantuml-reference fixtures output='tmp/krr-plantuml-reference':
    just plantuml-install
    bun run scripts/plantuml/diagram-update.ts --fixtures "{{fixtures}}" --output "{{output}}/official-svg" --jar "{{PLANTUML_CACHE_JAR}}" --dark-mode
    bun run scripts/mermaid/rasterize-svg-dir.ts --input "{{output}}/official-svg" --output "{{fixtures}}/official-dark" --theme dark

# Compare PlantUML fixtures with official dark-mode reference score
plantuml-compare fixtures min_score='100' output='tmp/krr-plantuml':
    just plantuml-render "{{fixtures}}" "{{output}}/rendered"
    bun run scripts/mermaid/rasterize-svg-dir.ts --input "{{output}}/rendered" --output "{{output}}/rendered-browser" --theme dark
    bun run scripts/plantuml/reference-compare.ts --official "{{fixtures}}/official-dark" --katana "{{output}}/rendered-browser" --output "{{output}}/comparison" --min-score "{{min_score}}"

# Compare PlantUML fixtures using the prebuilt krr binary
plantuml-compare-prebuilt fixtures min_score='100' output='tmp/krr-plantuml':
    just plantuml-render-prebuilt "{{fixtures}}" "{{output}}/rendered"
    bun run scripts/mermaid/rasterize-svg-dir.ts --input "{{output}}/rendered" --output "{{output}}/rendered-browser" --theme dark
    bun run scripts/plantuml/reference-compare.ts --official "{{fixtures}}/official-dark" --katana "{{output}}/rendered-browser" --output "{{output}}/comparison" --min-score "{{min_score}}"

# Compare representative PlantUML patterns for CI/CD
plantuml-compare-ci min_score='100':
    just krr-build
    just plantuml-compare-prebuilt tests/fixtures/plantuml/official "{{min_score}}" tmp/krr-plantuml-ci

# Render PlantUML fixtures for a timing smoke check
plantuml-bench fixtures:
    @start=$(date +%s); just plantuml-render "{{fixtures}}" tmp/krr-plantuml-bench; end=$(date +%s); elapsed=$((end - start)); echo "plantuml fixtures rendered in ${elapsed}s"
