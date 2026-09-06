from __future__ import annotations

import ast
import importlib.util
import re
import sys
import unittest
from pathlib import Path
from unittest.mock import patch


ROOT = Path(__file__).parents[2]
SPEC = importlib.util.spec_from_file_location(
    "pr_governance_status_writer_overflow", ROOT / "scripts/review/pr_governance_status_writer.py"
)
assert SPEC is not None and SPEC.loader is not None
WRITER = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = WRITER
SPEC.loader.exec_module(WRITER)


class GovernanceOverflowContractTest(unittest.TestCase):
    def setUp(self) -> None:
        self.writer = (ROOT / "scripts/review/pr_governance_status_writer.py").read_text(encoding="utf-8")
        self.workflow = (ROOT / ".github/workflows/pr-governance-status-writer.yml").read_text(encoding="utf-8")
        self.dispatcher = (ROOT / ".github/workflows/pr-governance.yml").read_text(encoding="utf-8")
        self.review_events = (ROOT / ".github/workflows/pr-governance-review-events.yml").read_text(
            encoding="utf-8"
        )

    def test_writer_has_no_matrix_or_256_target_limit(self) -> None:
        self.assertNotIn("matrix:", self.workflow)
        self.assertNotIn("MAX_MATRIX", self.writer)
        self.assertIn("tuple(number for number in targets if number not in preserved) if scope == \"all\" else ()", self.writer)
        self.assertIn("failures += 1", self.writer)

    def test_bounded_terminal_writes_carry_the_tail_to_the_next_dispatcher(self) -> None:
        self.assertIn("def governance_order(", self.writer)
        self.assertIn("def dispatcher_invalidation_url(", self.writer)
        self.assertIn('urlencode({"dispatcher_run_id": str(source.identifier), "carry_pending": str(carry_pending)})', self.writer)
        self.assertIn("Bind the writer scope to current App invalidations from one dispatcher.", self.writer)
        self.assertIn("Draft pull request cannot carry a terminal governance decision.", self.writer)
        # Terminal writes are split into four bounded, ordered segments.  The
        # segment boundary is part of the dispatch contract, not an old
        # single-run request budget.
        self.assertIn(
            'terminal_write_budget = 150 if scope == "all" and os.environ.get("GITHUB_ACTIONS") == "true"',
            self.writer,
        )
        self.assertIn('re.fullmatch(r"[1-4]", raw_continuation_index)', self.writer)
        self.assertIn("len(terminal_order) > 600", self.writer)
        self.assertIn("start = (continuation_index - 1) * 150", self.writer)
        self.assertIn("terminal_batch != expected_terminal_batch", self.writer)

        snapshot = WRITER.OpenSnapshot(
            (72, 73), {},
            (
                {"number": 72, "isDraft": False, "head_sha": "a" * 40},
                {"number": 73, "isDraft": False, "head_sha": "b" * 40},
            ),
        )
        source = WRITER.DispatcherSource(88, "issues", 1)
        with patch.multiple(WRITER, REPOSITORY="owner/repository", SERVER_URL="https://github.com"):
            marker = {
                "status": "in_progress", "conclusion": None,
                "details_url": WRITER.dispatcher_invalidation_url(source, 0),
            }
            with patch.object(WRITER, "check_run", return_value=marker) as checks:
                early, carry = WRITER.observed_invalidations(snapshot, source, "early", (72,))
            self.assertEqual(early.numbers, (72,))
            self.assertEqual(carry, frozenset())
            # The early writer owns its immutable pending Check Run after it
            # acquires the singleton; no dispatcher-side read/patch marker.
            self.assertEqual(checks.call_count, 0)

            with patch.object(WRITER, "check_run", return_value=marker) as checks:
                all_open, carry = WRITER.observed_invalidations(snapshot, source, "all", ())
            self.assertEqual(all_open.numbers, (72, 73))
            self.assertEqual(carry, frozenset())
            self.assertEqual(checks.call_count, 2)

    def test_preserved_early_source_is_removed_before_remaining_affected_targets_are_ordered(self) -> None:
        snapshot = WRITER.OpenSnapshot(
            (72, 73, 74), {},
            (
                {"number": 72, "isDraft": False, "head_sha": "a" * 40},
                {"number": 73, "isDraft": False, "head_sha": "b" * 40},
                {"number": 74, "isDraft": False, "head_sha": "c" * 40},
            ),
        )
        # The source was terminalized by the early writer; its sibling
        # claimant remains ahead of unrelated PRs in the all-open writer.
        selected = WRITER.OpenSnapshot(
            (73, 74), {}, tuple(item for item in snapshot.pull_requests if item["number"] != 72),
        )
        self.assertEqual(WRITER.governance_order(selected, frozenset(), (73,)), (73, 74))

    def test_open_pr_and_check_run_api_reads_use_fixed_pages_and_anchor_fences(self) -> None:
        self.assertIn('pulls?state=open&per_page=100', self.writer)
        self.assertIn('"check_name": CHECK_NAME', self.writer)
        self.assertIn("MAX_SHARED_SNAPSHOT_PAGES = 6", self.writer)
        self.assertIn("def _page_endpoint", self.writer)
        self.assertIn("def _included_page", self.writer)
        self.assertIn('command(["--include", endpoint]', self.writer)
        self.assertIn("GitHub pagination first page changed.", self.writer)
        self.assertNotIn("--paginate", self.writer)
        self.assertNotIn("--slurp", self.writer)

    def test_dispatcher_pagination_is_bounded_and_event_safe(self) -> None:
        self.assertNotIn("--paginate", self.dispatcher)
        self.assertNotIn("--slurp", self.dispatcher)
        self.assertGreaterEqual(self.dispatcher.count("timeout=20"), 9)
        self.assertGreaterEqual(self.dispatcher.count('rel="next"'), 9)
        self.assertGreaterEqual(
            self.dispatcher.count("range(2, 7)") + self.dispatcher.count("range(2,7)"), 9
        )
        for marker in (
            "Open pull request response first page changed.",
            "Current open pull request response first page changed.",
            "active_snapshot_attempts = 4",
            "ActiveWriterSnapshotChanged",
            "Governance writer active run list did not stabilize.",
            "Governance writer runs first page changed.",
            "Early governance Check Run first page changed.",
            "Affected-head barrier current pull request response first page changed.",
            "Affected-head barrier dispatcher generation is incomplete.",
        ):
            self.assertIn(marker, self.dispatcher)

    def test_every_governance_workflow_api_subprocess_has_a_twenty_second_timeout(self) -> None:
        """Keep the complete API-call inventory bounded as the three workflows grow."""
        workflows = (
            ("dispatcher", self.dispatcher, 79),
            ("status writer", self.workflow, 2),
            ("review events", self.review_events, 1),
        )
        for name, workflow, expected_count in workflows:
            with self.subTest(workflow=name):
                blocks = re.finditer(r"(?ms)^          python3 - <<'PY'\n(.*?)^          PY$", workflow)
                api_calls: list[ast.Call] = []
                for block in blocks:
                    source = "".join(
                        line[10:] if line.startswith("          ") else line
                        for line in block.group(1).splitlines(keepends=True)
                    )
                    tree = ast.parse(source)
                    finite_timeout_names = {
                        target.id: value.value
                        for assignment in ast.walk(tree)
                        if isinstance(assignment, ast.Assign)
                        and len(assignment.targets) == 1
                        and isinstance(target := assignment.targets[0], ast.Name)
                        and isinstance(value := assignment.value, ast.Constant)
                        and isinstance(value.value, (int, float))
                        and 0 < value.value <= 20
                    }
                    for call in ast.walk(tree):
                        if not (
                            isinstance(call, ast.Call)
                            and isinstance(call.func, ast.Attribute)
                            and isinstance(call.func.value, ast.Name)
                            and call.func.value.id == "subprocess"
                            and call.func.attr == "run"
                        ):
                            continue
                        rendered = ast.get_source_segment(source, call) or ""
                        if '["sleep"' in rendered or "['sleep'" in rendered:
                            continue
                        api_calls.append(call)
                        timeout = next((item.value for item in call.keywords if item.arg == "timeout"), None)
                        self.assertIsNotNone(timeout, rendered)
                        if isinstance(timeout, ast.Constant):
                            self.assertIsInstance(timeout.value, (int, float))
                            self.assertLessEqual(timeout.value, 20)
                        else:
                            self.assertIsInstance(timeout, ast.Call)
                            assert isinstance(timeout, ast.Call)
                            self.assertIsInstance(timeout.func, ast.Name)
                            assert isinstance(timeout.func, ast.Name)
                            self.assertEqual(timeout.func.id, "min")
                            self.assertIn("remaining", ast.unparse(timeout))
                            self.assertRegex(
                                source,
                                r"\bremaining\s*=\s*[A-Za-z_]*deadline\s*-\s*time\.(?:time|monotonic)\(\)",
                            )
                            self.assertTrue(
                                any(
                                    (
                                        isinstance(argument, ast.Constant)
                                        and isinstance(argument.value, (int, float))
                                        and 0 < argument.value <= 20
                                    )
                                    or (
                                        isinstance(argument, ast.Name)
                                        and argument.id in finite_timeout_names
                                    )
                                    for argument in timeout.args
                                ),
                                ast.unparse(timeout),
                            )

                # Updating this count makes a new production subprocess explicit
                # in review; the assertion above makes its timeout non-optional.
                self.assertEqual(len(api_calls), expected_count)

    def test_phase_deadlines_leave_a_terminal_start_margin_inside_six_hours(self) -> None:
        phase_seconds = (15 + 15 + 30 + 290) * 60
        self.assertLess(phase_seconds, 6 * 60 * 60)
        self.assertIn("root_deadline_epoch = int(time.time()) + 21_000", self.dispatcher)
        self.assertEqual(self.dispatcher.count("timeout-minutes: 15"), 2)
        self.assertIn("timeout-minutes: 30", self.dispatcher)
        self.assertIn("timeout-minutes: 290", self.dispatcher)
        self.assertEqual(self.dispatcher.count("ROOT_DEADLINE_EPOCH:"), 4)
        self.assertEqual(
            self.dispatcher.count("Terminal dispatch cannot complete before the root deadline."), 4
        )
        self.assertEqual(self.dispatcher.count("terminal_segment_seconds = 3_750"), 4)

    def test_malformed_or_multi_closing_prs_fail_closed_without_aborting_other_prs(self) -> None:
        self.assertIn("A malformed multi-Issue closer is a claimant", self.writer)
        self.assertIn("Canonical Issue closer set changed.", self.writer)
        self.assertIn("Do not make one malformed/changed PR leave other open PRs stale.", self.writer)

    def test_single_snapshot_removes_quadratic_300_pr_revalidation(self) -> None:
        self.assertIn("Take one complete O(N) open-PR snapshot", self.writer)
        self.assertIn("one complete snapshot prevents O(N^2) GETs", self.writer)
        self.assertNotIn("for listed in open_pulls()", self.writer)


if __name__ == "__main__":
    unittest.main()
