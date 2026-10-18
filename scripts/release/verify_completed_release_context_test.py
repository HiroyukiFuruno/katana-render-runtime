"""公開context境界を実APIのJSON構造に合わせて検証する単体テスト。"""

import copy
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest

JsonObject = dict[str, object]


def json_object(value: object) -> JsonObject:
    if not isinstance(value, dict) or not all(isinstance(key, str) for key in value):
        raise TypeError("fixture value is not a JSON object")
    return value


def object_at(parent: JsonObject, key: str) -> JsonObject:
    return json_object(parent[key])


def array_at(parent: JsonObject, key: str) -> list[object]:
    value = parent[key]
    if not isinstance(value, list):
        raise TypeError("fixture value is not a JSON array")
    return value


def increment_integer(parent: JsonObject, key: str, amount: int = 1) -> None:
    value = parent[key]
    if type(value) is not int:
        raise TypeError("fixture value is not an integer")
    parent[key] = value + amount


spec = importlib.util.spec_from_file_location(
    "release_context", Path(__file__).with_name("verify_completed_release_context.py")
)
if spec is None or spec.loader is None:
    raise ImportError("release-context module spec is unavailable")
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)
REPO = "Owner/repository"
HEAD = "a" * 40
PR_HEAD = "b" * 40
TREE = "c" * 40
BASE = "repos/" + REPO


class CompletedReleaseContextTest(unittest.TestCase):
    merge_run: JsonObject
    pull: JsonObject
    preflight: JsonObject
    jobs: list[JsonObject]
    responses: dict[str, object]

    def setUp(self) -> None:
        repo = {"full_name": REPO}
        self.merge_run = {
            "run_started_at": "2026-10-10T01:00:00Z",
            "id": 20,
            "run_attempt": 1,
            "workflow_id": 2,
            "path": gate.CI_PATH,
            "head_sha": HEAD,
            "head_branch": "master",
            "event": "push",
            "status": "completed",
            "conclusion": "success",
            "repository": repo,
            "head_repository": repo,
        }
        self.pull = {
            "number": 7,
            "state": "closed",
            "merged": True,
            "merged_at": "2026-10-10T01:00:00Z",
            "draft": False,
            "merge_commit_sha": HEAD,
            "head": {"ref": "release/v0.4.24", "sha": PR_HEAD, "repo": repo},
            "base": {"ref": "master", "repo": repo},
        }
        self.preflight = {
            "workflow_id": 2,
            "head_sha": "d" * 40,
            "head_branch": "release/v0.4.24",
            "event": "pull_request",
            "id": 19,
            "run_attempt": 1,
            "run_started_at": "2026-10-10T00:00:00Z",
            "path": gate.CI_PATH,
            "repository": repo,
            "head_repository": repo,
            "status": "completed",
            "conclusion": "success",
            "pull_requests": [{"number": 7, "head": {"sha": PR_HEAD}}],
        }
        self.jobs = [
            {
                "name": name,
                "run_id": 20,
                "head_sha": HEAD,
                "status": "completed",
                "conclusion": "success",
            }
            for name in (*gate.NATIVE_JOBS, gate.PAIRED_JOB)
        ]
        self.responses = {
            f"{BASE}/actions/workflows/test-and-build.yml": {
                "id": 2,
                "path": gate.CI_PATH,
                "state": "active",
            },
            f"{BASE}/actions/workflows/test-and-build.yml/runs?head_sha={HEAD}&per_page=100&page=1": {
                "total_count": 1,
                "workflow_runs": [self.merge_run],
            },
            f"{BASE}/commits/{HEAD}/pulls?per_page=100&page=1": [self.pull],
            f"{BASE}/pulls/7": self.pull,
            f"{BASE}/actions/runs/20/attempts/1/jobs?per_page=100&page=1": {
                "total_count": 4,
                "jobs": self.jobs,
            },
            f"{BASE}/actions/workflows/test-and-build.yml/runs?event=pull_request&branch=release/v0.4.24&per_page=100&page=1": {
                "total_count": 1,
                "workflow_runs": [self.preflight],
            },
            f"{BASE}/actions/runs/19/attempts/1/jobs?per_page=100&page=1": {
                "total_count": 1,
                "jobs": [
                    {
                        "name": "preflight",
                        "run_id": 19,
                        "status": "completed",
                        "conclusion": "success",
                    }
                ],
            },
            f"{BASE}/git/commits/{HEAD}": {"sha": HEAD, "tree": {"sha": TREE}},
            f"{BASE}/actions/runs/20": self.merge_run,
            f"{BASE}/actions/runs/19": self.preflight,
        }

    def fetch(self, path: str) -> object:
        return copy.deepcopy(self.responses[path])

    def response_object(self, path: str) -> JsonObject:
        return json_object(self.responses[path])

    def response_array(self, path: str, key: str) -> list[object]:
        return array_at(self.response_object(path), key)

    def response_items(self, path: str) -> list[object]:
        value = self.responses[path]
        if not isinstance(value, list):
            raise TypeError("fixture value is not a JSON array")
        return value

    def reject_mutations(
        self, payload: JsonObject, variants: tuple[tuple[str, object], ...]
    ) -> None:
        for key, value in variants:
            with self.subTest(key=key):
                original = payload[key]
                payload[key] = value
                with self.assertRaises(ValueError):
                    self.verify()
                payload[key] = original

    def verify(
        self,
        run_id: int | None = None,
        run_attempt: int | None = None,
        automatic: bool = False,
    ) -> dict[str, object]:
        return json_object(
            gate.verify(self.fetch, REPO, HEAD, run_id, run_attempt, automatic)
        )

    def test_success_emits_exact_merge_tree_branch_and_generation(self) -> None:
        self.assertEqual(
            self.verify(run_id=20, run_attempt=1, automatic=True),
            {
                "eligible": "true",
                "sha": HEAD,
                "tree": TREE,
                "version": "v0.4.24",
                "release_branch": "release/v0.4.24",
                "pull_request": "7",
                "run_id": "20",
                "run_attempt": "1",
            },
        )

    def test_nonrelease_master_is_skipped_only_automatically(self) -> None:
        self.responses[f"{BASE}/commits/{HEAD}/pulls?per_page=100&page=1"] = []
        self.assertEqual(self.verify(automatic=True), {"eligible": "false"})
        with self.assertRaisesRegex(ValueError, "one merged release"):
            self.verify()

    def test_unmerged_draft_and_foreign_release_pr_rejected(self) -> None:
        self.reject_mutations(
            self.pull,
            (
                ("state", "open"),
                ("merged", False),
                ("draft", True),
                ("merge_commit_sha", PR_HEAD),
            ),
        )
        object_at(self.pull, "head")["repo"] = {"full_name": "fork/repository"}
        with self.assertRaisesRegex(ValueError, "repository/base"):
            self.verify()

    def test_wrong_base_rejected(self) -> None:
        object_at(self.pull, "base")["ref"] = "develop"
        with self.assertRaisesRegex(ValueError, "repository/base"):
            self.verify()

    def test_duplicate_release_pr_rejected(self) -> None:
        self.response_items(f"{BASE}/commits/{HEAD}/pulls?per_page=100&page=1").append(
            copy.deepcopy(self.pull)
        )
        with self.assertRaisesRegex(ValueError, "one merged release"):
            self.verify()

    def test_pending_failed_foreign_wrong_workflow_or_nonmaster_ci_rejected(
        self,
    ) -> None:
        self.reject_mutations(
            self.merge_run,
            (
                ("status", "in_progress"),
                ("conclusion", "failure"),
                ("workflow_id", 99),
                ("path", ".github/workflows/other.yml"),
                ("event", "pull_request"),
                ("head_branch", "release/v0.4.24"),
                ("head_repository", {"full_name": "fork/repository"}),
            ),
        )

    def test_older_run_rerun_started_later_cannot_reuse_newer_run_success(self) -> None:
        newer_attempt = {
            **self.merge_run,
            "id": 18,
            "run_attempt": 2,
            "run_started_at": "2026-10-10T02:00:00Z",
            "status": "in_progress",
        }
        path = f"{BASE}/actions/workflows/test-and-build.yml/runs?head_sha={HEAD}&per_page=100&page=1"
        payload = self.response_object(path)
        self.response_array(path, "workflow_runs").append(newer_attempt)
        payload["total_count"] = 2
        with self.assertRaisesRegex(ValueError, "not completed"):
            self.verify()

    def test_stale_workflow_run_event_rejected(self) -> None:
        for run_id, attempt in ((19, 1), (20, 2)):
            with self.assertRaisesRegex(ValueError, "event is stale"):
                self.verify(run_id=run_id, run_attempt=attempt)

    def test_every_native_and_paired_job_must_succeed(self) -> None:
        for job in self.jobs:
            with self.subTest(job=job["name"]):
                job["conclusion"] = "skipped"
                with self.assertRaisesRegex(ValueError, "not completed successfully"):
                    self.verify()
                job["conclusion"] = "success"
                job["head_sha"] = PR_HEAD
                with self.assertRaisesRegex(ValueError, "foreign/duplicate"):
                    self.verify()
                job["head_sha"] = HEAD

    def test_missing_duplicate_and_incomplete_jobs_rejected(self) -> None:
        self.jobs.pop()
        with self.assertRaisesRegex(ValueError, "incomplete"):
            self.verify()
        self.response_object(
            f"{BASE}/actions/runs/20/attempts/1/jobs?per_page=100&page=1"
        )["total_count"] = 3
        with self.assertRaisesRegex(ValueError, "required CI job"):
            self.verify()

    def test_preflight_must_be_latest_exact_pr_head_success(self) -> None:
        self.reject_mutations(
            self.preflight,
            (
                ("status", "queued"),
                ("conclusion", "failure"),
                ("head_repository", {"full_name": "fork/repository"}),
            ),
        )
        pull_request = json_object(array_at(self.preflight, "pull_requests")[0])
        object_at(pull_request, "head")["sha"] = HEAD
        with self.assertRaisesRegex(ValueError, "exact-PR preflight"):
            self.verify()

    def test_skipped_required_preflight_job_rejected(self) -> None:
        job_payload = self.response_object(
            f"{BASE}/actions/runs/19/attempts/1/jobs?per_page=100&page=1"
        )
        json_object(array_at(job_payload, "jobs")[0])["conclusion"] = "skipped"
        with self.assertRaisesRegex(ValueError, "preflight is not completed"):
            self.verify()

    def test_mutating_ci_or_preflight_generation_and_pr_rejected(self) -> None:
        for path in (
            f"{BASE}/actions/runs/20",
            f"{BASE}/actions/runs/19",
            f"{BASE}/pulls/7",
        ):
            with self.subTest(path=path):
                count = 0

                def changed_fetch(requested: str, request_path: str = path) -> object:
                    nonlocal count
                    value = self.fetch(requested)
                    if requested == request_path:
                        count += 1
                        if "/pulls/" not in request_path or count > 1:
                            json_object(value)["changed"] = True
                    return value

                with self.assertRaisesRegex(ValueError, "changed"):
                    gate.verify(changed_fetch, REPO, HEAD)

    def test_same_second_preflight_pending_rerun_is_ambiguous(self) -> None:
        path = f"{BASE}/actions/workflows/test-and-build.yml/runs?event=pull_request&branch=release/v0.4.24&per_page=100&page=1"
        payload = self.response_object(path)
        self.response_array(path, "workflow_runs").append(
            {
                **self.preflight,
                "id": 18,
                "run_attempt": 2,
                "status": "in_progress",
                "conclusion": None,
            }
        )
        payload["total_count"] = 2
        with self.assertRaisesRegex(ValueError, "ambiguous latest exact-PR preflight"):
            self.verify()

    def test_same_second_merge_pending_rerun_is_ambiguous(self) -> None:
        path = f"{BASE}/actions/workflows/test-and-build.yml/runs?head_sha={HEAD}&per_page=100&page=1"
        payload = self.response_object(path)
        self.response_array(path, "workflow_runs").append(
            {
                **self.merge_run,
                "id": 18,
                "run_attempt": 2,
                "status": "in_progress",
                "conclusion": None,
            }
        )
        payload["total_count"] = 2
        with self.assertRaisesRegex(ValueError, "ambiguous latest exact-source CI"):
            self.verify()

    def test_missing_malformed_calendar_timestamp_rejected_for_both_lanes(self) -> None:
        for row in (self.merge_run, self.preflight):
            for timestamp in (None, "invalid", "2026-99-10T00:00:00Z"):
                with self.subTest(lane=row["id"], timestamp=timestamp):
                    previous = row["run_started_at"]
                    row["run_started_at"] = timestamp
                    with self.assertRaisesRegex(ValueError, "generation start time"):
                        self.verify()
                    row["run_started_at"] = previous

    def test_fresh_lists_reject_new_ci_or_preflight_generations_during_verification(
        self,
    ) -> None:
        merge_path = f"{BASE}/actions/workflows/test-and-build.yml/runs?head_sha={HEAD}&per_page=100&page=1"
        preflight_path = f"{BASE}/actions/workflows/test-and-build.yml/runs?event=pull_request&branch=release/v0.4.24&per_page=100&page=1"
        for path, original in (
            (merge_path, self.merge_run),
            (preflight_path, self.preflight),
        ):
            for mode in ("new_pending", "new_success", "same_id_new_attempt"):
                with self.subTest(lane=original["id"], mode=mode):
                    tree_read = False
                    list_reads = 0

                    def racing_fetch(
                        requested: str,
                        request_path: str = path,
                        original_row: JsonObject = original,
                        selected_mode: str = mode,
                    ) -> object:
                        nonlocal tree_read, list_reads
                        value = self.fetch(requested)
                        if requested == f"{BASE}/git/commits/{HEAD}":
                            tree_read = True
                        if requested == request_path:
                            list_reads += 1
                            if tree_read:
                                new = {
                                    **original_row,
                                    "run_started_at": "2026-10-10T02:00:00Z",
                                }
                                if selected_mode == "same_id_new_attempt":
                                    increment_integer(new, "run_attempt")
                                    json_object(value)["workflow_runs"] = [new]
                                else:
                                    increment_integer(new, "id", 2)
                                    if selected_mode == "new_pending":
                                        new.update(
                                            status="in_progress", conclusion=None
                                        )
                                    response_payload = json_object(value)
                                    array_at(response_payload, "workflow_runs").append(
                                        new
                                    )
                                    increment_integer(response_payload, "total_count")
                        return value

                    with self.assertRaises(ValueError):
                        gate.verify(racing_fetch, REPO, HEAD)
                    self.assertTrue(tree_read)
                    self.assertEqual(list_reads, 2)

    def test_malformed_committed_tree_rejected(self) -> None:
        commit = self.response_object(f"{BASE}/git/commits/{HEAD}")
        object_at(commit, "tree")["sha"] = "invalid"
        with self.assertRaisesRegex(ValueError, "commit/tree"):
            self.verify()

    def test_publication_receipt_binds_exact_acquired_generation(self) -> None:
        context = self.verify()
        receipt = {"repository": REPO, "head": HEAD, "runId": 20, "runAttempt": 1}
        gate.bind_acquisition(context, receipt, REPO)
        for key, value in (
            ("repository", "fork/repository"),
            ("head", PR_HEAD),
            ("runId", 21),
            ("runAttempt", 2),
        ):
            with self.subTest(key=key):
                with self.assertRaisesRegex(ValueError, "acquired evidence generation"):
                    gate.bind_acquisition(context, {**receipt, key: value}, REPO)
        with self.assertRaisesRegex(ValueError, "nonrelease"):
            gate.bind_acquisition({"eligible": "false"}, receipt, REPO)

    def test_local_committed_tree_and_version_are_bound(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "Cargo.toml").write_text(
                '[package]\nname="unit"\nversion="0.4.24"\n'
            )

            def git(*args: str) -> str:
                return subprocess.check_output(
                    ["git", "-C", str(root), *args], text=True
                ).strip()

            git("init", "-q", "-b", "master")
            git("add", ".")
            git(
                "-c",
                "user.name=Unit",
                "-c",
                "user.email=unit@example.invalid",
                "commit",
                "-qm",
                "fixture",
            )
            git("update-ref", "refs/remotes/origin/master", "HEAD")
            context = {
                "eligible": "true",
                "sha": git("rev-parse", "HEAD"),
                "tree": git("rev-parse", "HEAD^{tree}"),
                "version": "v0.4.24",
            }
            gate.bind_tree(root, context, "0.4.24")
            with self.assertRaisesRegex(ValueError, "requested"):
                gate.bind_tree(root, context, "v0.4.25")
            with self.assertRaisesRegex(ValueError, "local tree"):
                gate.bind_tree(root, {**context, "tree": TREE})
            with self.assertRaisesRegex(ValueError, "Cargo version"):
                gate.bind_tree(root, {**context, "version": "v0.4.25"})


class ReleaseCompletionRoutingTest(unittest.TestCase):
    def setUp(self) -> None:
        self.root = Path(__file__).parents[2]
        self.ci = (self.root / ".github/workflows/test-and-build.yml").read_text()
        self.release = (self.root / ".github/workflows/release.yml").read_text()
        self.preflight = (
            self.root / ".github/workflows/release-preflight.yml"
        ).read_text()
        self.retry = (
            self.root / ".github/workflows/release-publish-retry.yml"
        ).read_text()

    def test_public_release_is_ci_completion_driven_and_merge_sha_bound(self) -> None:
        self.assertIn(
            "workflow_run:\n    workflows: [CI]\n    types: [completed]", self.release
        )
        self.assertNotIn("  pull_request:", self.release)
        self.assertIn(
            "github.event.workflow_run.head_repository.full_name == github.repository",
            self.release,
        )
        self.assertIn(
            "if: needs.release-context.outputs.eligible == 'true'", self.release
        )
        self.assertIn("ref: ${{ needs.release-context.outputs.sha }}", self.release)
        self.assertLess(
            self.release.index("Verify completed merged release context"),
            self.release.index("Checkout verified merge source"),
        )
        self.assertLess(
            self.release.index("Reverify completed release identity"),
            self.release.index("Create release tag"),
        )

    def test_pr_preflight_is_same_ci_dependency_without_duplicate_context(self) -> None:
        self.assertIn(
            "  preflight:\n    name: preflight\n    needs: [test, paired-performance]",
            self.ci,
        )
        self.assertIn("--acquire-current-producer", self.ci)
        self.assertIn("Release preflight checks", self.ci)
        self.assertNotIn("  pull_request:", self.preflight)
        self.assertNotIn("for attempt", self.preflight)
        self.assertIn(
            'run: just VERSION="${{ steps.version.outputs.version }}" release-check',
            self.preflight,
        )

    def test_public_manual_and_retry_never_accept_running_workflow(self) -> None:
        justfile = (self.root / "Justfile").read_text()
        for text in (justfile, self.release, self.preflight, self.retry):
            self.assertNotIn("--wait-for-ci", text)
            self.assertNotIn("--acquire-current-producer", text)
        self.assertIn("verify_completed_release_context.py", self.retry)
        self.assertIn(
            '--head "$(git --no-replace-objects -C release-source rev-parse HEAD)"',
            self.retry,
        )
        self.assertIn("--acquire", self.retry)

    def test_both_publication_routes_bind_receipt_after_acquisition(self) -> None:
        for workflow, reverify, acquire, publish in (
            (
                self.release,
                "Reverify completed release identity before publication",
                "Acquire completed exact-source paired performance evidence",
                "Create release tag",
            ),
            (
                self.retry,
                "Reverify acquired completed generation before retry publication",
                "Acquire completed exact-tag CI and paired performance evidence",
                "Publish crates.io from release tag",
            ),
        ):
            with self.subTest(publication=publish):
                self.assertLess(workflow.index(acquire), workflow.index(reverify))
                self.assertLess(workflow.index(reverify), workflow.index(publish))
                step = workflow[
                    workflow.index("      - name: " + reverify) : workflow.index(
                        "      - name: " + publish
                    )
                ]
                self.assertIn("--acquisition-receipt ", step)

    def test_retry_context_reads_pull_requests_with_explicit_permission(self) -> None:
        permissions = self.retry[
            self.retry.index("permissions:") : self.retry.index("\nenv:")
        ]
        self.assertIn("  actions: read\n", permissions)
        self.assertIn("  contents: write\n", permissions)
        self.assertIn("  pull-requests: read\n", permissions)

    def test_both_performance_gates_remain_mandatory(self) -> None:
        justfile = (self.root / "Justfile").read_text()
        self.assertIn(
            "release-verify: html-performance-nonregression-check release-target-check",
            justfile,
        )
        self.assertRegex(
            justfile, r"(?m)^release-quality: .*html-performance-nonregression-check$"
        )


if __name__ == "__main__":
    unittest.main()
