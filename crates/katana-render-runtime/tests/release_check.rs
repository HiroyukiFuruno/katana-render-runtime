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
fn pre_push_uses_the_ordered_issue_contract_dispatcher() -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let lefthook = std::fs::read_to_string(root.join("lefthook.yml"))?;
    let dispatcher = std::fs::read_to_string(root.join("scripts/hooks/pre-push.sh"))?;

    assert!(lefthook.contains("run: bash scripts/hooks/pre-push.sh"));
    let repository_check = dispatcher
        .find("just check")
        .ok_or("repository check is missing from pre-push dispatcher")?;
    let issue_contract = dispatcher
        .find("python3 scripts/hooks/verify_push_issue.py")
        .ok_or("Issue contract is missing from pre-push dispatcher")?;
    assert!(
        repository_check < issue_contract,
        "repository-specific check must run before the Issue contract"
    );
    Ok(())
}

#[test]
fn local_quality_gate_runs_repository_automation_contract_tests()
-> Result<(), Box<dyn std::error::Error>> {
    let justfile = std::fs::read_to_string(workspace_root()?.join("Justfile"))?;
    let check = recipe_body(&justfile, "check")?;
    let automation = recipe_body(&justfile, "automation-contract-test")?;

    assert!(check.contains("automation-contract-test"));
    assert!(automation.contains("unittest discover -s scripts/hooks"));
    assert!(automation.contains("cleanup_release_state_test.py"));
    Ok(())
}

#[test]
fn dependency_update_all_keeps_direct_transitive_and_strict_quality_gates()
-> Result<(), Box<dyn std::error::Error>> {
    let justfile = std::fs::read_to_string(workspace_root()?.join("Justfile"))?;
    let recipe = recipe_body(&justfile, "depends-update-all")?;

    for required in [
        "{{CARGO}} upgrade -i",
        "{{CARGO}} update",
        "bun update --latest",
        "runtime-assets/depends-update-all.ts",
        "just mermaid-compare-full",
        "just drawio-compare-full",
        "just check",
        "just coverage",
    ] {
        assert!(
            recipe.contains(required),
            "depends-update-all must require {required}"
        );
    }
    Ok(())
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
fn pull_requests_require_draft_review_completion_before_ready()
-> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    assert_agent_pr_review_contract(root)?;
    assert_primary_pr_skill_contracts(root)?;
    assert_governed_skill_contracts(root)?;
    assert_review_evidence_contract(root)?;
    assert_pr_ready_recipe_contract(root)?;
    assert_governance_workflow_contract(root)?;
    Ok(())
}

#[test]
fn every_push_restarts_review_from_current_initial_snapshot() -> TestResult {
    let root = workspace_root()?;
    for path in [
        ".agents/skills/commit_and_push/SKILL.md",
        ".codex/skills/commit_and_push/SKILL.md",
    ] {
        let skill = std::fs::read_to_string(root.join(path))?;
        assert_in_order(
            &skill,
            &[
                RequiredTerm::Exact("push 後、GitHub API から current PR"),
                RequiredTerm::Exact("新しい push は新しい HEAD と current 本文に対する **initial review**"),
                RequiredTerm::Exact("以前の final reviewを同一修正循環へ持ち越さない"),
                RequiredTerm::Exact("phase=initial head=<40-lowerhex> body-sha256=<64-lowerhex>"),
                RequiredTerm::Exact("@codex review"),
                RequiredTerm::Exact("initial review の指摘を解消して thread reply/resolve"),
                RequiredTerm::Exact("marker投稿直前に current PR の HEAD・本文を再取得"),
                RequiredTerm::Exact("取得値が initial marker と異なる場合は final を投稿せず、新しい initial review からやり直す"),
                RequiredTerm::Exact("phase=final head=<40-lowerhex> body-sha256=<64-lowerhex>"),
                RequiredTerm::Exact("initial/final のいずれでも新規指摘が出たら"),
                RequiredTerm::Exact("修正 → push → 新HEAD・本文の initial review から反復する"),
            ],
        )
        .map_err(|error| format!("{path}: {error}"))?;
    }
    Ok(())
}

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Debug)]
enum RequiredTerm {
    Exact(&'static str),
    Alternatives(&'static [&'static str]),
}

impl RequiredTerm {
    fn is_present(&self, text: &str) -> bool {
        self.first_position_at_or_after(text, 0).is_some()
    }

    fn first_position_at_or_after(&self, text: &str, start: usize) -> Option<(usize, usize)> {
        match self {
            Self::Exact(term) => text[start..]
                .find(term)
                .map(|offset| (start + offset, term.len())),
            Self::Alternatives(terms) => terms
                .iter()
                .filter_map(|term| {
                    text[start..]
                        .find(term)
                        .map(|offset| (start + offset, term.len()))
                })
                .min_by_key(|(position, _)| *position),
        }
    }
}

struct SkillContract {
    path: &'static str,
    required_terms: &'static [RequiredTerm],
    promotes_ready: bool,
}

const PRIMARY_SKILL_ORDER: &[RequiredTerm] = &[
    RequiredTerm::Exact("gh pr create --draft"),
    RequiredTerm::Exact("krr-review phase=initial"),
    RequiredTerm::Exact("krr-review phase=final"),
    RequiredTerm::Exact("pr-ready-check"),
    RequiredTerm::Exact("gh pr ready"),
];

const READY_PROMOTION_ORDER: &[RequiredTerm] = &[
    RequiredTerm::Exact("Draft"),
    RequiredTerm::Exact("initial"),
    RequiredTerm::Exact("final"),
    RequiredTerm::Exact("pr-ready-check"),
    RequiredTerm::Exact("gh pr ready"),
];

const SKILL_CONTRACTS: &[SkillContract] = &[
    SkillContract {
        path: ".codex/skills/commit_and_push/SKILL.md",
        required_terms: &[
            RequiredTerm::Exact("Draft"),
            RequiredTerm::Exact("final"),
            RequiredTerm::Exact("subagent"),
            RequiredTerm::Alternatives(&["reply", "返信"]),
            RequiredTerm::Alternatives(&["resolve", "解決"]),
            RequiredTerm::Exact("pr-ready-check"),
            RequiredTerm::Exact("gh pr ready <number>"),
            RequiredTerm::Exact("freshなmerge承認"),
            RequiredTerm::Exact("merge --apply"),
        ],
        promotes_ready: true,
    },
    SkillContract {
        path: ".codex/skills/create_pull_request/SKILL.md",
        required_terms: &[
            RequiredTerm::Exact("Draft"),
            RequiredTerm::Exact("initial"),
            RequiredTerm::Exact("subagent"),
            RequiredTerm::Alternatives(&["reply", "返信"]),
            RequiredTerm::Alternatives(&["resolve", "解決"]),
            RequiredTerm::Exact("final"),
            RequiredTerm::Exact("pr-ready-check"),
            RequiredTerm::Exact("Ready"),
        ],
        promotes_ready: true,
    },
    SkillContract {
        path: ".codex/skills/impl-release/SKILL.md",
        required_terms: &[
            RequiredTerm::Exact("Draft"),
            RequiredTerm::Exact("initial"),
            RequiredTerm::Exact("subagent"),
            RequiredTerm::Alternatives(&["reply", "返信"]),
            RequiredTerm::Alternatives(&["resolve", "解決"]),
            RequiredTerm::Exact("final"),
            RequiredTerm::Exact("pr-ready-check"),
            RequiredTerm::Exact("Ready"),
        ],
        promotes_ready: true,
    },
    SkillContract {
        path: ".codex/skills/kdr-workflow-guide/SKILL.md",
        required_terms: &[
            RequiredTerm::Exact("Draft"),
            RequiredTerm::Exact("initial"),
            RequiredTerm::Exact("subagent"),
            RequiredTerm::Alternatives(&["reply", "返信"]),
            RequiredTerm::Alternatives(&["resolve", "解決"]),
            RequiredTerm::Exact("final"),
            RequiredTerm::Exact("pr-ready-check"),
            RequiredTerm::Exact("Ready"),
        ],
        promotes_ready: true,
    },
    SkillContract {
        path: ".codex/skills/self-review/SKILL.md",
        required_terms: &[
            RequiredTerm::Exact("Draft"),
            RequiredTerm::Exact("initial"),
            RequiredTerm::Exact("subagent"),
            RequiredTerm::Alternatives(&["reply", "返信"]),
            RequiredTerm::Alternatives(&["resolve", "解決"]),
            RequiredTerm::Exact("final"),
            RequiredTerm::Exact("pr-ready-check"),
        ],
        promotes_ready: false,
    },
    SkillContract {
        path: ".codex/skills/gh-address-comments/SKILL.md",
        required_terms: &[
            RequiredTerm::Exact("Draft"),
            RequiredTerm::Exact("subagent"),
            RequiredTerm::Alternatives(&["reply", "返信"]),
            RequiredTerm::Alternatives(&["resolve", "解決"]),
            RequiredTerm::Exact("final"),
            RequiredTerm::Exact("pr-ready-check"),
        ],
        promotes_ready: false,
    },
];

fn assert_agent_pr_review_contract(root: &Path) -> TestResult {
    let agents = std::fs::read_to_string(root.join("AGENTS.md"))?;
    for required in [
        "Draft",
        "review thread",
        "Ready",
        "司令塔",
        "空き枠",
        "利用不可",
    ] {
        assert!(
            agents.contains(required),
            "AGENTS.md must require {required} in the PR review gate"
        );
    }
    Ok(())
}

fn assert_primary_pr_skill_contracts(root: &Path) -> TestResult {
    for path in [
        ".codex/skills/create_pull_request/SKILL.md",
        ".agents/skills/create_pull_request/SKILL.md",
        ".codex/skills/impl-release/SKILL.md",
    ] {
        assert_primary_skill_contract(root, path)?;
    }
    Ok(())
}

fn assert_primary_skill_contract(root: &Path, path: &str) -> TestResult {
    let skill = std::fs::read_to_string(root.join(path))?;
    assert_in_order(&skill, PRIMARY_SKILL_ORDER)?;
    assert_unresolved_threads_are_required(&skill, path);
    if path == ".codex/skills/create_pull_request/SKILL.md"
        || path == ".agents/skills/create_pull_request/SKILL.md"
    {
        assert_closing_issue_contract(&skill, path)?;
    }
    if path == ".agents/skills/create_pull_request/SKILL.md" {
        assert_agents_pr_skill_contract(&skill);
    }
    Ok(())
}

fn assert_agents_pr_skill_contract(skill: &str) {
    assert!(skill.contains("gh pr create --draft"));
    assert!(skill.contains("phase=initial head=$head_sha body-sha256=$body_sha256"));
    assert!(skill.contains("phase=final head=$head_sha body-sha256=$body_sha256"));
    assert!(skill.contains("thread への reply→resolve") || skill.contains("thread への reply"));
    assert!(skill.contains("just pr-ready-check \"<pr-number>\" &&"));
    assert!(skill.contains("gh pr ready"));
    assert!(!skill.contains("gh pr create --base \"<base-branch>\""));
}

const REVIEW_EVIDENCE_SKILL_PATHS: &[&str] = &[
    ".agents/skills/commit_and_push/SKILL.md",
    ".agents/skills/create_pull_request/SKILL.md",
    ".agents/skills/impl-release/SKILL.md",
    ".codex/skills/commit_and_push/SKILL.md",
    ".codex/skills/create_pull_request/SKILL.md",
    ".codex/skills/gh-address-comments/SKILL.md",
    ".codex/skills/impl-release/SKILL.md",
    ".codex/skills/kdr-workflow-guide/SKILL.md",
    ".codex/skills/self-review/SKILL.md",
];

const MIRRORED_REVIEW_SKILLS: &[(&str, &str)] = &[
    (
        ".agents/skills/commit_and_push/SKILL.md",
        ".codex/skills/commit_and_push/SKILL.md",
    ),
    (
        ".agents/skills/create_pull_request/SKILL.md",
        ".codex/skills/create_pull_request/SKILL.md",
    ),
    (
        ".agents/skills/impl-release/SKILL.md",
        ".codex/skills/impl-release/SKILL.md",
    ),
];

const STRICT_REVIEW_MARKER_TERMS: &[RequiredTerm] = &[
    RequiredTerm::Alternatives(&[
        "krr-review phase=<initial|final>",
        "krr-review phase=(?:initial|final)",
        "krr-review phase=initial",
    ]),
    RequiredTerm::Exact("head="),
    RequiredTerm::Exact("body-sha256="),
    RequiredTerm::Alternatives(&["strict", "完全一致"]),
];

const CURRENT_TRUSTED_BODY_EVIDENCE_TERMS: &[RequiredTerm] = &[
    RequiredTerm::Exact("pr_body_sha256"),
    RequiredTerm::Alternatives(&[
        "current PR body",
        "current PR本文",
        "現在のPR本文",
        "本文digest",
        "本文 digest",
    ]),
    RequiredTerm::Alternatives(&["exactly one", "ちょうど1個"]),
    RequiredTerm::Exact("fail-closed"),
];

const NO_ISSUES_EVIDENCE_TERMS: &[RequiredTerm] = &[
    RequiredTerm::Alternatives(&[
        "trusted bot の Issue comment",
        "trusted review bot の Issue comment",
        "trusted botがPR Issue",
    ]),
    RequiredTerm::Exact("created_at == updated_at"),
    RequiredTerm::Alternatives(&[
        "phase windowごとの候補一意性",
        "phase window の候補を一意",
        "各 phase window の候補は一意",
        "各 phase windowの候補一意",
        "各phase windowの候補一意",
        "各 phase windowで一意",
        "各 phase window で一意",
        "各phase windowで一意",
        "候補一意性",
        "高々1件",
    ]),
    RequiredTerm::Alternatives(&["重複", "duplicate"]),
    RequiredTerm::Exact("fail-closed"),
    RequiredTerm::Alternatives(&["Issue freshness", "Issue更新後"]),
    RequiredTerm::Alternatives(&["未resolve", "未 resolve", "未解決", "unresolved"]),
    RequiredTerm::Alternatives(&[
        "同一current HEAD・body digest",
        "同一current HEAD・同一PR body digest",
        "current HEAD/body digest が同一",
        "current HEAD/body digest同一",
        "同一 current HEAD・同一 PR body digest",
        "current HEAD/body digest が両 marker で同一",
        "current HEAD/body digest 同一",
        "同じHEAD・本文digest",
        "同じ HEAD・本文digest",
        "同じHEAD/body digest",
        "最新HEAD・本文digest",
    ]),
    RequiredTerm::Alternatives(&["再利用", "reuse"]),
    RequiredTerm::Alternatives(&["formal review/指摘経路", "formal review", "指摘経路"]),
    RequiredTerm::Alternatives(&[
        "final marker後の別",
        "final marker 後の別",
        "final後の別",
        "final marker後に別",
        "final marker 後に別",
    ]),
    RequiredTerm::Alternatives(&["8192", "8192 bytes"]),
    RequiredTerm::Alternatives(&["nested details", "nested/sentinel", "nested"]),
    RequiredTerm::Alternatives(&["sentinel", "closing/sentinel"]),
    RequiredTerm::Alternatives(&["reaction", "リアクション"]),
];

fn assert_review_evidence_contract(root: &Path) -> TestResult {
    let mut missing = Vec::new();
    collect_skill_evidence(root, &mut missing)?;
    collect_mirror_evidence(root, &mut missing)?;
    collect_document_evidence(root, &mut missing)?;

    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "review-evidence contract is incomplete:\n{}",
            missing.join("\n")
        )
        .into())
    }
}

fn collect_skill_evidence(root: &Path, missing: &mut Vec<String>) -> TestResult {
    for path in REVIEW_EVIDENCE_SKILL_PATHS {
        let skill = std::fs::read_to_string(root.join(path))?;
        collect_missing_review_evidence_terms(&skill, path, missing);
        collect_missing_terms(&skill, path, NO_ISSUES_EVIDENCE_TERMS, missing);
    }
    Ok(())
}

fn collect_mirror_evidence(root: &Path, missing: &mut Vec<String>) -> TestResult {
    for (agents_path, codex_path) in MIRRORED_REVIEW_SKILLS {
        let agents_skill = std::fs::read_to_string(root.join(agents_path))?;
        let codex_skill = std::fs::read_to_string(root.join(codex_path))?;
        if review_evidence_signature(&agents_skill) != review_evidence_signature(&codex_skill) {
            missing.push(format!(
                "{agents_path} and {codex_path} must mirror the strict marker and trusted body-evidence contract"
            ));
        }
    }
    Ok(())
}

fn collect_document_evidence(root: &Path, missing: &mut Vec<String>) -> TestResult {
    for path in ["AGENTS.md", "docs/issue-driven-workflow.md"] {
        let document = std::fs::read_to_string(root.join(path))?;
        collect_missing_review_evidence_terms(&document, path, missing);
        collect_missing_terms(document.as_str(), path, NO_ISSUES_EVIDENCE_TERMS, missing);
    }
    Ok(())
}

fn collect_missing_terms(
    text: &str,
    path: &str,
    required_terms: &[RequiredTerm],
    missing: &mut Vec<String>,
) {
    for required in required_terms {
        if !required.is_present(text) {
            missing.push(format!(
                "{path} must contain no-issues evidence contract term {required:?}"
            ));
        }
    }
}

fn review_evidence_signature(text: &str) -> Vec<bool> {
    STRICT_REVIEW_MARKER_TERMS
        .iter()
        .chain(CURRENT_TRUSTED_BODY_EVIDENCE_TERMS)
        .chain(NO_ISSUES_EVIDENCE_TERMS)
        .map(|required| required.is_present(text))
        .collect()
}

fn collect_missing_review_evidence_terms(text: &str, path: &str, missing: &mut Vec<String>) {
    for required in STRICT_REVIEW_MARKER_TERMS
        .iter()
        .chain(CURRENT_TRUSTED_BODY_EVIDENCE_TERMS)
    {
        if !required.is_present(text) {
            missing.push(format!(
                "{path} must contain review-evidence contract term {required:?}"
            ));
        }
    }
}

fn assert_closing_issue_contract(skill: &str, path: &str) -> TestResult {
    assert_closing_issue_order(skill, path)?;
    assert_closing_issue_examples(skill, path);
    assert_closing_issue_capacity(skill);
    Ok(())
}

fn assert_closing_issue_order(skill: &str, path: &str) -> TestResult {
    let closing = skill
        .find("GitHub closing keyword")
        .ok_or_else(|| format!("{path} must require GitHub closing keywords"))?;
    let draft = skill
        .find("gh pr create --draft")
        .ok_or_else(|| format!("{path} must create Draft PRs"))?;
    let ready = skill
        .find("gh pr ready")
        .ok_or_else(|| format!("{path} must describe Ready promotion"))?;
    assert!(
        closing < draft,
        "{path} must collect closing references before Draft creation"
    );
    assert!(
        closing < ready,
        "{path} must retain the closing contract before Ready"
    );
    Ok(())
}

fn assert_closing_issue_examples(skill: &str, path: &str) {
    for required in [
        "Closes #N",
        "Fixes #N",
        "Resolves #N",
        "Refs #N",
        "完全一致",
        "不足も余分も許可しません",
    ] {
        assert!(skill.contains(required), "{path} must document {required}");
    }
}

fn assert_closing_issue_capacity(skill: &str) {
    assert!(skill.contains("256 non-Draft target invariant"));
    assert!(skill.contains("bypass"));
    assert!(skill.contains("Draft に戻す") || skill.contains("Draftへ戻す"));
    assert!(
        skill.contains("closing reference を外す")
            || skill.contains("closing referenceを外す")
            || skill.contains("closing reference を外して")
            || skill.contains("closing referenceを外して")
    );
}

fn assert_unresolved_threads_are_required(skill: &str, path: &str) {
    assert!(
        ["未resolve", "未 resolve", "未解決"]
            .iter()
            .any(|term| skill.contains(term)),
        "{path} must require resolved review threads"
    );
}

fn assert_governed_skill_contracts(root: &Path) -> TestResult {
    for contract in SKILL_CONTRACTS {
        let skill = std::fs::read_to_string(root.join(contract.path))?;
        assert_hyphen_case_frontmatter(&skill, contract.path)?;
        assert_required_terms(&skill, contract)?;
        assert_ready_promotion_boundary(&skill, contract)?;
    }
    Ok(())
}

fn assert_hyphen_case_frontmatter(skill: &str, path: &str) -> TestResult {
    let name = frontmatter_name(skill).ok_or("skill frontmatter name is missing")?;
    assert!(
        name.chars()
            .all(|character| character.is_ascii_lowercase() || character == '-'),
        "skill frontmatter name must be hyphen-case: {path}"
    );
    Ok(())
}

fn assert_required_terms(skill: &str, contract: &SkillContract) -> TestResult {
    for required in contract.required_terms {
        assert!(
            required.is_present(skill),
            "{} must contain {required:?}",
            contract.path
        );
    }
    Ok(())
}

fn assert_ready_promotion_boundary(skill: &str, contract: &SkillContract) -> TestResult {
    if contract.promotes_ready {
        assert!(
            skill.contains("gh pr ready"),
            "{} must own Ready promotion",
            contract.path
        );
        if contract.path == ".codex/skills/commit_and_push/SKILL.md" {
            assert_commit_and_push_ready_boundary(skill)?;
        } else {
            assert_in_order(skill, READY_PROMOTION_ORDER)
                .map_err(|error| format!("{}: {error}", contract.path))?;
        }
    } else {
        assert!(
            !skill.contains("gh pr ready"),
            "{} must not promote Ready",
            contract.path
        );
        assert!(
            skill.contains("Ready"),
            "{} must describe the Ready boundary",
            contract.path
        );
        assert_in_order(skill, contract.required_terms)?;
    }
    Ok(())
}

fn assert_commit_and_push_ready_boundary(skill: &str) -> TestResult {
    assert_eq!(
        skill.matches("gh pr ready <number>").count(),
        1,
        "commit-and-push must not describe a pre-gate Ready transition"
    );
    assert_in_order(
        skill,
        &[
            RequiredTerm::Exact("final"),
            RequiredTerm::Exact("main が機械ゲートを実行"),
            RequiredTerm::Exact("成功したらmainは `gh pr ready <number>`"),
            RequiredTerm::Exact("freshなmerge承認"),
            RequiredTerm::Exact("merge直前"),
            RequiredTerm::Exact("just pr-ready-check"),
            RequiredTerm::Exact("merge --apply"),
        ],
    )?;
    assert!(
        skill.contains("通常のGitHub CLI merge（`gh pr merge`）・人間/admin bypassは禁止"),
        "commit-and-push must continue rejecting direct merge and bypass paths"
    );
    Ok(())
}

fn assert_pr_ready_recipe_contract(root: &Path) -> TestResult {
    let justfile = std::fs::read_to_string(root.join("Justfile"))?;
    let ready_check = recipe_body(&justfile, "pr-ready-check")?;
    let automation = recipe_body(&justfile, "automation-contract-test")?;
    assert_pr_ready_recipe_structure(&justfile, ready_check, automation)?;
    assert_pr_ready_recipe_invocations(root)?;
    assert_pr_ready_recipe_documentation(root)?;
    assert_governance_bootstrap_documentation(root)?;
    assert_no_legacy_pr_ready_invocation(root)
}

fn assert_pr_ready_recipe_structure(
    justfile: &str,
    ready_check: &str,
    automation: &str,
) -> TestResult {
    assert!(justfile.contains("pr-ready-check pr:"));
    assert_pr_ready_recipe_metadata(ready_check);
    assert_pr_ready_recipe_issue_dispatch(ready_check);
    assert_pr_ready_recipe_readiness_dispatch(ready_check);
    assert_pr_ready_recipe_order(ready_check);
    assert!(automation.contains("unittest discover -s scripts/review"));
    Ok(())
}

fn assert_pr_ready_recipe_metadata(ready_check: &str) {
    for required in [
        "set -euo pipefail",
        "pr={{quote(pr)}}",
        "gh pr view \"$pr\" --json baseRefOid,headRefOid,headRefName,baseRefName,isDraft",
        "gh repo view --json nameWithOwner",
        "gh returned incomplete repository metadata",
        "gh returned incomplete or unsafe PR/default metadata",
        "gh returned incomplete PR metadata",
        "IFS=\"$(printf '\\011')\" read -r repository_owner repository_name repository repository_extra",
        "IFS=\"$(printf '\\011')\" read -r base_sha head_sha branch base_branch parsed_repository trusted_default_sha readiness_mode extra",
        "not any(ord(character) < 32 or ord(character) == 127",
        "fields[3] == default[\"name\"]",
        "isinstance(fields[6], bool)",
        "\"require-draft\" if fields[6] else \"allow-ready\"",
        "\"$readiness_mode\" != \"require-draft\" && \"$readiness_mode\" != \"allow-ready\"",
    ] {
        assert!(
            ready_check.contains(required),
            "recipe must contain {required}"
        );
    }
    assert!(!ready_check.contains("mapfile"));
}

fn assert_pr_ready_recipe_issue_dispatch(ready_check: &str) {
    for required in [
        "scripts/hooks/verify_push_issue.py",
        "--pr-number \"$pr\"",
        "--pr-base-sha \"$base_sha\"",
        "--pr-head-sha \"$head_sha\"",
        "--pr-branch \"$branch\"",
        "--repository \"$repository\"",
    ] {
        assert!(
            ready_check.contains(required),
            "Issue contract must contain {required}"
        );
    }
}

fn assert_pr_ready_recipe_readiness_dispatch(ready_check: &str) {
    assert!(ready_check.contains("scripts/review/verify_pr_ready.py"));
    assert!(ready_check.contains("\"--$readiness_mode\""));
    assert!(ready_check.contains("\"require-draft\""));
    assert!(ready_check.contains("\"allow-ready\""));
}

fn assert_pr_ready_recipe_order(ready_check: &str) {
    assert!(
        ready_check.find("scripts/hooks/verify_push_issue.py")
            < ready_check.find("scripts/review/verify_pr_ready.py"),
        "the Issue contract must run before PR-readiness verification"
    );
}

fn assert_pr_ready_recipe_invocations(root: &Path) -> TestResult {
    assert_canonical_pr_ready_invocation(root)?;
    assert_injection_is_rejected(root)?;
    assert_legacy_pr_ready_invocation_is_rejected(root)?;
    Ok(())
}

fn assert_canonical_pr_ready_invocation(root: &Path) -> TestResult {
    let output = run_just(root, &["--dry-run", "pr-ready-check", "72"])?;
    assert!(
        output.status.success(),
        "canonical positional invocation must succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = command_output(output)?;
    assert!(text.contains("scripts/hooks/verify_push_issue.py"));
    assert!(text.contains("scripts/review/verify_pr_ready.py"));
    Ok(())
}

fn assert_injection_is_rejected(root: &Path) -> TestResult {
    let output = run_just(root, &["pr-ready-check", "1\"; exit 0; #"])?;
    assert!(!output.status.success());
    let text = command_output(output)?;
    assert!(
        text.contains("pr-ready-check requires a positive numeric pull request number"),
        "the numeric guard must stop before any GitHub probe: {text}"
    );
    Ok(())
}

fn assert_legacy_pr_ready_invocation_is_rejected(root: &Path) -> TestResult {
    let output = run_just(root, &["--dry-run", "PR=72", "pr-ready-check"])?;
    assert!(
        !output.status.success(),
        "legacy variable assignment must be rejected in favor of `just pr-ready-check <number>`"
    );
    Ok(())
}

fn run_just(root: &Path, args: &[&str]) -> std::io::Result<std::process::Output> {
    Command::new("just").args(args).current_dir(root).output()
}

fn command_output(output: std::process::Output) -> Result<String, Box<dyn std::error::Error>> {
    Ok(format!(
        "{}{}",
        String::from_utf8(output.stdout)?,
        String::from_utf8(output.stderr)?
    ))
}

fn assert_pr_ready_recipe_documentation(root: &Path) -> TestResult {
    let agents = std::fs::read_to_string(root.join("AGENTS.md"))?;
    assert!(agents.contains("just pr-ready-check <number>"));
    for required in ["参照IssueがOPEN", "依存更新証跡", "PR rangeのIssue契約"] {
        assert!(
            agents.contains(required),
            "AGENTS.md must document {required}"
        );
    }
    Ok(())
}

fn assert_governance_bootstrap_documentation(root: &Path) -> TestResult {
    let agents = std::fs::read_to_string(root.join("AGENTS.md"))?;
    let workflow = std::fs::read_to_string(root.join("docs/issue-driven-workflow.md"))?;
    assert!(
        !agents.contains("KRR_GOVERNANCE_APP_TOKEN"),
        "AGENTS.md must not require an externally supplied installation token"
    );
    assert_bootstrap_overview_terms(&agents, &workflow);
    assert_bootstrap_protection_is_pre_merge(&workflow);
    assert_bootstrap_cli_boundary(&workflow);
    Ok(())
}

fn assert_bootstrap_overview_terms(agents: &str, workflow: &str) {
    for document in [agents, workflow] {
        for required in [
            "KRR / PR governance bootstrap",
            "PR外の専用GitHub App",
            "固定HEAD",
            "PR内の例外",
            "自己承認",
            "verify_push_issue.py",
            "KRR / PR governance (trusted check)",
            "KRR / PR governance review latch",
            "app_id=15368",
            "使い捨てPR",
            "latest initial marker",
            "current PR",
        ] {
            assert!(
                document.contains(required),
                "bootstrap contract must document {required}"
            );
        }
    }
}

fn assert_bootstrap_protection_is_pre_merge(workflow: &str) {
    assert!(
        !workflow.contains("bootstrap PRのmerge後に設定する"),
        "bootstrap protection must be active before merge"
    );
}

const BOOTSTRAP_CLI_BOUNDARY_TERMS: &[&str] = &[
    "通常gateの代替ではない",
    "PR内のworkflow/branch/Issueを条件にした自己例外",
    "/Users/hiroyuki_furuno/.codex/skills/krr-pr-governance-bootstrap/scripts/bootstrap_pr_governance.py",
    "--expected-base",
    "--expected-diff-sha256",
    "--allowed-workflow",
    "activate",
    "finalize",
    "verify",
    "--apply",
    "--smoke-pr",
    "KRR_GOVERNANCE_APP_JWT",
    "prepare",
    "prepare --apply",
    "merge",
    "merge --apply",
    "freshな",
    "expires_at",
    "scope",
    "permissions",
    "least-privilege",
    "exact Integration App",
    "bypass_mode=pull_request",
    "Appによる直接ref更新",
    "classic branch protection",
    "enforce_admins=true",
    "required_conversation_resolution=true",
    "strict=true",
    "人間/admin/UI/通常のGitHub CLI",
    "JWT/private key/Installation token",
    "引数・出力へ出してはならず",
    "PR checkoutのコードをbootstrap evidenceとして実行せず",
];

fn assert_bootstrap_cli_boundary(workflow: &str) {
    for required in BOOTSTRAP_CLI_BOUNDARY_TERMS {
        assert!(
            workflow.contains(required),
            "bootstrap boundary must document {required}"
        );
    }
    assert!(
        !workflow.contains("KRR_GOVERNANCE_APP_TOKEN"),
        "bootstrap boundary must not use an externally supplied installation token"
    );
}

fn assert_no_legacy_pr_ready_invocation(root: &Path) -> TestResult {
    let legacy_syntax = "just ".to_owned() + "PR=";
    let legacy_references = Command::new("git")
        .args(["grep", "-n", "--fixed-strings", &legacy_syntax])
        .current_dir(root)
        .output()?;
    assert_eq!(
        legacy_references.status.code(),
        Some(1),
        "legacy invocation remains in tracked files: {}",
        String::from_utf8_lossy(&legacy_references.stdout)
    );
    Ok(())
}

fn assert_governance_workflow_contract(root: &Path) -> TestResult {
    let governance = std::fs::read_to_string(root.join(".github/workflows/pr-governance.yml"))?;
    let writer =
        std::fs::read_to_string(root.join(".github/workflows/pr-governance-status-writer.yml"))?;
    let writer_program =
        std::fs::read_to_string(root.join("scripts/review/pr_governance_status_writer.py"))?;
    let review_sensor =
        std::fs::read_to_string(root.join(".github/workflows/pr-governance-review-events.yml"))?;
    assert_governance_trigger_contract(&governance, &writer_program);
    assert_governance_sensor_contract(&review_sensor);
    assert_governance_source_contract(&governance, &writer, &writer_program);
    assert_governance_status_contract(&governance, &writer_program);
    assert_governance_access_contract(&governance, &writer);
    Ok(())
}

fn assert_governance_trigger_contract(governance: &str, writer_program: &str) {
    assert!(writer_program.contains("scripts/review/verify_pr_ready.py"));
    assert!(writer_program.contains("--allow-ready"));
    assert!(governance.contains("issue_comment:"));
    assert!(governance.contains("workflow_run:"));
    assert!(governance.contains("PR governance review sensor"));
    assert!(governance.contains("pull_request_target:"));
    assert!(
        !governance
            .lines()
            .any(|line| line.trim() == "statuses: write")
    );
}

fn assert_governance_sensor_contract(sensor: &str) {
    for trigger in [
        "pull_request:",
        "pull_request_review:",
        "pull_request_review_comment:",
        "converted_to_draft",
    ] {
        assert!(
            sensor.contains(trigger),
            "review sensor must contain {trigger}"
        );
    }
    assert!(!sensor.contains("pull_request_target:"));
    assert!(sensor.contains("KRR / PR governance review latch"));
    assert!(sensor.contains("actions: read"));
    assert!(sensor.contains("checks: read"));
}

fn assert_governance_source_contract(governance: &str, writer: &str, writer_program: &str) {
    for required in [
        "Resolve current open pull requests from the trusted default branch",
        "Validate trusted default-branch writer",
        "Rebind trusted default branch before token creation",
        "ref: ${{ github.sha }}",
        "persist-credentials: false",
        "trusted_dispatcher_source",
        "rebind_trusted_default_writer",
    ] {
        assert!(
            governance.contains(required)
                || writer.contains(required)
                || writer_program.contains(required),
            "trusted source contract must contain {required}"
        );
    }
    assert!(!writer.contains("ref: refs/pull/"));
    assert!(!writer.contains("github.event.pull_request.merge_commit_sha"));
}

fn assert_governance_status_contract(governance: &str, writer_program: &str) {
    for required in [
        "repos/{REPOSITORY}/check-runs",
        "CHECK_NAME = \"KRR / PR governance (trusted check)\"",
        "CHECK_EXTERNAL_PREFIX",
        "in_progress",
        "success",
        "failure",
        "check_fingerprint",
    ] {
        assert!(
            governance.contains(required) || writer_program.contains(required),
            "trusted governance Check Run contract must contain {required}"
        );
    }
    assert!(governance.contains("environment: pr-governance"));
    assert!(!governance.contains("/statuses/"));
}

fn assert_governance_access_contract(governance: &str, writer: &str) {
    assert!(
        governance
            .contains("actions/create-github-app-token@fee1f7d63c2ff003460e3d139729b119787bc349")
            || writer.contains(
                "actions/create-github-app-token@fee1f7d63c2ff003460e3d139729b119787bc349"
            )
    );
    for required in [
        "KRR_GOVERNANCE_APP_ID",
        "KRR_GOVERNANCE_APP_PRIVATE_KEY",
        "outputs.token",
        "GH_TOKEN: ${{ steps.",
        "permission-checks: write",
        "CHECK_WRITE_TOKEN",
    ] {
        assert!(
            governance.contains(required) || writer.contains(required),
            "trusted governance must contain {required}"
        );
    }
    assert!(!governance.contains("statuses: write"));
    assert!(!writer.contains("statuses: write"));
}

fn frontmatter_name(skill: &str) -> Option<&str> {
    let frontmatter = skill.strip_prefix("---\n")?.split_once("\n---")?.0;
    frontmatter
        .lines()
        .find_map(|line| line.strip_prefix("name:").map(str::trim))
}

fn assert_in_order(text: &str, terms: &[RequiredTerm]) -> TestResult {
    let mut previous = 0;
    for required in terms {
        let (position, length) = required
            .first_position_at_or_after(text, previous)
            .ok_or_else(|| format!("required term is missing: {required:?}"))?;
        previous = position + length;
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
