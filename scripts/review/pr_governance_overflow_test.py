from __future__ import annotations

import ast
import builtins
import importlib.util
import json
import os
import re
import subprocess
import sys
import tempfile
import textwrap
import time
import unittest
from itertools import zip_longest
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
        self.dispatcher_workflow = (ROOT / ".github/workflows/pr-governance.yml").read_text(encoding="utf-8")
        self.dispatcher = self.dispatcher_workflow + "\n" + "\n".join(
            (ROOT / "scripts/review" / script).read_text(encoding="utf-8")
            for script in (
                "pr_governance_preflight.py",
                "pr_governance_resolve_event.py",
            )
        )
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
        # Terminal writes are split into four bounded, ordered 125-head
        # segments. The segment boundary is part of the dispatch contract,
        # not an old single-run request budget.
        self.assertIn(
            'terminal_write_budget = MAX_TERMINAL_BATCH if scope == "all" and os.environ.get("GITHUB_ACTIONS") == "true"',
            self.writer,
        )
        self.assertIn("MAX_TERMINAL_BATCH = 125", self.writer)
        self.assertIn("MAX_TERMINAL_CONTINUATIONS = 4", self.writer)
        self.assertIn("MAX_TERMINAL_ORDER = MAX_TERMINAL_BATCH * MAX_TERMINAL_CONTINUATIONS", self.writer)
        self.assertIn('re.fullmatch(rf"[1-{MAX_TERMINAL_CONTINUATIONS}]", raw_continuation_index)', self.writer)
        self.assertIn("len(terminal_order) > MAX_TERMINAL_ORDER", self.writer)
        self.assertIn("start = (continuation_index - 1) * MAX_TERMINAL_BATCH", self.writer)
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
            self.dispatcher.count("range(2, 7)") + self.dispatcher.count("range(2,7)"), 8
        )
        for marker in (
            "Open pull request response first page changed.",
            "Current open pull request response first page changed.",
            "active_snapshot_attempts = 4",
            "ActiveWriterSnapshotChanged",
            "Governance writer active run list did not stabilize.",
            "Early writer dispatcher generation is invalid.",
            "Early governance Check Run first page changed.",
            "Affected-head barrier current pull request response first page changed.",
            "Affected-head barrier dispatcher generation is incomplete.",
        ):
            self.assertIn(marker, self.dispatcher)
        self.assertIn(
            'urlencode({"branch":branch,"head_sha":head,"created":f">={generation_created_at}","per_page":"100"})',
            self.dispatcher,
        )
        self.assertIn(
            'urlencode({"branch":branch,"head_sha":head,"created":f">={source.get(\'created_at\')}","per_page":"100","page":str(page_number)})',
            self.dispatcher,
        )
        self.assertNotIn(
            'pr-governance-status-writer.yml/runs?per_page=100&page={page_number}',
            self.dispatcher,
        )
        self.assertNotIn(
            'pr-governance.yml/runs?per_page=100&page={page_number}',
            self.dispatcher,
        )

    def test_every_governance_workflow_api_subprocess_has_a_twenty_second_timeout(self) -> None:
        """Keep the complete API-call inventory bounded as the three workflows grow."""
        workflows = (
            # The two removed per-chunk history scans are replaced by the
            # single aggregate retained-history audit before invalidators.
            ("dispatcher", self.dispatcher, 81),
            ("status writer", self.workflow, 2),
            ("review events", self.review_events, 1),
        )
        for name, workflow, expected_count in workflows:
            with self.subTest(workflow=name):
                blocks = re.finditer(r"(?ms)^          python3 - <<'PY'\n(.*?)^          PY$", workflow)
                api_calls: list[ast.Call] = []
                sources = [
                    "".join(
                        line[10:] if line.startswith("          ") else line
                        for line in block.group(1).splitlines(keepends=True)
                    )
                    for block in blocks
                ]
                if name == "dispatcher":
                    sources.extend(
                        (ROOT / "scripts/review" / script).read_text(encoding="utf-8")
                        for script in (
                            "pr_governance_preflight.py",
                            "pr_governance_resolve_event.py",
                        )
                    )
                for source in sources:
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

    def test_dispatcher_run_blocks_fit_the_actions_command_limit(self) -> None:
        """GitHub Actions rejects an expanded `run:` command over 21,000 bytes."""
        blocks = re.finditer(r"(?ms)^        run: \|\n(.*?)(?=^      - name: |^  [A-Za-z_-]|\Z)", self.dispatcher_workflow)
        for block in blocks:
            command = "".join(
                line[10:] if line.startswith("          ") else line
                for line in block.group(1).splitlines(keepends=True)
            )
            with self.subTest(command=command.splitlines()[0] if command else "empty"):
                self.assertLessEqual(len(command), 21_000)
        for script in ("pr_governance_preflight.py", "pr_governance_resolve_event.py"):
            self.assertLessEqual(
                len((ROOT / "scripts/review" / script).read_text(encoding="utf-8")),
                300_000,
            )

    def test_phase_deadlines_bound_the_complete_300_head_schedule_inside_the_job_deadline(self) -> None:
        phase_seconds = (15 + 15 + 30 + 290) * 60
        self.assertLess(phase_seconds, 6 * 60 * 60)
        self.assertIn("root_deadline_epoch = int(time.time()) + 21_000", self.dispatcher)
        self.assertEqual(self.dispatcher.count("timeout-minutes: 15"), 2)
        self.assertIn("timeout-minutes: 30", self.dispatcher)
        self.assertIn("timeout-minutes: 290", self.dispatcher)
        self.assertIn("The operational cap includes the whole job", self.dispatcher)
        self.assertIn("governed_heads = {target_snapshots[number][0] for number in targets}", self.dispatcher)
        self.assertIn("preserved_governed_heads = {", self.dispatcher)
        self.assertIn("def complete_reconciliation_seconds(head_count):", self.dispatcher)
        self.assertIn("TERMINAL_INTER_SEGMENT_SECONDS = 3_610", self.dispatcher)
        self.assertIn("RECONCILIATION_CONTROL_PLANE_RESERVE_SECONDS = 60 * 60", self.dispatcher)
        self.assertIn("len(governed_heads) > max_governed_heads", self.dispatcher)
        self.assertIn("if invalidation_head_cap_exceeded:", self.dispatcher)
        early_writer_seconds = 120 + 180 + 50 * 2 * 8.1

        def complete_seconds(head_count: int) -> float:
            terminal_heads = max(0, head_count - 50)
            terminal_segments = (terminal_heads + 125 - 1) // 125
            terminal_schedule = 0 if terminal_segments == 0 else 3_750 + (terminal_segments - 1) * max(3_750, 3_610)
            return head_count * 8.1 + early_writer_seconds + terminal_schedule

        cap = max(
            head_count
            for head_count in range(1, 601)
            if complete_seconds(head_count) <= 290 * 60 - 60 * 60
        )
        self.assertEqual(cap, 300)
        self.assertLessEqual(complete_seconds(cap), 290 * 60 - 60 * 60)
        self.assertGreater(complete_seconds(cap + 1), 290 * 60 - 60 * 60)
        self.assertEqual(self.dispatcher.count("ROOT_DEADLINE_EPOCH:"), 5)
        self.assertEqual(
            self.dispatcher.count("Terminal dispatch cannot complete before the root deadline."), 4
        )
        self.assertEqual(self.dispatcher.count("terminal_segment_seconds = 3_750"), 4)

    def test_retained_history_admission_keeps_300_one_page_heads_and_rejects_the_next_budget_unit(self) -> None:
        """History reads are admitted from observed pages before any pending POST."""
        self.assertIn("Audit and fence retained Check Run histories before invalidation", self.dispatcher)
        self.assertIn("HISTORY_SCAN_WORKERS = 4", self.dispatcher)
        self.assertIn("HISTORY_SCAN_COORDINATION_SECONDS = 60", self.dispatcher)
        self.assertIn("HISTORY_SCAN_RATE_REFRESH_REQUESTS = 2", self.dispatcher)
        self.assertIn("Retained-history scan budget cannot complete before the root deadline.", self.dispatcher)
        self.assertIn("HISTORY_CARRY_MANIFEST", self.dispatcher)
        self.assertIn("from datetime import datetime", self.dispatcher)
        self.assertIn("from urllib.parse import parse_qs, urlencode, urlparse", self.dispatcher)
        self.assertIn("history_requests=sum(count+1 for count in page_counts.values())", self.dispatcher)
        self.assertIn("started+scan_seconds+post_scan_seconds>root_deadline", self.dispatcher)
        self.assertIn("history_requests+1>rate_remaining-HISTORY_SCAN_REQUEST_RESERVE", self.dispatcher)
        self.assertNotIn("prior_carry_pending", self.dispatcher)
        self.assertNotIn("prior_scan_deadline", self.dispatcher)

        # One initial page read and one final page-one fence are required for
        # every head. Four fixed workers let the admission test calculate a
        # concrete root-deadline boundary without changing the existing
        # 300-head target cap.
        def scan_seconds(page_counts: list[int]) -> int:
            workers = [0] * 4
            for pages in sorted(page_counts, reverse=True):
                index = min(range(4), key=workers.__getitem__)
                workers[index] += pages * 15
            return 2 * 15 + ((len(page_counts) + 3) // 4) * 15 + max(workers) + 60

        accepted_pages = [1] * 180 + [2] * 120
        rejected_pages = [1] * 179 + [2] * 121
        self.assertEqual(sum(pages + 1 for pages in accepted_pages), 720)
        self.assertEqual(scan_seconds(accepted_pages), 2_790)
        self.assertEqual(sum(pages + 1 for pages in rejected_pages), 721)
        self.assertGreater(scan_seconds(rejected_pages), 2_790)

    def test_retained_history_audit_fences_all_heads_before_segment_posts(self) -> None:
        match = re.search(
            r"- name: Audit and fence retained Check Run histories before invalidation.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.dispatcher_workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = textwrap.dedent(match.group(1))

        def run(
            heads: list[str], root_seconds: int = 100_000, rate_remaining: int = 5_000
        ) -> tuple[subprocess.CompletedProcess[str], list[list[str]]]:
            with tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); fake = directory / "gh"; output = directory / "output"; calls = directory / "calls"; rate = directory / "rate"
                rate.write_text(str(rate_remaining), encoding="utf-8")
                fake.write_text(
                    "#!/usr/bin/env python3\n"
                    "import json, os, pathlib, sys\n"
                    "args = sys.argv[1:]\n"
                    "with open(pathlib.Path(sys.argv[0]).with_name('calls'), 'a', encoding='utf-8') as log: log.write(json.dumps(args) + '\\n')\n"
                    "if args[-1] == 'rate_limit':\n"
                    "    json.dump({'resources': {'core': {'remaining': int(pathlib.Path(sys.argv[0]).with_name('rate').read_text()), 'reset': 4_000_000_000}}}, sys.stdout); raise SystemExit(0)\n"
                    "endpoint = next(value for value in args if value.startswith('repos/'))\n"
                    "head = endpoint.split('/commits/', 1)[1].split('/', 1)[0]\n"
                    "page = int(endpoint.rsplit('page=', 1)[1])\n"
                    "two_pages = head.startswith('a')\n"
                    "full_history = head.startswith('c')\n"
                    "total = 18_001 if full_history else (101 if two_pages else 0)\n"
                    "payload = {'total_count': total, 'check_runs': ([{}] * (100 if page == 1 else 1) if two_pages or full_history else [])}\n"
                    "if '--include' in args: sys.stdout.write('HTTP/2 200\\n\\n')\n"
                    "json.dump(payload, sys.stdout)\n",
                    encoding="utf-8",
                )
                fake.chmod(0o755)
                environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com",
                    "GH_TOKEN": "read", "CHECK_APP_ID": "42", "HEADS": json.dumps(heads),
                    "ROOT_DEADLINE_EPOCH": str(int(time.time()) + root_seconds), "POST_SCAN_SECONDS": "0",
                    "GITHUB_OUTPUT": str(output), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", program], env=environment, capture_output=True, text=True, check=False)
                if result.returncode == 0:
                    manifest = json.loads(dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())["carry_manifest"])
                    self.assertEqual(manifest, [[head, 0] for head in heads])
                observed_calls = [json.loads(line) for line in calls.read_text(encoding="utf-8").splitlines()]
                self.assertTrue(all("--method" not in call for call in observed_calls))
                return result, observed_calls

        heads = [f"{number:040x}" for number in range(1, 301)]
        result, _ = run(heads)
        self.assertEqual(result.returncode, 0, result.stderr)
        accepted = [f"a{number:039x}" for number in range(120)] + [f"b{number:039x}" for number in range(180)]
        rejected = [f"a{number:039x}" for number in range(121)] + [f"b{number:039x}" for number in range(179)]
        # The runtime rate-limit response leaves 721 reads after the fixed
        # 150-request reserve: 720 retained-history reads plus the mandatory
        # final rate refresh. A single additional observed page fails before
        # any segment can POST.
        accepted_result, _ = run(accepted, rate_remaining=871)
        rejected_result, rejected_calls = run(rejected, rate_remaining=871)
        self.assertEqual(accepted_result.returncode, 0, accepted_result.stderr)
        self.assertNotEqual(rejected_result.returncode, 0)
        self.assertEqual(len(rejected_calls), 301)  # rate probe plus first pages only
        root_rejected, _ = run(rejected, root_seconds=2_790)
        self.assertNotEqual(root_rejected.returncode, 0)
        # The configured 150-writer target can encounter 181 retained pages
        # per HEAD.  Its 27,300 Check-Run reads exceed the live admission and
        # fail closed before any invalidator segment posts.
        max_history = [f"c{number:039x}" for number in range(150)]
        max_history_result, max_history_calls = run(max_history)
        self.assertNotEqual(max_history_result.returncode, 0)
        self.assertEqual(len(max_history_calls), 151)

    def test_dispatcher_embedded_python_stays_compatible_with_the_runner(self) -> None:
        """The workflow executes trusted source on the runner's Python 3.9."""
        self.assertIn(
            'program = "from __future__ import annotations\\n" + program',
            self.dispatcher_workflow,
        )
        self.assertIn("def resolver_zip(*iterables, strict=False):", self.dispatcher_workflow)
        self.assertIn("return builtins.zip(*iterables)", self.dispatcher_workflow)
        self.assertIn("zip_longest(*iterables, fillvalue=sentinel)", self.dispatcher_workflow)
        self.assertIn('"zip": resolver_zip', self.dispatcher_workflow)
        self.assertIn("zip(snapshots,targets,strict=True)", self.dispatcher_workflow)
        start = self.dispatcher_workflow.index("          def resolver_zip(")
        end = self.dispatcher_workflow.index("          exec(compile", start)
        namespace = {"builtins": builtins, "zip_longest": zip_longest}
        exec(textwrap.dedent(self.dispatcher_workflow[start:end]), namespace)
        resolver_zip = namespace["resolver_zip"]
        assert callable(resolver_zip)
        self.assertEqual(list(resolver_zip((1, 2), (3, 4))), [(1, 3), (2, 4)])
        with self.assertRaises(ValueError):
            list(resolver_zip((1,), (2, 3), strict=True))

    def test_preinvalidation_and_all_open_union_of_550_heads_fails_closed_before_chunking(self) -> None:
        """The two invalidator partitions must share one computed run budget."""
        start = self.dispatcher_workflow.index("          governed_heads = {target_snapshots[number][0] for number in targets}")
        end = self.dispatcher_workflow.index("          pre_chunk_snapshots = [", start)
        source = textwrap.dedent(self.dispatcher_workflow[start:end])
        preinvalidate_targets = list(range(1, 51))
        all_invalidation_targets = list(range(51, 551))
        namespace = {
            "EARLY_WRITER_TARGET_CAP": 50,
            "CHECK_RUN_WRITE_PACE_SECONDS": 8.1,
            "EARLY_WRITER_STARTUP_SECONDS": 120,
            "EARLY_WRITER_INITIAL_EVIDENCE_SECONDS": 180,
            "TERMINAL_BATCH_HEAD_CAP": 125,
            "TERMINAL_SEGMENT_SECONDS": 3_750,
            "TERMINAL_INTER_SEGMENT_SECONDS": 3_610,
            "RECONCILIATION_JOB_TIMEOUT_SECONDS": 290 * 60,
            "RECONCILIATION_CONTROL_PLANE_RESERVE_SECONDS": 60 * 60,
            "target_snapshots": {
                number: (f"{number:040x}", False)
                for number in preinvalidate_targets + all_invalidation_targets
            },
            "targets": preinvalidate_targets + all_invalidation_targets,
            "priority_targets": preinvalidate_targets,
            "preinvalidate_targets": preinvalidate_targets,
            "all_invalidation_targets": all_invalidation_targets,
            "all_invalidation_heads": {
                f"{number:040x}" for number in all_invalidation_targets
            },
        }

        exec(source, namespace)

        self.assertEqual(len(namespace["invalidation_governed_heads"]), 550)
        self.assertEqual(namespace["max_governed_heads"], 300)
        self.assertTrue(namespace["invalidation_head_cap_exceeded"])
        self.assertEqual(namespace["pre_chunks"], [[], []])
        self.assertEqual(namespace["all_chunks"], [[], []])

    def test_terminal_budget_uses_all_governed_heads_not_invalidation_union(self) -> None:
        """A 50-head early prefix cannot hide its remaining terminal segment."""
        start = self.dispatcher_workflow.index("          governed_heads = {target_snapshots[number][0] for number in targets}")
        end = self.dispatcher_workflow.index("          pre_chunk_snapshots = [", start)
        source = textwrap.dedent(self.dispatcher_workflow[start:end])
        targets = list(range(1, 350))
        priority_targets = list(range(1, 51))
        preinvalidate_targets = [1]
        all_invalidation_targets = list(range(51, 350))
        namespace = {
            "EARLY_WRITER_TARGET_CAP": 50,
            "CHECK_RUN_WRITE_PACE_SECONDS": 8.1,
            "EARLY_WRITER_STARTUP_SECONDS": 120,
            "EARLY_WRITER_INITIAL_EVIDENCE_SECONDS": 180,
            "TERMINAL_BATCH_HEAD_CAP": 125,
            "TERMINAL_SEGMENT_SECONDS": 3_750,
            "TERMINAL_INTER_SEGMENT_SECONDS": 3_610,
            "RECONCILIATION_JOB_TIMEOUT_SECONDS": 290 * 60,
            "RECONCILIATION_CONTROL_PLANE_RESERVE_SECONDS": 60 * 60,
            "target_snapshots": {number: (f"{number:040x}", False) for number in targets},
            "targets": targets,
            "priority_targets": priority_targets,
            "preinvalidate_targets": preinvalidate_targets,
            "all_invalidation_targets": all_invalidation_targets,
            "all_invalidation_heads": {
                f"{number:040x}" for number in all_invalidation_targets
            },
        }

        exec(source, namespace)

        self.assertEqual(len(namespace["invalidation_governed_heads"]), 300)
        self.assertEqual(len(namespace["governed_heads"]), 349)
        self.assertEqual(len(namespace["preserved_governed_heads"]), 50)
        self.assertEqual(namespace["max_governed_heads"], 300)
        self.assertTrue(namespace["invalidation_head_cap_exceeded"])
        self.assertEqual(namespace["pre_chunks"], [[], []])
        self.assertEqual(namespace["all_chunks"], [[], []])

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
