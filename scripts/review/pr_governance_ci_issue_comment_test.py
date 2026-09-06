from __future__ import annotations

import re
import runpy
import shlex
import unittest
from pathlib import Path


class GovernanceCiAndIssueContractTest(unittest.TestCase):
    def test_documented_commit_recipes_satisfy_the_real_issue_reference_parser(self) -> None:
        root = Path(__file__).parents[2]
        parser = runpy.run_path(str(root / "scripts/hooks/verify_push_issue.py"))["issue_numbers"]
        for relative in (
            ".codex/skills/commit_and_push/SKILL.md",
            ".agents/skills/commit_and_push/SKILL.md",
            ".codex/skills/impl-release/SKILL.md",
            ".agents/skills/impl-release/SKILL.md",
        ):
            text = (root / relative).read_text(encoding="utf-8")
            recipes = re.findall(r"(?m)^git commit -m .+$", text)
            self.assertTrue(recipes, relative)
            for recipe in recipes:
                with self.subTest(path=relative, recipe=recipe):
                    arguments = shlex.split(recipe.replace("${issue_number}", "64"))
                    message = arguments[arguments.index("-m") + 1]
                    self.assertEqual(parser(message, "HiroyukiFuruno/katana-render-runtime"), {64})

    def setUp(self) -> None:
        root = Path(__file__).parents[2]
        self.dispatcher = (root / ".github/workflows/pr-governance.yml").read_text(encoding="utf-8")
        self.writer = (root / "scripts/review/pr_governance_status_writer.py").read_text(encoding="utf-8")
        self.writer_workflow = (root / ".github/workflows/pr-governance-status-writer.yml").read_text(encoding="utf-8")

    def test_ci_and_release_generations_bind_path_repo_pr_base_head_and_attempt(self) -> None:
        for name, path in (
            ("CI", ".github/workflows/test-and-build.yml"),
            ("release-preflight", ".github/workflows/release-preflight.yml"),
        ):
            self.assertIn(f'generation(number, base, head, "{name}", "{path}", evidence)', self.writer)
            self.assertIn(f'run.get("name") == name and workflow_path_matches(run.get("path"), path)', self.writer)
        for text in (
            'run.get("event") == "pull_request"', 'item.get("number") == number',
            'run_base.get("sha") == base', 'run_head.get("sha") == head',
            'run.get("workflow_id") == workflow_id', 'type(run.get("run_attempt")) is int',
            'Default-branch CI workflow ID is invalid.', 'if evidence is None:\n        trusted_workflow_blob(path, base, head)', 'return max(matches, key=lambda item:',
        ):
            self.assertIn(text, self.writer)

    def test_governed_pull_request_workflows_have_no_path_filter(self) -> None:
        """Every governed PR path must start CI and release-preflight generation."""
        root = Path(__file__).parents[2]
        for relative in (".github/workflows/test-and-build.yml", ".github/workflows/release-preflight.yml"):
            text = (root / relative).read_text(encoding="utf-8")
            pull_request = re.search(r"(?ms)^  pull_request:\n(?P<body>.*?)(?=^  [A-Za-z0-9_-]+:|\Z)", text)
            self.assertIsNotNone(pull_request, relative)
            body = pull_request.group("body")
            self.assertNotRegex(body, r"(?m)^    paths(?:-ignore)?:")

        ci_text = (root / ".github/workflows/test-and-build.yml").read_text(encoding="utf-8")
        push = re.search(r"(?ms)^  push:\n(?P<body>.*?)(?=^  pull_request:)", ci_text)
        self.assertIsNotNone(push)
        self.assertRegex(push.group("body"), r"(?m)^    paths:")

    def test_workflow_run_filter_is_job_level_and_allows_only_trusted_pr_events(self) -> None:
        """Classify the source before the shared barrier can be mutated."""
        job_bodies = {}
        for job in ("preflight-workflow-run-source", "establish-resolver-failure-barrier"):
            match = re.search(
                rf"(?ms)^  {re.escape(job)}:\n"
                rf"(?P<body>.*?)(?=^  [A-Za-z0-9_-]+:|\Z)",
                self.dispatcher,
            )
            self.assertIsNotNone(match, job)
            assert match is not None
            job_bodies[job] = match.group("body")

        allowlist = (
            "github.run_attempt == 1 && (github.event_name != 'workflow_run' || ("
            "(github.event.workflow_run.name == 'PR governance review sensor' && "
            "(github.event.workflow_run.event == 'pull_request' || "
            "github.event.workflow_run.event == 'pull_request_review' || "
            "github.event.workflow_run.event == 'pull_request_review_comment')) || "
            "((github.event.workflow_run.name == 'CI' || "
            "github.event.workflow_run.name == 'release-preflight') && "
            "github.event.workflow_run.event == 'pull_request')))"
        )
        expected = "${{ " + allowlist + " }}"
        preflight = job_bodies["preflight-workflow-run-source"]
        barrier = job_bodies["establish-resolver-failure-barrier"]
        for body, label in ((preflight, "preflight"), (barrier, "barrier")):
            if_match = re.search(r"(?m)^    if:\s*(?P<expression>.+)$", body)
            self.assertIsNotNone(if_match, label)
            steps_position = body.find("\n    steps:")
            self.assertGreater(steps_position, -1, label)
            assert if_match is not None
            self.assertLess(if_match.start(), steps_position, label)
            expression = if_match.group("expression").strip()
            self.assertEqual(
                expression,
                expected
                if label == "preflight"
                else "${{ needs.preflight-workflow-run-source.outputs.reconcile == 'true' && "
                + allowlist
                + " }}",
            )

        self.assertIn("outputs:\n      reconcile: ${{ steps.scope.outputs.reconcile }}", preflight)
        self.assertIn("valid: ${{ steps.scope.outputs.valid }}", preflight)
        self.assertIn("id: scope", preflight)
        self.assertIn('output.write("reconcile="', preflight)
        self.assertIn('output.write("valid="', preflight)
        self.assertIn("EVENT_SOURCE_VALID: ${{ needs.preflight-workflow-run-source.outputs.valid }}", barrier)
        self.assertIn('if os.environ.get("EVENT_SOURCE_VALID") != "true":', barrier)
        self.assertIn("barrier remains active", barrier)

        allowed_events = {
            "PR governance review sensor": {
                "pull_request",
                "pull_request_review",
                "pull_request_review_comment",
            },
            "CI": {"pull_request"},
            "release-preflight": {"pull_request"},
        }
        fixtures = (
            (1, "PR governance review sensor", "pull_request", True),
            (1, "PR governance review sensor", "pull_request_review", True),
            (1, "PR governance review sensor", "pull_request_review_comment", True),
            (1, "CI", "pull_request", True),
            (1, "release-preflight", "pull_request", True),
            (1, "CI", "push", False),
            (1, "release-preflight", "workflow_dispatch", False),
            # Reject name/event cross-products: the sensor-only events are
            # not CI/preflight inputs, and CI is not a review-sensor input.
            (1, "CI", "pull_request_review", False),
            (1, "release-preflight", "pull_request_review_comment", False),
            (1, "PR governance review sensor", "workflow_dispatch", False),
            (2, "PR governance review sensor", "pull_request", False),
            (2, "CI", "pull_request", False),
            (2, "release-preflight", "pull_request", False),
        )
        for attempt, name, event, expected in fixtures:
            with self.subTest(attempt=attempt, name=name, event=event):
                self.assertEqual(
                    attempt == 1 and event in allowed_events.get(name, set()),
                    expected,
                )

    def test_success_re_reads_ci_generation_from_one_final_shared_snapshot_before_post(self) -> None:
        self.assertIn("final_evidence_for_pr(decision.head, initial_evidence)", self.writer)
        # The final shared snapshot must build the query through urlencode so
        # the bounded endpoint remains correct even when a future parameter
        # contains characters requiring escaping.
        self.assertIn(
            '"repos/{REPOSITORY}/actions/runs?" + urlencode({"head_sha": head, "per_page": 100})',
            self.writer,
        )
        self.assertIn("def finalize_decision", self.writer)
        self.assertIn("latest != generations", self.writer)
        self.assertIn("CI generation changed during governance revalidation.", self.writer)
        self.assertIn("check_changed_since(decision.head, decision.pending_check_fingerprint)", self.writer)

    def test_default_branch_writer_uses_protected_environment_and_split_tokens(self) -> None:
        self.assertIn("environment: pr-governance", self.writer_workflow)
        self.assertIn("Writer SHA does not match default branch head.", self.writer_workflow)
        self.assertIn("Writer workflow differs from default branch.", self.writer_workflow)
        self.assertIn("ref: ${{ github.sha }}", self.writer_workflow)
        self.assertIn("bootstrap-validation bound this immutable dispatch SHA", self.writer_workflow)
        self.assertIn("persist-credentials: false", self.writer_workflow)
        self.assertIn("permission-checks: write", self.writer_workflow)
        read_token_start = self.writer_workflow.index(
            "      - name: Create read-only governance App token"
        )
        read_token_end = self.writer_workflow.index(
            "      - name:", read_token_start + 1
        )
        read_token_step = self.writer_workflow[read_token_start:read_token_end]
        self.assertIn("id: read-token", read_token_step)
        self.assertIn("permission-administration: read", read_token_step)
        self.assertNotIn("permission-checks: write", read_token_step)
        self.assertEqual(
            self.writer_workflow.count("permission-administration: read"), 1
        )
        self.assertIn("def read_environment(*, default_token: bool = False)", self.writer)
        self.assertIn('return {"GH_TOKEN": token, "PATH": os.environ["PATH"]}', self.writer)
        self.assertIn("environment = {\"GH_TOKEN\": token, \"PATH\": environment[\"PATH\"]}", self.writer)
        self.assertIn("DEFAULT_READ_TOKEN: ${{ github.token }}", self.writer_workflow)
        self.assertIn("Create read-only governance App token", self.writer_workflow)

    def test_issue_comment_and_issue_events_are_bounded_to_one_default_branch_arbiter(self) -> None:
        self.assertIn("issue_comment:", self.dispatcher)
        self.assertIn("issues:", self.dispatcher)
        self.assertIn("workflow_dispatch:", self.writer_workflow)
        # The dispatcher passes only its immutable run ID; it never passes a
        # caller-controlled count, PR ref, SHA, or target set to the writer.
        self.assertNotIn("invalidated_count", self.writer_workflow)
        self.assertNotIn("invalidated_count", self.dispatcher)
        self.assertIn("dispatcher_run_id:", self.writer_workflow)
        self.assertNotRegex(self.writer_workflow, r"inputs\.pr(?:\s|}}|\])")

    def test_ready_and_merge_harness_rechecks_the_same_gate_immediately_before_merge(self) -> None:
        root = Path(__file__).parents[2]
        for relative in (
            "AGENTS.md",
            "docs/issue-driven-workflow.md",
            ".codex/skills/impl-release/SKILL.md",
            ".agents/skills/impl-release/SKILL.md",
            ".codex/skills/create_pull_request/SKILL.md",
            ".agents/skills/create_pull_request/SKILL.md",
            ".codex/skills/kdr-workflow-guide/SKILL.md",
            ".agents/skills/kdr-workflow-guide/SKILL.md",
            ".codex/skills/gh-address-comments/SKILL.md",
            ".codex/skills/commit_and_push/SKILL.md",
            ".agents/skills/commit_and_push/SKILL.md",
        ):
            with self.subTest(path=relative):
                text = (root / relative).read_text(encoding="utf-8")
                self.assertIn("merge --apply", text)
                self.assertIn("just pr-ready-check", text)
                self.assertIn("直前", text)
                self.assertRegex(
                    text,
                    r"(?:gh pr merge|GitHub CLI merge|通常のGitHub CLI merge).*(?:禁止|使わない|bypass)",
                )
                self.assertRegex(text, r"(?:UI|ブラウザ).*(?:禁止|bypass)")
                self.assertRegex(text, r"(?:人間|human).*(?:禁止|bypass)")
                self.assertRegex(text, r"admin.*(?:禁止|bypass)")
                if relative.endswith("commit_and_push/SKILL.md"):
                    self.assertLess(
                        text.index("`gh pr ready <number>`"),
                        text.index("freshなmerge承認"),
                    )

    def test_review_skills_restart_initial_review_after_every_push_or_body_edit(self) -> None:
        root = Path(__file__).parents[2]
        for relative in (
            ".codex/skills/self-review/SKILL.md",
            ".codex/skills/gh-address-comments/SKILL.md",
        ):
            with self.subTest(path=relative):
                text = (root / relative).read_text(encoding="utf-8")
                self.assertRegex(
                    text,
                    r"(?:修正の push|修正.*push).*?(?:無効|失効).*?(?:strict )?`?initial`?.*(?:完了|cloud review)",
                )
                self.assertRegex(
                    text,
                    r"(?:同一 HEAD|同じHEAD).*?(?:本文編集|PR本文).*?(?:initial|初回)",
                )
                self.assertRegex(
                    text,
                    r"(?:initial review|initial marker).*?(?:完了|同一HEAD/body).*?(?:final review|final marker)",
                )
                self.assertNotIn("push 後、最終 cloud review", text)

    def test_bootstrap_mirrors_require_a_fresh_app_jwt_for_verify(self) -> None:
        root = Path(__file__).parents[2]
        for relative in (
            "AGENTS.md",
            "docs/issue-driven-workflow.md",
            ".codex/skills/impl-release/SKILL.md",
            ".agents/skills/impl-release/SKILL.md",
            ".codex/skills/create_pull_request/SKILL.md",
            ".agents/skills/create_pull_request/SKILL.md",
            ".codex/skills/kdr-workflow-guide/SKILL.md",
            ".agents/skills/kdr-workflow-guide/SKILL.md",
            ".codex/skills/gh-address-comments/SKILL.md",
        ):
            with self.subTest(path=relative):
                text = (root / relative).read_text(encoding="utf-8")
                self.assertIn("activate/merge/finalize/verify", text)
                self.assertIn("fresh", text)
                self.assertNotIn("activate/merge/finalizeだけ", text)
                self.assertNotIn("activate/merge/finalizeの各apply", text)


if __name__ == "__main__":
    unittest.main()
