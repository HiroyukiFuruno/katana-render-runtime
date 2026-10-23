use std::{
    path::{Path, PathBuf},
    process::Command,
};

const PUBLISHED_V0422_RELEASE_HEAD: &str = "185d056de67282a4056729296e57c164a6a343d6";
const PUBLISHED_V0422_LEGACY_PIN: &str = "1bc497bdfd3b8c4318e9ee1609d147b6925b43e5";
const PUBLISHED_V0422_MANIFEST: &str =
    "1ddeddc37c0735a7278755f26f8985aee9176fc978b437766d57be7d0e342d9e";

#[test]
fn release_check_requires_all_quality_and_publish_readiness_gates()
-> Result<(), Box<dyn std::error::Error>> {
    let justfile = std::fs::read_to_string(workspace_root()?.join("Justfile"))?;
    let release_check = recipe_body(&justfile, "release-check")?;
    let quality = recipe_body(&justfile, "release-quality")?;
    let specific = recipe_body(&justfile, "release-specific")?;

    for required_gate in ["release-quality", "release-specific"] {
        assert!(
            release_check.contains(required_gate),
            "release-check must require {required_gate}"
        );
    }
    for required_gate in ["check", "html-performance-nonregression-check"] {
        assert!(
            quality.contains(required_gate),
            "release-quality must require {required_gate}"
        );
    }
    for required_gate in ["release-openspec-archive", "release-verify"] {
        assert!(
            specific.contains(required_gate),
            "release-specific must require {required_gate}"
        );
    }
    Ok(())
}

#[test]
fn release_verify_tests_the_packaged_library_sources() -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let justfile = std::fs::read_to_string(root.join("Justfile"))?;
    let recipe = recipe_body(&justfile, "release-verify")?;
    let packaged_runtime_test =
        std::fs::read_to_string(root.join("scripts/release/test_packaged_runtime.py"))?;

    assert!(recipe.contains("python3 scripts/release/test_packaged_runtime.py"));
    for required_option in ["\"--lib\"", "\"--locked\"", "\"--offline\""] {
        assert!(
            packaged_runtime_test.contains(required_option),
            "packaged runtime test must retain {required_option}"
        );
    }
    Ok(())
}

#[test]
fn release_target_check_requires_v0_4_24_intent() -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    assert_release_target_check(root, "0.4.24", "0.4.23", "missing-release-head", false)?;
    assert_release_target_check(root, "0.4.24", "0.4.22", "HEAD", false)?;
    assert_release_target_check(root, "0.4.25", "0.4.23", "HEAD", false)?;
    assert_release_target_check(
        root,
        "0.4.24",
        "0.4.23",
        "8654789f69d24401b8ae67036db1cdb3e7f4294e",
        false,
    )?;
    for version in [
        "0.3.9", "0.4.0", "0.4.1", "0.4.2", "0.4.3", "0.4.4", "0.4.5", "0.4.6", "0.4.7", "0.4.8",
        "0.4.9", "0.4.10", "0.4.11", "0.4.12", "0.4.13", "0.4.14", "0.4.15", "0.4.16", "0.4.17",
        "0.4.18", "0.4.19", "0.4.20", "0.4.21", "0.4.22", "0.4.23", "0.5.0", "1.0.0", "2.0.0",
    ] {
        assert_release_target_check(root, version, "0.4.23", "HEAD", false)?;
    }
    Ok(())
}

#[test]
fn v0_4_24_validation_snapshot_passes_the_release_contract()
-> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let snapshot = current_release_validation_snapshot(root)?;
    assert_release_target_check(root, "0.4.24", "0.4.23", &snapshot, true)?;
    assert_release_target_check(root, "0.4.24", "0.4.24", &snapshot, true)?;
    Ok(())
}

#[test]
fn published_v0_4_22_checker_keeps_its_historical_contract()
-> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let checker = historical_release_target_checker(root)?;
    assert_historical_v0422_versions(root, &checker)?;
    assert_historical_v0422_pin(root, &checker)?;
    assert_historical_v0422_manifest(root, &checker)?;
    Ok(())
}

fn assert_historical_v0422_versions(
    root: &Path,
    checker: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    for (target, latest, expected) in [
        ("0.4.22", "0.4.21", true),
        ("0.4.22", "0.4.22", true),
        ("0.4.23", "0.4.22", false),
        ("0.4.22", "0.4.20", false),
        ("0.5.0", "0.4.21", false),
    ] {
        assert_eq!(
            historical_release_target_check(
                root,
                checker,
                target,
                latest,
                PUBLISHED_V0422_RELEASE_HEAD,
            )?,
            expected,
            "target={target}, latest={latest}"
        );
    }
    Ok(())
}

fn assert_historical_v0422_pin(
    root: &Path,
    checker: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let parent = Command::new("git")
        .args(["rev-parse", &format!("{PUBLISHED_V0422_LEGACY_PIN}^")])
        .current_dir(root)
        .output()?;
    if !parent.status.success() {
        return Err("historical release pin parent is unavailable".into());
    }
    let parent = String::from_utf8(parent.stdout)?;
    assert!(!historical_release_target_check(
        root,
        checker,
        "0.4.22",
        "0.4.21",
        parent.trim(),
    )?);
    Ok(())
}

fn assert_historical_v0422_manifest(
    root: &Path,
    checker: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let printed_manifest = Command::new("python3")
        .arg("-c")
        .arg(checker)
        .args([
            "--target-version",
            "v0.4.22",
            "--head-ref",
            PUBLISHED_V0422_RELEASE_HEAD,
            "--print-release-manifest",
        ])
        .current_dir(root)
        .output()?;
    if !printed_manifest.status.success() {
        return Err("historical release manifest command failed".into());
    }
    assert_eq!(
        String::from_utf8(printed_manifest.stdout)?.trim(),
        PUBLISHED_V0422_MANIFEST
    );
    Ok(())
}

fn assert_release_target_check(
    root: &Path,
    target: &str,
    latest: &str,
    head_ref: &str,
    expected: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        release_target_check(root, target, latest, head_ref)?,
        expected,
        "target={target}, latest={latest}, head={head_ref}"
    );
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
    let root = workspace_root()?;
    let justfile = std::fs::read_to_string(root.join("Justfile"))?;
    let coverage = recipe_body(&justfile, "coverage")?;
    let recipe = recipe_body(&justfile, "root-test-and-coverage")?;
    let aggregate = recipe_body(&justfile, "coverage-result-check")?;

    assert_coverage_recipe_arms(coverage);
    assert_root_coverage_contract(root, &justfile, recipe)?;
    assert_coverage_aggregate_contract(aggregate);
    assert_coverage_defaults(&justfile);
    Ok(())
}

fn assert_coverage_recipe_arms(coverage: &str) {
    assert_eq!(
        coverage
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>(),
        [
            "just root-test-and-coverage",
            "just performance-harness-coverage",
            "just coverage-result-check",
        ]
    );
}

fn assert_root_coverage_contract(
    root: &Path,
    justfile: &str,
    recipe: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let producer =
        std::fs::read_to_string(root.join("scripts/release/native_nextest_root_coverage.py"))?;
    let commands =
        std::fs::read_to_string(root.join("scripts/release/native_nextest_coverage_commands.py"))?;
    assert_root_coverage_recipe(justfile, recipe);
    assert_root_coverage_producer(&producer);
    assert_root_coverage_commands(&commands);
    assert_root_coverage_stage_order(&producer)?;
    assert_contains_none(
        &format!("{recipe}\n{producer}\n{commands}"),
        &[
            "--fail-under-lines",
            "--fail-uncovered-lines",
            "rm -rf target/",
            "cargo clean",
            "--ignore-run-fail",
        ],
    );
    Ok(())
}

fn assert_root_coverage_recipe(justfile: &str, recipe: &str) {
    assert!(
        justfile
            .lines()
            .any(|line| line == "root-test-and-coverage: plantuml-install")
    );
    assert_eq!(
        recipe.trim(),
        "plantuml-install\n    CARGO={{quote(CARGO)}} TEST_THREADS={{quote(TEST_THREADS)}} python3 scripts/release/native_nextest_root_coverage.py --repo \"{{REPO_ROOT}}\""
    );
}

fn assert_root_coverage_producer(producer: &str) {
    assert_contains_all(
        producer,
        &[
            "cargo = shlex.split(os.environ.get(\"CARGO\", \"cargo\"))",
            "os.environ.get(\"TEST_THREADS\", \"1\")",
            "clean_arguments()",
            "nextest_arguments(repo, threads, parent)",
            "report_arguments(repo)",
            "local.local_collect_finish(repo, \"root\", 0)",
        ],
    );
}

fn assert_root_coverage_commands(commands: &str) {
    assert_contains_all(
        commands,
        &[
            "\"llvm-cov\", \"clean\", \"--workspace\", \"--profraw-only\"",
            "\"llvm-cov\", \"nextest\", \"--workspace\", \"--all-targets\", \"--all-features\"",
            "\"--cargo-profile\", \"krr-coverage\", \"--no-report\"",
            "\"llvm-cov\", \"report\", \"--profile\", \"krr-coverage\"",
            "\"--show-missing-lines\", \"--output-path\", str(repo / REPORT_DIRECTORY / \"root.json\")",
        ],
    );
}

fn assert_root_coverage_stage_order(producer: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut remaining = producer
        .split("def run_root(repo: Path) -> int:")
        .nth(1)
        .ok_or("root coverage producer must define run_root")?;
    for stage in [
        "identity_sha, cacheable = _begin(repo)",
        "code, run_id = _test(repo, cargo, os.environ.get(\"TEST_THREADS\", \"1\"),",
        "report = _guarded(repo, \"--coverage-collection-domain\", cargo, report_arguments(repo))",
        "code = _execute(directory, \"report\", report, repo, _child_environment(repo))",
        "local.local_collect_finish(repo, \"root\", 0)",
    ] {
        let position = remaining
            .find(stage)
            .ok_or("missing ordered root coverage stage")?;
        remaining = &remaining[position + stage.len()..];
    }
    Ok(())
}

fn assert_coverage_aggregate_contract(recipe: &str) {
    assert_eq!(
        recipe
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>(),
        ["python3 scripts/release/native_coverage_local_aggregate.py --repo \"{{REPO_ROOT}}\""]
    );
}

fn assert_coverage_defaults(justfile: &str) {
    assert_contains_all(
        justfile,
        &[
            "COVERAGE_MIN_LINES := env_var_or_default(\"COVERAGE_MIN_LINES\", \"100\")",
            "COVERAGE_MAX_UNCOVERED_LINES := env_var_or_default(\"COVERAGE_MAX_UNCOVERED_LINES\", \"0\")",
        ],
    );
}

#[test]
fn quality_gate_requires_the_html_runtime_in_the_crate_package()
-> Result<(), Box<dyn std::error::Error>> {
    let justfile = std::fs::read_to_string(workspace_root()?.join("Justfile"))?;
    let package_check = recipe_body(&justfile, "html-runtime-package-check")?;
    let rust_lane = recipe_body(&justfile, "check-rust")?;

    assert!(
        rust_lane.contains("just html-runtime-package-check"),
        "the Cargo lane must require the HTML runtime package gate"
    );
    assert!(rust_lane.contains("just plantuml-runtime-package-check"));
    assert_contains_all(
        package_check,
        &[
            "src/renderer/backends/html_runtime/dom_bootstrap.js",
            "assets/test-fixtures/font-probes/recursive-latin-crsv-normal.ttf",
            "assets/test-fixtures/font-probes/native-class-ja-probe.svg",
            "assets/test-fixtures/font-probes/licenses/Recursive-OFL-1.1.txt",
        ],
    );
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
fn pre_push_uses_the_ordered_issue_contract_dispatcher() -> Result<(), Box<dyn std::error::Error>> {
    let (lefthook, dispatcher) = pre_push_sources()?;
    assert!(lefthook.contains("run: bash scripts/hooks/pre-push.sh"));
    let issue_contract = dispatcher
        .find("python3 scripts/hooks/verify_push_issue.py")
        .ok_or("Issue contract is missing from pre-push dispatcher")?;
    let first_head_guard = dispatcher
        .find("python3 scripts/hooks/pre_push_head_guard.py \"${reviewed_head}\" <\"${updates}\"")
        .ok_or("first reviewed-HEAD guard is missing from pre-push dispatcher")?;
    let repository_check = dispatcher
        .find("just check")
        .ok_or("repository check is missing from pre-push dispatcher")?;
    let second_head_guard = dispatcher
        .rfind("python3 scripts/hooks/pre_push_head_guard.py \"${reviewed_head}\" <\"${updates}\"")
        .ok_or("second reviewed-HEAD guard is missing from pre-push dispatcher")?;
    assert!(
        issue_contract < first_head_guard,
        "the cheap Issue contract must run before the first reviewed-HEAD guard"
    );
    assert!(
        first_head_guard < repository_check && repository_check < second_head_guard,
        "the repository check must run between the reviewed-HEAD guards"
    );
    assert_eq!(
        dispatcher
            .matches("python3 scripts/hooks/pre_push_head_guard.py")
            .count(),
        2,
        "pre-push must guard the reviewed HEAD both before and after repository checks"
    );
    Ok(())
}

#[test]
fn pre_push_captures_reviewed_head_and_retains_push_updates()
-> Result<(), Box<dyn std::error::Error>> {
    let (_, dispatcher) = pre_push_sources()?;
    let updates_capture = dispatcher
        .find("cat >\"${updates}\"")
        .ok_or("push updates are not retained by pre-push dispatcher")?;
    let reviewed_head_capture = dispatcher
        .find("reviewed_head=\"$(git rev-parse HEAD)\"")
        .ok_or("reviewed HEAD is not captured by pre-push dispatcher")?;
    let issue_contract = dispatcher
        .find("python3 scripts/hooks/verify_push_issue.py")
        .ok_or("Issue contract is missing from pre-push dispatcher")?;
    assert!(
        updates_capture < reviewed_head_capture && reviewed_head_capture < issue_contract,
        "push updates and reviewed HEAD must be captured before validation"
    );
    assert_eq!(
        dispatcher
            .matches("reviewed_head=\"$(git rev-parse HEAD)\"")
            .count(),
        1,
        "the reviewed HEAD must be captured exactly once for both guards"
    );
    assert_eq!(
        dispatcher.matches("<\"${updates}\"").count(),
        4,
        "both Issue dispatch branches and both HEAD guards must read retained push updates"
    );
    Ok(())
}

fn pre_push_sources() -> Result<(String, String), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let lefthook = std::fs::read_to_string(root.join("lefthook.yml"))?;
    let dispatcher = std::fs::read_to_string(root.join("scripts/hooks/pre-push.sh"))?;
    Ok((lefthook, dispatcher))
}

#[test]
fn local_quality_gate_runs_repository_automation_contract_tests()
-> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let justfile = std::fs::read_to_string(root.join("Justfile"))?;
    let check = recipe_body(&justfile, "check")?;
    let automation = recipe_body(&justfile, "automation-contract-test")?;
    let scheduler = std::fs::read_to_string(root.join("scripts/hooks/run_parallel_checks.py"))?;
    let moon = std::fs::read_to_string(root.join("moon.yml"))?;
    let moon_workspace = std::fs::read_to_string(root.join(".moon/workspace.yml"))?;

    assert!(check.contains("run_parallel_checks.py"));
    assert_moon_quality_dag(&moon, &scheduler, &moon_workspace);
    assert_python_automation_discovery(automation);
    Ok(())
}

#[test]
fn dependency_update_all_keeps_direct_transitive_and_strict_quality_gates()
-> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let justfile = std::fs::read_to_string(root.join("Justfile"))?;
    let recipe = recipe_body(&justfile, "depends-update-all")?;
    let commands = recipe_body(&justfile, "depends-update-mutations")?
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    let positions = dependency_update_command_positions(&commands)?;

    assert_dependency_update_command_order(&commands, positions);
    let scheduler = std::fs::read_to_string(root.join("scripts/hooks/run_parallel_checks.py"))?;
    let moon = std::fs::read_to_string(root.join("moon.yml"))?;
    let moon_workspace = std::fs::read_to_string(root.join(".moon/workspace.yml"))?;
    assert_dependency_update_quality_gates(recipe, &commands, &scheduler, &moon, &moon_workspace);
    Ok(())
}

fn dependency_update_command_positions(
    commands: &[&str],
) -> Result<(usize, usize, usize, usize), std::io::Error> {
    let broad_upgrade = commands
        .iter()
        .position(|line| line.starts_with("{{CARGO}} upgrade "))
        .ok_or_else(|| std::io::Error::other("broad Cargo upgrade command is missing"))?;
    let html_pair_update = commands
        .iter()
        .position(|line| line.contains("scripts/release/update_html5ever_pair.py"))
        .ok_or_else(|| std::io::Error::other("html5ever pair update command is missing"))?;
    let lockfile_update = commands
        .iter()
        .position(|line| *line == "{{CARGO}} update")
        .ok_or_else(|| std::io::Error::other("Cargo lockfile update command is missing"))?;
    let usvg_skrifa_pair_update = commands
        .iter()
        .position(|line| line.contains("scripts/release/update_usvg_skrifa_pair.py"))
        .ok_or_else(|| std::io::Error::other("usvg/skrifa pair update command is missing"))?;

    Ok((
        broad_upgrade,
        html_pair_update,
        lockfile_update,
        usvg_skrifa_pair_update,
    ))
}

fn assert_dependency_update_command_order(
    commands: &[&str],
    positions: (usize, usize, usize, usize),
) {
    let (broad_upgrade, html_pair_update, lockfile_update, usvg_skrifa_pair_update) = positions;
    assert_eq!(
        commands[broad_upgrade],
        "{{CARGO}} upgrade -i allow --pinned allow --exclude skrifa"
    );
    assert_eq!(
        commands[html_pair_update],
        "python3 scripts/release/update_html5ever_pair.py --cargo \"{{CARGO}}\""
    );
    assert!(broad_upgrade < html_pair_update && html_pair_update < lockfile_update);
    assert_usvg_skrifa_pair_order(commands, lockfile_update, usvg_skrifa_pair_update);
}

fn assert_usvg_skrifa_pair_order(commands: &[&str], lockfile_update: usize, pair_update: usize) {
    assert_eq!(
        commands[pair_update],
        "python3 scripts/release/update_usvg_skrifa_pair.py --cargo \"{{CARGO}}\""
    );
    assert_eq!(
        commands
            .iter()
            .filter(|line| line.contains("scripts/release/update_usvg_skrifa_pair.py"))
            .count(),
        1,
        "usvg/skrifa pair synchronization must run exactly once"
    );
    assert!(
        lockfile_update < pair_update
            && commands
                .iter()
                .position(|line| {
                    *line == "python3 scripts/release/update_html_performance_harness_lock.py --cargo \"{{CARGO}}\""
                })
                .zip(commands
                    .iter()
                    .position(|line| *line == "bun update --latest '!@moonrepo/*'"))
                .is_some_and(|(harness_lock_update, bun_update)| {
                    pair_update < harness_lock_update && harness_lock_update < bun_update
                }),
        "dependency updates must synchronize both Rust pairs and the harness lock before Bun"
    );
}

fn assert_dependency_update_quality_gates(
    recipe: &str,
    mutations: &[&str],
    scheduler: &str,
    moon: &str,
    moon_workspace: &str,
) {
    assert_ordered_dependency_update_stages(recipe);
    assert_dependency_mutation_commands(mutations);
    assert_eq!(recipe.matches("just check").count(), 1);
    assert!(!recipe.contains("just coverage"));
    assert_moon_quality_dag(moon, scheduler, moon_workspace);
}

fn assert_ordered_dependency_update_stages(recipe: &str) {
    let ordered_stages = [
        "python3 scripts/hooks/run_locked_command.py --protect-root-producer -- just depends-update-mutations",
        "just check",
        "just mermaid-compare-full",
        "just drawio-compare-full",
    ]
    .map(|stage| recipe.find(stage));
    assert!(
        ordered_stages.iter().all(Option::is_some),
        "depends-update-all is missing a required stage: {ordered_stages:?}"
    );
    assert!(
        ordered_stages.windows(2).all(|pair| pair[0] < pair[1]),
        "dependency mutations, quality checks, and full comparisons must stay ordered"
    );
}

fn assert_dependency_mutation_commands(mutations: &[&str]) {
    for required in [
        "{{CARGO}} update",
        "bun update --latest",
        "runtime-assets/depends-update-all.ts",
        "scripts/drawio/resource-update.ts",
        "runtime-package-asset-compressor.ts --write",
        "just fmt",
        "just runtime-bundle-build",
    ] {
        assert!(
            mutations.iter().any(|command| command.contains(required)),
            "depends-update-mutations must require {required}"
        );
    }
}

#[test]
fn release_workflow_runs_safe_cleanup_after_crates_publish()
-> Result<(), Box<dyn std::error::Error>> {
    let workflow =
        std::fs::read_to_string(workspace_root()?.join(".github/workflows/release.yml"))?;
    let publish = workflow
        .find("- name: Publish crates.io")
        .ok_or("crates.io publish step is missing")?;
    let cleanup = workflow
        .find("- name: Cleanup published release state")
        .ok_or("release cleanup step is missing")?;

    assert!(
        publish < cleanup,
        "cleanup must run after crates.io publish"
    );
    assert!(workflow.contains("python3 scripts/release/cleanup_release_state.py"));
    assert!(workflow.contains("--release-branch \"${RELEASE_BRANCH}\""));
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
    let mut start = None;
    let mut offset = 0;
    for line in justfile.split_inclusive('\n') {
        let header = line.trim_end_matches(['\r', '\n']);
        let Some((name, _)) = header.split_once(':') else {
            offset += line.len();
            continue;
        };
        let name = name.trim();
        let is_recipe = name == recipe
            || name
                .strip_prefix(recipe)
                .is_some_and(|parameters| parameters.starts_with(char::is_whitespace));
        if is_recipe {
            start = Some((offset, name.len()));
            break;
        }
        offset += line.len();
    }
    let (start, header_len) = start.ok_or_else(|| format!("{recipe} recipe is missing"))?;
    let body = &justfile[start + header_len + 1..];
    Ok(body.split("\n\n").next().unwrap_or(body))
}

fn assert_contains_all(source: &str, required: &[&str]) {
    for fragment in required {
        assert!(
            source.contains(fragment),
            "missing required fragment: {fragment}"
        );
    }
}

fn assert_contains_none(source: &str, forbidden: &[&str]) {
    for fragment in forbidden {
        assert!(
            !source.contains(fragment),
            "unexpected fragment: {fragment}"
        );
    }
}

fn assert_moon_quality_dag(moon: &str, scheduler: &str, moon_workspace: &str) {
    assert!(scheduler.contains("\"krr:coverage\""));
    assert!(moon_workspace.contains("krr: '.'"));
    assert_contains_all(
        moon,
        &[
            "  performance-coverage:\n    command: ['just', 'performance-harness-coverage']\n    deps: ['heavy-native', 'check']",
            "  coverage:\n    command: ['just', 'coverage-result-check']\n    deps: ['performance-coverage']",
            "  unit-test:\n    command: ['just', 'root-test-and-coverage']\n    deps: ['runtime-asset-script-test']",
            "  check:\n    command: noop\n    inputs: *quality-inputs\n    deps:\n      - heavy-native\n      - runtime-asset-script-test\n      - automation-contract-test\n      - build-capacity-check",
        ],
    );
    for task in ["automation-contract-test", "build-capacity-check"] {
        assert!(moon.contains(&format!(
            "  {task}:\n    command: ['just', '{task}']\n    deps: ['static']"
        )));
    }
}

fn assert_python_automation_discovery(automation: &str) {
    for script_dir in ["scripts/hooks", "scripts/release"] {
        assert!(
            automation.contains(&format!("unittest discover -s {script_dir} -p '*_test.py'")),
            "automation contract gate must discover tests under {script_dir}"
        );
    }
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

fn current_release_validation_snapshot(root: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let contract =
        std::fs::read_to_string(root.join("scripts/release/verify_release_target_test.py"))?;
    let snapshot = parse_release_validation_snapshot(&contract)?;
    verify_release_validation_snapshot_commit(root, snapshot)?;
    Ok(snapshot.to_owned())
}

fn parse_release_validation_snapshot(contract: &str) -> Result<&str, Box<dyn std::error::Error>> {
    const DECLARATION: &str = "CURRENT_RELEASE_VALIDATION_SNAPSHOT = ";
    let declarations: Vec<_> = contract
        .lines()
        .filter(|line| {
            line.trim_start()
                .starts_with("CURRENT_RELEASE_VALIDATION_SNAPSHOT")
        })
        .collect();
    if declarations.len() != 1 {
        return Err("release validation snapshot declaration must occur exactly once".into());
    }
    let snapshot = declarations[0]
        .trim_start()
        .strip_prefix(DECLARATION)
        .and_then(|value| value.strip_prefix('"'))
        .and_then(|value| value.strip_suffix('"'))
        .ok_or("release validation snapshot declaration is malformed")?;
    if snapshot.len() != 40
        || !snapshot
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err("release validation snapshot must be a lowercase 40-digit SHA".into());
    }
    Ok(snapshot)
}

fn verify_release_validation_snapshot_commit(
    root: &Path,
    snapshot: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let resolved = Command::new("git")
        .args(["rev-parse", "--verify", &format!("{snapshot}^{{commit}}")])
        .current_dir(root)
        .output()?;
    if !resolved.status.success() || String::from_utf8(resolved.stdout)?.trim() != snapshot {
        return Err("release validation snapshot does not resolve to that commit".into());
    }
    Ok(())
}

fn historical_release_target_checker(root: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args([
            "show",
            "185d056de67282a4056729296e57c164a6a343d6:scripts/release/verify-release-target.py",
        ])
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err("published v0.4.22 release checker is unavailable".into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

fn historical_release_target_check(
    root: &Path,
    checker: &str,
    target: &str,
    latest: &str,
    head_ref: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let output = Command::new("python3")
        .arg("-c")
        .arg(checker)
        .args([
            "--target-version",
            target,
            "--latest-version",
            latest,
            "--head-ref",
            head_ref,
        ])
        .current_dir(root)
        .output()?;
    Ok(output.status.success())
}
