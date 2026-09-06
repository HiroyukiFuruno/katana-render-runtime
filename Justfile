set shell := ["bash", "-uc"]

REPO_ROOT := justfile_directory()
RTK := env_var_or_default("RTK", `command -v rtk 2> /dev/null || true`)
RTK_CMD := if RTK == "" { "" } else { RTK + " " }
JOBS := env_var_or_default("JOBS", "2")
FIXTURE_JOBS := env_var_or_default("FIXTURE_JOBS", JOBS)
TEST_THREADS := env_var_or_default("TEST_THREADS", "1")
TEST_THREAD_ARGS := if TEST_THREADS == "" { "" } else { " -- --test-threads=" + TEST_THREADS }
RUNTIME_UPDATE_LOG_DIR := env_var_or_default("RUNTIME_UPDATE_LOG_DIR", "tmp/runtime-update-logs")
export RUSTFLAGS := env_var_or_default("RUSTFLAGS", "-D warnings")
CARGO := env_var_or_default("CARGO", RTK_CMD + "cargo")
KRR_BIN := env_var_or_default("KRR_BIN", REPO_ROOT + "/target/debug/krr")
VERSION := env_var_or_default("VERSION", `awk -F '"' '/^version = / { print $2; exit }' Cargo.toml`)
VERSION_BARE := replace(VERSION, "v", "")
TAG := "v" + VERSION_BARE
RELEASE_REPO := env_var_or_default("RELEASE_REPO", "HiroyukiFuruno/katana-render-runtime")
COVERAGE_MIN_LINES := env_var_or_default("COVERAGE_MIN_LINES", "100")
COVERAGE_MAX_UNCOVERED_LINES := env_var_or_default("COVERAGE_MAX_UNCOVERED_LINES", "0")
MERMAID_JS_VERSION := "11.17.2"
MERMAID_ZENUML_JS_VERSION := "0.2.3"
DRAWIO_JS_VERSION := "31.3.2"
MATHJAX_JS_VERSION := "4.1.3"
ZENUML_CORE_JS_VERSION := "3.47.9"
PLANTUML_JAR_VERSION := "1.2026.7"
PLANTUML_JAR_CHECKSUM := "1eb8cd1d0253227f3652586bc3b53cb3d5cfe69b5dcca41ce9b92ab1ce4f58ff"
PLAYWRIGHT_VERSION := "1.60.0"
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

# Check Rust formatting
fmt-check:
    {{CARGO}} fmt --all -- --check

# Run strict Clippy checks
lint:
    RUSTFLAGS="-D warnings" {{CARGO}} clippy -j {{JOBS}} --workspace --all-targets --all-features -- -D warnings -D clippy::unwrap_used -D clippy::expect_used -D clippy::todo -D clippy::unimplemented -D clippy::dbg_macro -D clippy::panic -D clippy::wildcard_imports -D clippy::too_many_lines -D clippy::cognitive_complexity

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
    {{CARGO}} test --workspace --all-targets --all-features --locked{{TEST_THREAD_ARGS}}

# Run coverage as a required full-check gate
coverage: plantuml-install
    {{CARGO}} llvm-cov clean --workspace
    {{CARGO}} llvm-cov --workspace --all-targets --all-features --locked --summary-only --fail-under-lines {{COVERAGE_MIN_LINES}} --fail-uncovered-lines {{COVERAGE_MAX_UNCOVERED_LINES}}{{TEST_THREAD_ARGS}}

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

# Run TypeScript formatter/linter gate
biome:
    bun run biome

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
    if ! printf '%s\n' "$package_files" | grep -qx "src/renderer/backends/html_runtime/dom_bootstrap.js"; then \
      echo "missing HTML runtime package file: dom_bootstrap.js" >&2; \
      exit 1; \
    fi

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
    bun test --path-ignore-patterns 'tmp/**' scripts/runtime-assets/runtime-asset-common_test.ts scripts/runtime-assets/update_test.ts scripts/runtime-assets/latest-check_test.ts scripts/runtime-assets/update_zenuml_test.ts scripts/runtime-assets/depends-update-all_test.ts scripts/runtime-assets/runtime-package-asset-compressor_test.ts

[private]
runtime-package-asset-check:
    bun run scripts/runtime-assets/runtime-package-asset-compressor.ts --check
    @package_files="$({{CARGO}} package -p katana-render-runtime --locked --allow-dirty --list)"; \
    for kind_file in \
      "drawio/{{DRAWIO_JS_VERSION}}/drawio.min.js" \
      "mermaid/{{MERMAID_JS_VERSION}}/mermaid.min.js"; do \
        if ! printf '%s\n' "$package_files" | grep -qx "vendor/$kind_file.br"; then \
          echo "missing compressed runtime package asset: vendor/$kind_file.br" >&2; \
          exit 1; \
        fi; \
        if printf '%s\n' "$package_files" | grep -qx "vendor/$kind_file"; then \
          echo "raw runtime asset must not be included in the crates.io package: vendor/$kind_file" >&2; \
          exit 1; \
        fi; \
      done

# Verify repository hook and release cleanup contracts
automation-contract-test:
    python3 -m unittest discover -s scripts/hooks -p '*_test.py'
    python3 -m unittest discover -s scripts/release -p 'cleanup_release_state_test.py'
    python3 -m unittest discover -s scripts/review -p '*_test.py'

# Run the local quality gate
check: fmt-check lint runtime-bundle-check runtime-asset-script-test automation-contract-test unit-test ast-lint dependency-leak biome typecheck runtime-asset-check runtime-package-asset-check runtime-bundle-package-check html-runtime-package-check plantuml-runtime-package-check
    @echo "checks passed"

# Sweep old build artifacts locally (older than 7 days)
sweep:
    @{{CARGO}} sweep --time 7 || true

# Remove build artifacts
clean: sweep
    {{CARGO}} clean

# Verify VERSION follows the remote release line
release-target-check:
    bash scripts/release/verify-version.sh "{{VERSION}}"
    python3 scripts/release/verify-release-target.py --target-version "{{VERSION}}" --repo "{{RELEASE_REPO}}"
    bash scripts/release/assert-tag-safe.sh "{{TAG}}" origin
    bash scripts/release/assert-crates-not-published.sh "{{VERSION}}"

# Verify package metadata and dry-run the first publishable crate
release-verify: release-target-check
    bash scripts/release/verify-version.sh "{{VERSION}}"
    bash scripts/release/verify-internal-dependencies.sh "{{VERSION}}"
    {{CARGO}} package -p katana-render-runtime --locked --allow-dirty
    {{CARGO}} test --manifest-path "target/package/katana-render-runtime-{{VERSION_BARE}}/Cargo.toml" --lib --locked
    {{CARGO}} package -p katana-render-runtime-cli --locked --allow-dirty --list >/dev/null
    bash scripts/release/verify-crate-size.sh katana-render-runtime "{{VERSION}}"
    {{CARGO}} publish -p katana-render-runtime --dry-run --locked --allow-dirty

# Verify completed OpenSpec changes are archived before release PRs
release-openspec-archive:
    bash scripts/release/check-openspec-release-archive.sh --self-test
    bash scripts/release/check-openspec-release-archive.sh "{{VERSION}}"

# Verify release branch readiness before merging
release-check: release-openspec-archive check coverage release-verify

# Verify pull request readiness before merging
pr-ready-check pr:
    #!/usr/bin/env bash
    set -euo pipefail
    pr={{quote(pr)}}
    if [[ ! "$pr" =~ ^[1-9][0-9]*$ ]]; then
      echo "pr-ready-check requires a positive numeric pull request number" >&2
      exit 2
    fi
    pr_metadata="$(gh pr view "$pr" --json baseRefOid,headRefOid,headRefName,baseRefName,isDraft)"
    repository_metadata="$(gh repo view --json nameWithOwner)"
    if ! IFS="$(printf '\011')" read -r repository_owner repository_name repository repository_extra < <(python3 -c 'import json, re, sys; value = json.loads(sys.argv[1]).get("nameWithOwner"); match = re.fullmatch(r"([A-Za-z0-9_.-]+)/([A-Za-z0-9_.-]+)", value) if isinstance(value, str) else None; match or sys.exit("gh returned incomplete or unsafe repository metadata"); print(match.group(1), match.group(2), value, sep=chr(9))' "$repository_metadata"); then
      echo "gh returned incomplete repository metadata" >&2
      exit 2
    fi
    if [[ -n "${repository_extra:-}" || -z "$repository_owner" || -z "$repository_name" || -z "$repository" ]]; then
      echo "gh returned incomplete repository metadata" >&2
      exit 2
    fi
    default_metadata="$(gh api graphql -f query='query($owner:String!,$name:String!){repository(owner:$owner,name:$name){defaultBranchRef{name target{__typename ... on Commit {oid}}}}}' -f owner="$repository_owner" -f name="$repository_name")"
    if ! IFS="$(printf '\011')" read -r base_sha head_sha branch base_branch parsed_repository trusted_default_sha readiness_mode extra < <(python3 -c 'import json, re, sys; pull = json.loads(sys.argv[1]); repository = sys.argv[2]; graph = json.loads(sys.argv[3]); data = graph.get("data") if isinstance(graph, dict) else None; node = data.get("repository") if isinstance(data, dict) else None; default = node.get("defaultBranchRef") if isinstance(node, dict) else None; target = default.get("target") if isinstance(default, dict) else None; fields = (pull.get("baseRefOid"), pull.get("headRefOid"), pull.get("headRefName"), pull.get("baseRefName"), repository, target.get("oid") if isinstance(target, dict) else None, pull.get("isDraft")); valid = isinstance(fields[0], str) and re.fullmatch(r"[0-9a-fA-F]{40}", fields[0]) and isinstance(fields[1], str) and re.fullmatch(r"[0-9a-fA-F]{40}", fields[1]) and isinstance(fields[2], str) and not any(ord(character) < 32 or ord(character) == 127 for character in fields[2]) and isinstance(fields[3], str) and re.fullmatch(r"[A-Za-z0-9._/-]+", fields[3]) and isinstance(default, dict) and isinstance(default.get("name"), str) and re.fullmatch(r"[A-Za-z0-9._/-]+", default["name"]) and fields[3] == default["name"] and re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", fields[4]) and isinstance(target, dict) and target.get("__typename") == "Commit" and isinstance(fields[5], str) and re.fullmatch(r"[0-9a-fA-F]{40}", fields[5]) and isinstance(fields[6], bool); valid or sys.exit("gh returned incomplete or unsafe PR/default metadata"); print(*fields[:6], "require-draft" if fields[6] else "allow-ready", sep=chr(9))' "$pr_metadata" "$repository" "$default_metadata"); then
      echo "gh returned incomplete PR metadata" >&2
      exit 2
    fi
    if [[ -n "${extra:-}" || -z "$base_sha" || -z "$head_sha" || -z "$branch" || -z "$base_branch" || -z "$parsed_repository" || "$parsed_repository" != "$repository" || -z "$trusted_default_sha" || ( "$readiness_mode" != "require-draft" && "$readiness_mode" != "allow-ready" ) ]]; then
      echo "gh returned incomplete PR metadata" >&2
      exit 2
    fi
    python3 scripts/hooks/verify_push_issue.py --pr-number "$pr" --pr-base-sha "$base_sha" --pr-head-sha "$head_sha" --pr-branch "$branch" --repository "$repository" --trusted-default-sha "$trusted_default_sha"
    python3 scripts/review/verify_pr_ready.py --pr "$pr" --repository "$repository" "--$readiness_mode" --expected-base-sha "$base_sha" --expected-head-sha "$head_sha"

# Install Playwright Chromium for official Mermaid / Draw.io reference rendering
browser-install:
    @if [[ "$(uname -s)" == "Linux" ]]; then bunx playwright install --with-deps chromium; else bunx playwright install chromium; fi

# Build the local krr CLI once before parallel fixture compares
krr-build:
    {{CARGO}} build -p katana-render-runtime-cli

# Force-update all Rust and JavaScript dependencies plus pinned runtime assets, then run required checks
depends-update-all:
    {{CARGO}} upgrade -i
    {{CARGO}} update
    bun update --latest
    bun add -d typescript@6.0.3
    bun run scripts/runtime-assets/depends-update-all.ts
    bun run scripts/drawio/resource-update.ts --resources "{{DRAWIO_RESOURCE_DIR}}" --manifest "{{DRAWIO_RESOURCE_MANIFEST}}"
    bun run scripts/runtime-assets/runtime-package-asset-compressor.ts --write
    just runtime-bundle-build
    just mermaid-reference-all
    just mermaid-compare-full
    just mermaid-compare-ci
    just drawio-reference-all
    just drawio-compare-full
    just drawio-compare-ci
    just check
    just coverage

# Install pinned PlantUML LGPL JAR into the PlantUML cache
plantuml-install version=PLANTUML_JAR_VERSION output=PLANTUML_CACHE_JAR:
    @set -euo pipefail; \
    url="https://repo1.maven.org/maven2/net/sourceforge/plantuml/plantuml-lgpl/{{version}}/plantuml-lgpl-{{version}}.jar"; \
    target="{{output}}"; \
    mkdir -p "$(dirname "$target")"; \
    tmp="$target.tmp"; \
    curl -fsSL "$url" -o "$tmp"; \
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

# Update official Mermaid reference SVG / PNG
mermaid-reference fixtures output='tmp/krr-mermaid-official':
    bun run scripts/mermaid/diagram-update.ts --fixtures "{{fixtures}}" --output "{{output}}" --markdown-output "{{fixtures}}/official-dark" --theme dark --mermaid-js "{{MERMAID_JS}}" --mermaid-zenuml-js "{{MERMAID_ZENUML_JS}}" --skip-errors

# Update all committed Mermaid reference SVG / PNG fixtures
mermaid-reference-all:
    @set -euo pipefail; \
    mkdir -p "{{RUNTIME_UPDATE_LOG_DIR}}"; \
    printf '%s\n' \
      "tests/fixtures/mermaid/en" \
      "tests/fixtures/mermaid/ja" \
      "tests/fixtures/mermaid/representative" \
      | xargs -P "{{FIXTURE_JOBS}}" -I {} bash -c 'slug=${1#tests/fixtures/mermaid/}; log="{{RUNTIME_UPDATE_LOG_DIR}}/mermaid-reference-${slug//\//-}.log"; if just mermaid-reference "$1" "tmp/krr-mermaid-official/$slug" >"$log" 2>&1; then echo "mermaid reference passed: $slug (log: $log)"; else echo "mermaid reference failed: $slug (log: $log)" >&2; tail -n 80 "$log" >&2; exit 1; fi' _ {}

# Compare committed official Mermaid reference with krr rendering through ImageMagick score
mermaid-compare fixtures min_score='99' output='tmp/krr-mermaid' fixture_glob='*.md':
    just mermaid-render "{{fixtures}}" "{{output}}/rendered" "{{fixture_glob}}"
    bun run scripts/mermaid/rasterize-svg-dir.ts --input "{{output}}/rendered" --output "{{output}}/rendered-browser" --theme dark
    bun run scripts/mermaid/reference-compare.ts --official "{{fixtures}}/official-dark" --katana "{{output}}/rendered-browser" --output "{{output}}/comparison" --theme dark --min-score "{{min_score}}"

# Compare Mermaid fixtures using the prebuilt krr binary
mermaid-compare-prebuilt fixtures min_score='99' output='tmp/krr-mermaid' fixture_glob='*.md':
    just mermaid-render-prebuilt "{{fixtures}}" "{{output}}/rendered" "{{fixture_glob}}"
    bun run scripts/mermaid/rasterize-svg-dir.ts --input "{{output}}/rendered" --output "{{output}}/rendered-browser" --theme dark
    bun run scripts/mermaid/reference-compare.ts --official "{{fixtures}}/official-dark" --katana "{{output}}/rendered-browser" --output "{{output}}/comparison" --theme dark --min-score "{{min_score}}"

# Compare representative Mermaid patterns for CI/CD
mermaid-compare-ci min_score='99':
    just krr-build
    just mermaid-compare-prebuilt tests/fixtures/mermaid/representative "{{min_score}}" tmp/krr-mermaid-ci

# Compare full Mermaid fixture sets for local release validation
mermaid-compare-full min_score='99':
    just krr-build
    @set -euo pipefail; \
    mkdir -p "{{RUNTIME_UPDATE_LOG_DIR}}"; \
    printf '%s\n' \
      "tests/fixtures/mermaid/en" \
      "tests/fixtures/mermaid/ja" \
      | xargs -P "{{FIXTURE_JOBS}}" -I {} bash -c 'slug=${1#tests/fixtures/mermaid/}; log="{{RUNTIME_UPDATE_LOG_DIR}}/mermaid-compare-${slug//\//-}.log"; if just mermaid-compare-prebuilt "$1" "$2" "tmp/krr-mermaid-full/$slug" >"$log" 2>&1; then echo "mermaid compare passed: $slug (log: $log)"; else echo "mermaid compare failed: $slug (log: $log)" >&2; tail -n 80 "$log" >&2; exit 1; fi' _ {} "{{min_score}}"

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
