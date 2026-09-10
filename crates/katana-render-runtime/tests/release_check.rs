use std::{
    path::{Path, PathBuf},
    process::Command,
};

#[test]
fn release_check_requires_all_quality_and_publish_readiness_gates()
-> Result<(), Box<dyn std::error::Error>> {
    let justfile = std::fs::read_to_string(workspace_root()?.join("Justfile"))?;
    let recipe = recipe_body(&justfile, "release-check")?;

    for required_gate in [
        "release-openspec-archive",
        "check",
        "coverage",
        "release-verify",
    ] {
        assert!(
            recipe.contains(required_gate),
            "release-check must require {required_gate}"
        );
    }
    Ok(())
}

#[test]
fn release_verify_tests_the_packaged_library_sources() -> Result<(), Box<dyn std::error::Error>> {
    let justfile = std::fs::read_to_string(workspace_root()?.join("Justfile"))?;
    let recipe = recipe_body(&justfile, "release-verify")?;

    assert!(recipe.contains(
        "test --manifest-path \"target/package/katana-render-runtime-{{VERSION_BARE}}/Cargo.toml\" --lib --locked"
    ));
    Ok(())
}

#[test]
fn release_target_check_requires_v0_4_20_intent() -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    assert!(release_target_check(root, "0.4.20", "0.4.19", "HEAD")?);
    assert!(release_target_check(root, "0.4.20", "0.4.20", "HEAD")?);
    assert!(!release_target_check(
        root,
        "0.4.20",
        "0.4.19",
        "missing-release-head",
    )?);
    assert!(!release_target_check(root, "0.4.20", "0.4.18", "HEAD")?);
    assert!(!release_target_check(root, "0.4.21", "0.4.19", "HEAD")?);
    for version in [
        "0.3.9", "0.4.0", "0.4.1", "0.4.2", "0.4.3", "0.4.4", "0.4.5", "0.4.6", "0.4.7", "0.4.8",
        "0.4.9", "0.4.10", "0.4.11", "0.4.12", "0.4.13", "0.4.14", "0.4.15", "0.4.16", "0.4.17",
        "0.4.18", "0.4.19", "0.5.0", "1.0.0", "2.0.0",
    ] {
        assert!(!release_target_check(root, version, "0.4.19", "HEAD",)?);
    }
    Ok(())
}

#[test]
fn crates_publish_retries_transient_registry_failures_with_a_visibility_probe()
-> Result<(), Box<dyn std::error::Error>> {
    let script =
        std::fs::read_to_string(workspace_root()?.join("scripts/release/publish-crates.sh"))?;

    assert!(script.contains("PUBLISH_ATTEMPTS:-3"));
    assert!(script.contains("PUBLISH_RETRY_DELAY_SECONDS:-10"));
    assert!(script.contains("cargo info \"${package}@${version}\""));
    assert!(script.contains("if cargo publish"));
    assert!(script.contains("sleep \"${delay}\""));
    Ok(())
}

#[test]
fn publish_recovery_uses_the_immutable_release_tag_with_current_retry_tools()
-> Result<(), Box<dyn std::error::Error>> {
    let workflow = std::fs::read_to_string(
        workspace_root()?.join(".github/workflows/release-publish-retry.yml"),
    )?;

    assert!(workflow.contains("path: release-source"));
    assert!(workflow.contains("ref: ${{ inputs.version }}"));
    assert!(workflow.contains("git -C release-source rev-parse HEAD"));
    assert!(workflow.contains("../release-tools/scripts/release/publish-crates.sh"));
    Ok(())
}

#[test]
fn archive_gate_release_recipe_runs_the_script_contract_test()
-> Result<(), Box<dyn std::error::Error>> {
    let justfile = std::fs::read_to_string(workspace_root()?.join("Justfile"))?;
    let recipe = recipe_body(&justfile, "release-openspec-archive")?;

    assert!(recipe.contains("bash scripts/release/check-openspec-release-archive.sh --self-test"));
    assert!(
        recipe.contains("bash scripts/release/check-openspec-release-archive.sh \"{{VERSION}}\"")
    );
    Ok(())
}

#[test]
fn coverage_gate_remains_strict_and_includes_integration_targets()
-> Result<(), Box<dyn std::error::Error>> {
    let justfile = std::fs::read_to_string(workspace_root()?.join("Justfile"))?;
    let recipe = recipe_body(&justfile, "coverage")?;

    assert!(recipe.contains("--all-targets"));
    assert!(recipe.contains("--fail-under-lines {{COVERAGE_MIN_LINES}}"));
    assert!(recipe.contains("--fail-uncovered-lines {{COVERAGE_MAX_UNCOVERED_LINES}}"));
    assert!(
        justfile
            .contains("COVERAGE_MIN_LINES := env_var_or_default(\"COVERAGE_MIN_LINES\", \"100\")")
    );
    assert!(justfile.contains("COVERAGE_MAX_UNCOVERED_LINES := env_var_or_default(\"COVERAGE_MAX_UNCOVERED_LINES\", \"0\")"));
    Ok(())
}

#[test]
fn quality_gate_requires_the_html_runtime_in_the_crate_package()
-> Result<(), Box<dyn std::error::Error>> {
    let justfile = std::fs::read_to_string(workspace_root()?.join("Justfile"))?;
    let package_check = recipe_body(&justfile, "html-runtime-package-check")?;

    assert!(
        justfile.contains("html-runtime-package-check plantuml-runtime-package-check"),
        "check must require the HTML runtime package gate"
    );
    assert!(package_check.contains("src/renderer/backends/html_runtime/dom_bootstrap.js"));
    Ok(())
}

#[test]
fn interactive_runtime_has_no_external_browser_or_helper_path()
-> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    for path in interactive_runtime_surfaces(root)? {
        assert_surface_has_no_external_browser_path(&path)?;
    }
    let session = std::fs::read_to_string(root.join(HTML_BROWSER_SESSION_PATH))?;
    let static_renderer = std::fs::read_to_string(root.join(STATIC_HTML_RENDERER_PATH))?;
    assert!(session.contains("HtmlInteractiveSession"));
    assert!(static_renderer.contains("HtmlRenderer"));
    Ok(())
}

#[test]
fn html_release_flow_never_requires_an_external_browser() -> Result<(), Box<dyn std::error::Error>>
{
    let root = workspace_root()?;
    let justfile = std::fs::read_to_string(root.join("Justfile"))?;
    let release_check = recipe_body(&justfile, "release-check")?;
    assert!(!release_check.contains("browser-install"));
    for workflow_path in [
        ".github/workflows/release-preflight.yml",
        ".github/workflows/release.yml",
    ] {
        let workflow = std::fs::read_to_string(root.join(workflow_path))?;
        for forbidden in external_browser_release_tokens() {
            assert!(
                !workflow.contains(forbidden),
                "HTML release flow must not use an external browser: {forbidden}"
            );
        }
    }
    Ok(())
}

#[test]
fn linux_release_workflows_install_the_runtime_test_prerequisites()
-> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    for workflow_path in [
        ".github/workflows/release-preflight.yml",
        ".github/workflows/release.yml",
    ] {
        let workflow = std::fs::read_to_string(root.join(workflow_path))?;
        assert!(
            workflow.contains("sudo apt-get install -y fonts-noto-cjk graphviz"),
            "{workflow_path} must install the Linux runtime test prerequisites"
        );
    }
    Ok(())
}

#[test]
fn html_platform_prerequisite_and_fallback_policy_is_contractually_documented()
-> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let readme = std::fs::read_to_string(root.join(README_PATH))?;
    let release_notes = std::fs::read_to_string(root.join(RELEASE_DOC_PATH))?;

    assert!(
        readme.contains("Platform Prerequisites for HTML"),
        "README must document HTML platform prerequisites"
    );
    assert!(
        readme.contains("system font fallback only") && readme.contains("tofu"),
        "README must document system fallback and tofu risk"
    );
    assert!(
        release_notes.contains("HTML 系プレビュー前提条件")
            && release_notes.contains("release contract"),
        "Release docs must document HTML platform prerequisites as release contract"
    );
    assert!(
        release_notes.contains("system font fallback") && release_notes.contains("tofu"),
        "Release docs must document system font fallback and tofu behavior"
    );
    assert!(
        release_notes.contains("外部ブラウザや WebView を経由せず"),
        "Release docs must explicitly state no external browser/WebView dependency"
    );
    Ok(())
}

const HTML_BROWSER_SESSION_PATH: &str =
    "crates/katana-render-runtime/src/renderer/backends/html_browser/session.rs";
const STATIC_HTML_RENDERER_PATH: &str =
    "crates/katana-render-runtime/src/renderer/backends/html.rs";
const README_PATH: &str = "README.md";
const RELEASE_DOC_PATH: &str = "docs/release.md";

fn interactive_runtime_surfaces(root: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut surfaces = [
        "Cargo.toml",
        "crates/katana-render-runtime/Cargo.toml",
        "crates/katana-render-runtime/src/lib.rs",
        "crates/katana-render-runtime/src/renderer/backends/html_css.rs",
        "crates/katana-render-runtime/src/renderer/backends/html_css_rule.rs",
        "crates/katana-render-runtime/src/renderer/backends/html_css_selector.rs",
        "crates/katana-render-runtime/src/renderer/backends/html_document.rs",
        "crates/katana-render-runtime/src/renderer/backends/html_dom_helpers.rs",
        "crates/katana-render-runtime/src/renderer/backends/html_browser/mod.rs",
        "crates/katana-render-runtime/src/renderer/backends/html_runtime.rs",
        HTML_BROWSER_SESSION_PATH,
    ]
    .map(|relative_path| root.join(relative_path))
    .to_vec();

    for relative_directory in [
        "crates/katana-render-runtime/src/renderer/backends/html_browser",
        "crates/katana-render-runtime/src/renderer/backends/html_interactive",
        "crates/katana-render-runtime/src/renderer/backends/html_runtime",
        "crates/katana-render-runtime/src/renderer/backends/html_subresources",
    ] {
        collect_production_rust_sources(&root.join(relative_directory), &mut surfaces)?;
    }

    surfaces.sort();
    surfaces.dedup();
    Ok(surfaces)
}

fn collect_production_rust_sources(
    directory: &Path,
    surfaces: &mut Vec<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_production_rust_sources(&path, surfaces)?;
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if path.extension().and_then(|extension| extension.to_str()) == Some("rs")
            && file_name != "tests.rs"
            && !file_name.ends_with("_tests.rs")
        {
            surfaces.push(path);
        }
    }
    Ok(())
}

fn assert_surface_has_no_external_browser_path(
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let surface = std::fs::read_to_string(path)?;
    for forbidden in forbidden_external_browser_surfaces() {
        assert!(
            !surface.contains(forbidden),
            "external browser surface must not re-enter KRR: {forbidden}"
        );
    }
    Ok(())
}

fn forbidden_external_browser_surfaces() -> [&'static str; 6] {
    [
        "headless_chrome",
        "html_chromium_engine",
        "HtmlBrowserProcess",
        "HtmlBrowserProcessConfig",
        "HtmlBrowserCommand",
        "HTML_BROWSER_PROTOCOL_VERSION",
    ]
}

fn external_browser_release_tokens() -> [&'static str; 5] {
    [
        "KRR_CHROMIUM",
        "KRR_CHROME_BIN",
        "krr-html-chromium",
        "html_chromium_engine",
        "Enable Chromium user namespace sandbox",
    ]
}

fn workspace_root() -> Result<&'static Path, Box<dyn std::error::Error>> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .ok_or_else(|| "workspace root is unavailable".into())
}

fn recipe_body<'a>(justfile: &'a str, recipe: &str) -> Result<&'a str, Box<dyn std::error::Error>> {
    let header = format!("{recipe}:");
    let start = justfile
        .find(&header)
        .ok_or_else(|| format!("{recipe} recipe is missing"))?;
    let body = &justfile[start + header.len()..];
    Ok(body.split("\n\n").next().unwrap_or(body))
}

fn release_target_check(
    root: &Path,
    target_version: &str,
    latest_version: &str,
    head_ref: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let output = Command::new("python3")
        .args([
            "scripts/release/verify-release-target.py",
            "--target-version",
            target_version,
            "--latest-version",
            latest_version,
            "--head-ref",
            head_ref,
        ])
        .current_dir(root)
        .output()?;
    Ok(output.status.success())
}
