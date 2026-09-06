from __future__ import annotations

import ast
import json
import importlib.util
import os
import re
import stat
import subprocess
import sys
import tempfile
import textwrap
import time
import unittest
from pathlib import Path
from unittest.mock import patch
from urllib.parse import parse_qs, urlparse


ROOT = Path(__file__).parents[2]
sys.path.insert(0, str(ROOT / "scripts/hooks"))
import verify_push_issue as canonical_issue_contract


class GovernanceReviewSensorIdentityContractTest(unittest.TestCase):
    """Exercise the Actions workflow_run repository identity boundary."""

    repository = "owner/repository"
    repository_identity = (101, "repository", f"https://api.github.com/repos/{repository}")
    head = "a" * 40
    base = "b" * 40

    def setUp(self) -> None:
        self.sensor = (ROOT / ".github/workflows/pr-governance-review-events.yml").read_text(encoding="utf-8")
        start = self.sensor.index("          check_name =")
        end = self.sensor.index("          deadline =", start)
        self.program = textwrap.dedent(self.sensor[start:end])

    def namespace(self) -> dict[str, object]:
        environment = {
            "HEAD_SHA": self.head,
            "PR_NUMBER": "72",
            "PR_BASE_SHA": self.base,
            "PR_BASE_REF": "master",
            "DEFAULT_BRANCH": "master",
            "SOURCE_RUN_ID": "17",
            "CHECK_APP_ID": "4766933",
            "POLL_INTERVAL_SECONDS": "60",
            "POLL_TIMEOUT_SECONDS": "5400",
            "GITHUB_REPOSITORY": self.repository,
            "GITHUB_SERVER_URL": "https://github.com",
        }
        namespace: dict[str, object] = {
            "json": json,
            "os": os,
            "parse_qs": parse_qs,
            "re": re,
            "subprocess": subprocess,
            "sys": sys,
            "time": __import__("time"),
            "urlencode": __import__("urllib.parse", fromlist=["urlencode"]).urlencode,
            "urlsplit": __import__("urllib.parse", fromlist=["urlsplit"]).urlsplit,
        }
        with patch.dict(os.environ, environment, clear=False):
            exec(self.program, namespace)
        return namespace

    def source(self, nested_repo: object | None = None) -> dict[str, object]:
        repo = nested_repo if nested_repo is not None else {
            "id": 101,
            "name": "repository",
            "url": "https://api.github.com/repos/owner/repository",
        }
        return {
            "id": 17,
            "name": "PR governance review sensor",
            "event": "pull_request_review",
            "path": ".github/workflows/pr-governance-review-events.yml@master",
            "head_sha": self.head,
            "status": "in_progress",
            "conclusion": None,
            "run_attempt": 1,
            "run_number": 4,
            "repository": {"id": 101, "name": "repository", "url": self.repository_identity[2]},
            "pull_requests": [{
                "number": 72,
                "base": {"sha": self.base, "ref": "master", "repo": repo},
                "head": {"sha": self.head, "repo": repo},
            }],
        }

    def test_actions_minimal_nested_repository_is_bound_to_rest_identity(self) -> None:
        namespace = self.namespace()
        identity = namespace["repository_rest_identity"]
        matches = namespace["sensor_run_matches"]
        assert callable(identity) and callable(matches)
        self.assertEqual(
            identity({
                "id": 101,
                "name": "repository",
                "url": "https://api.github.com/repos/owner/repository",
                "full_name": self.repository,
            }),
            self.repository_identity,
        )
        self.assertEqual(
            identity({
                "id": 101,
                "name": "repository",
                "url": "https://api.github.com/repos/owner/repository",
            }),
            self.repository_identity,
        )
        self.assertTrue(matches(self.source(), self.repository_identity))

    def test_nested_repository_foreign_malformed_and_field_mismatches_fail_closed(self) -> None:
        namespace = self.namespace()
        matches = namespace["sensor_run_matches"]
        assert callable(matches)
        malformed = (
            None,
            [],
            {},
            {"id": True, "name": "repository", "url": self.repository_identity[2]},
            {"id": 202, "name": "repository", "url": self.repository_identity[2]},
            {"id": 101, "name": "other", "url": self.repository_identity[2]},
            {"id": 101, "name": "repository", "url": "https://api.github.com/repos/other/repository"},
            {"id": 101, "name": "repository", "url": self.repository_identity[2], "full_name": "other/repository"},
            {"id": 101, "name": "repository", "url": self.repository_identity[2], "full_name": 101},
        )
        for nested_repo in malformed:
            for side in ("base", "head"):
                with self.subTest(nested_repo=nested_repo, side=side):
                    candidate = json.loads(json.dumps(self.source()))
                    candidate["pull_requests"][0][side]["repo"] = nested_repo
                    self.assertFalse(matches(candidate, self.repository_identity))

    def test_top_level_rest_identity_and_run_identity_mismatches_fail_closed(self) -> None:
        namespace = self.namespace()
        identity = namespace["repository_rest_identity"]
        matches = namespace["sensor_run_matches"]
        assert callable(identity) and callable(matches)
        valid = {
            "id": 101,
            "name": "repository",
            "url": "https://api.github.com/repos/owner/repository",
            "full_name": self.repository,
        }
        for field, value in (
            ("id", True),
            ("name", "other"),
            ("name", None),
            ("url", "https://api.github.com/repos/other/repository"),
            ("url", None),
            ("full_name", "other/repository"),
            ("full_name", None),
        ):
            with self.subTest(field=field, value=value):
                candidate = dict(valid)
                candidate[field] = value
                self.assertIsNone(identity(candidate))
        for partial in (
            {"full_name": self.repository},
            {"id": 101, "name": "repository"},
            {"id": 101, "url": self.repository_identity[2]},
            {"id": 101, "name": "repository", "full_name": self.repository},
        ):
            with self.subTest(partial=partial):
                self.assertIsNone(identity(partial))
        run_repository = {"id": 101, "name": "repository", "url": self.repository_identity[2]}
        for changed in (
            {**run_repository, "id": 202},
            {**run_repository, "id": True},
            {**run_repository, "name": "other"},
            {**run_repository, "url": "https://api.github.com/repos/other/repository"},
            {**run_repository, "full_name": "other/repository"},
        ):
            with self.subTest(run_repository=changed):
                candidate = self.source()
                candidate["repository"] = changed
                self.assertFalse(matches(candidate, self.repository_identity))

    def test_writer_run_uses_rest_identity_without_requiring_full_name(self) -> None:
        namespace = self.namespace()
        matches = namespace["writer_run_matches"]
        assert callable(matches)
        writer = {
            "id": 91,
            "name": "PR governance status writer",
            "event": "workflow_dispatch",
            "path": ".github/workflows/pr-governance-status-writer.yml@master",
            "repository": {"id": 101, "name": "repository", "url": self.repository_identity[2]},
            "run_attempt": 1,
            "status": "in_progress",
        }
        self.assertTrue(matches(writer, "91", self.repository_identity))
        for changed in (
            {**writer, "repository": {**writer["repository"], "full_name": "other/repository"}},
            {**writer, "repository": {**writer["repository"], "id": 202}},
            {**writer, "repository": {**writer["repository"], "name": "other"}},
            {**writer, "repository": {**writer["repository"], "url": "https://api.github.com/repos/other/repository"}},
        ):
            with self.subTest(repository=changed["repository"]):
                self.assertFalse(matches(changed, "91", self.repository_identity))

    def test_source_bound_check_run_scan_finds_current_sensor_check_after_two_historical_pages(self) -> None:
        """A retained same-head history cannot hide the current source's Check Run."""
        namespace = self.namespace()
        reader = namespace["source_bound_trusted_check_runs"]
        assert callable(reader)
        candidate = {
            "id": 201,
            "name": "KRR / PR governance (trusted check)",
            "app": {"id": 4_766_933},
            "head_sha": self.head,
            "external_id": f"krr-governance/v1/{self.head}/writer-91",
            "details_url": "https://github.com/owner/repository/actions/runs/91?source_run_id=17",
            "status": "completed",
            "conclusion": "success",
        }
        pages = {
            1: [{"id": value} for value in range(1, 101)],
            2: [{"id": value} for value in range(101, 201)],
            3: [candidate],
        }
        calls: list[tuple[int, bool]] = []

        def fake_run(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
            endpoint = arguments[-1]
            page_match = re.search(r"[?&]page=(\d+)", endpoint)
            assert page_match is not None
            page = int(page_match.group(1))
            included = "--include" in arguments
            calls.append((page, included))
            self.assertIn(page, pages)
            payload = json.dumps({"total_count": 201, "check_runs": pages[page]})
            if included:
                self.assertEqual(page, 3)
                payload = "HTTP/2 200 OK\n\n" + payload
            return subprocess.CompletedProcess(arguments, 0, payload, "")

        with patch.object(subprocess, "run", side_effect=fake_run):
            self.assertIsNone(reader())
            result = reader()
        self.assertEqual(result, [candidate])
        self.assertEqual(calls, [(1, False), (2, False), (3, True), (1, False)])

    def test_source_bound_check_run_scan_keeps_an_empty_history_pending(self) -> None:
        """No Check Run yet is a valid pending state, not malformed pagination."""
        namespace = self.namespace()
        reader = namespace["source_bound_trusted_check_runs"]
        assert callable(reader)
        calls: list[bool] = []

        def fake_run(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
            self.assertTrue(arguments[-1].endswith("page=1"))
            included = "--include" in arguments
            calls.append(included)
            payload = json.dumps({"total_count": 0, "check_runs": []})
            if included:
                payload = "HTTP/2 200 OK\n\n" + payload
            return subprocess.CompletedProcess(arguments, 0, payload, "")

        with patch.object(subprocess, "run", side_effect=fake_run):
            self.assertIsNone(reader())
            self.assertEqual(reader(), [])
        self.assertEqual(calls, [False, True])

    def test_source_bound_check_run_scan_has_a_budget_derived_page_ceiling(self) -> None:
        """The 90-minute latch cannot turn retained history into an unbounded API scan."""
        namespace = self.namespace()
        reader = namespace["source_bound_trusted_check_runs"]
        page_limit = namespace["max_source_check_pages"]
        assert callable(reader)
        self.assertEqual(page_limit, 181)

        def fake_run(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
            self.assertTrue(arguments[-1].endswith("page=1"))
            payload = json.dumps({"total_count": page_limit * 100 + 1, "check_runs": [{"id": value} for value in range(1, 101)]})
            return subprocess.CompletedProcess(arguments, 0, payload, "")

        with patch.object(subprocess, "run", side_effect=fake_run), self.assertRaises(SystemExit):
            reader()

    def test_source_bound_check_run_scan_fails_closed_when_page_one_anchor_races(self) -> None:
        """A changed anchor clears the candidate and bounded scanning restarts."""
        namespace = self.namespace()
        reader = namespace["source_bound_trusted_check_runs"]
        assert callable(reader)
        candidate = {
            "id": 201,
            "name": "KRR / PR governance (trusted check)",
            "app": {"id": 4_766_933},
            "head_sha": self.head,
            "external_id": f"krr-governance/v1/{self.head}/writer-91",
            "details_url": "https://github.com/owner/repository/actions/runs/91?source_run_id=17",
        }
        first_page_reads = 0
        calls: list[tuple[int, bool]] = []

        def fake_run(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
            nonlocal first_page_reads
            endpoint = arguments[-1]
            page_match = re.search(r"[?&]page=(\d+)", endpoint)
            assert page_match is not None
            page = int(page_match.group(1))
            calls.append((page, "--include" in arguments))
            if page == 1:
                first_page_reads += 1
                runs = [{"id": value} for value in (range(1, 101) if first_page_reads == 1 else range(301, 401))]
            elif page == 2:
                runs = [{"id": value} for value in range(101, 201)]
            else:
                self.assertEqual(page, 3)
                runs = [candidate]
            payload = json.dumps({"total_count": 201, "check_runs": runs})
            if "--include" in arguments:
                self.assertEqual(page, 3)
                payload = "HTTP/2 200 OK\n\n" + payload
            return subprocess.CompletedProcess(arguments, 0, payload, "")

        with patch.object(subprocess, "run", side_effect=fake_run):
            self.assertIsNone(reader())
            self.assertIsNone(reader())
            result = reader()
        self.assertEqual(result, [candidate])
        self.assertEqual(first_page_reads, 3)
        self.assertEqual(
            calls,
            [(1, False), (2, False), (3, True), (1, False), (2, False), (3, True), (1, False)],
        )

    def test_source_bound_check_run_scan_rejects_ambiguous_raced_and_malformed_pages(self) -> None:
        """Pagination may wait for a stable source snapshot, never accept an ambiguous one."""
        candidate = {
            "id": 201,
            "name": "KRR / PR governance (trusted check)",
            "app": {"id": 4_766_933},
            "head_sha": self.head,
            "external_id": f"krr-governance/v1/{self.head}/writer-91",
            "details_url": "https://github.com/owner/repository/actions/runs/91?source_run_id=17",
        }

        for mode in ("ambiguous", "count-race", "duplicate", "malformed", "link-overflow"):
            with self.subTest(mode=mode):
                namespace = self.namespace()
                reader = namespace["source_bound_trusted_check_runs"]
                assert callable(reader)
                total = 202 if mode == "ambiguous" else 201
                pages: dict[int, object] = {
                    1: [{"id": value} for value in range(1, 101)],
                    2: [{"id": value} for value in range(101, 201)],
                    3: [candidate],
                }
                if mode == "ambiguous":
                    pages[3] = [candidate, {**candidate, "id": 202}]
                if mode == "duplicate":
                    pages[2] = [{"id": 100}, *[{"id": value} for value in range(101, 200)]]
                if mode == "malformed":
                    pages[2] = "not-a-list"

                def fake_run(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
                    endpoint = arguments[-1]
                    page_match = re.search(r"[?&]page=(\d+)", endpoint)
                    assert page_match is not None
                    page = int(page_match.group(1))
                    page_total = total
                    if mode == "count-race" and page == 2:
                        page_total -= 1
                    payload = json.dumps({"total_count": page_total, "check_runs": pages[page]})
                    if "--include" in arguments:
                        self.assertEqual(page, 3)
                        link = 'Link: <https://api.github.com/next>; rel="next"\n' if mode == "link-overflow" else ""
                        payload = f"HTTP/2 200 OK\n{link}\n" + payload
                    return subprocess.CompletedProcess(arguments, 0, payload, "")

                with patch.object(subprocess, "run", side_effect=fake_run):
                    if mode == "ambiguous":
                        self.assertIsNone(reader())
                    with self.assertRaises(SystemExit):
                        reader()


class GovernanceDispatcherContractTest(unittest.TestCase):
    def setUp(self) -> None:
        self.workflow = (ROOT / ".github/workflows/pr-governance.yml").read_text(encoding="utf-8")
        prior_started_at = os.environ.get("TERMINAL_SEGMENT_STARTED_AT")
        os.environ["TERMINAL_SEGMENT_STARTED_AT"] = str(int(time.time()))
        if prior_started_at is None:
            self.addCleanup(os.environ.pop, "TERMINAL_SEGMENT_STARTED_AT", None)
        else:
            self.addCleanup(os.environ.__setitem__, "TERMINAL_SEGMENT_STARTED_AT", prior_started_at)

    @staticmethod
    def _workflow_program(match: re.Match[str]) -> str:
        """Normalize extracted YAML Python and make polling deterministic in tests."""
        program = (
            textwrap.dedent(match.group(1))
        )
        # Keep production deadline arithmetic intact while advancing it on a
        # deterministic clock.  A no-op sleep would leave a timeout loop with
        # a real deadline and make the fixture depend on wall-clock timing.
        # Workflow snippets use both ``import time`` and combined imports
        # such as ``import json, os, ..., time``.  Remove precisely that
        # import binding and inject the deterministic clock once, so a
        # terminal deadline cannot accidentally keep reading wall time.
        def without_time_import(import_match: re.Match[str]) -> str:
            indentation, names = import_match.groups()
            modules = [name.strip() for name in names.split(",")]
            if "time" not in modules:
                return import_match.group(0)
            retained = [name for name in modules if name != "time"]
            return f"{indentation}import {', '.join(retained)}" if retained else ""

        # The standard unittest runner can resolve macOS Python 3.9, whose
        # ``zip`` has no ``strict=`` keyword.  The production snippets keep
        # their strict calls unchanged; only the extracted test program uses
        # an equivalent sentinel-based implementation, including the same
        # unequal-length failure, so this does not weaken the assertion.
        program = re.sub(
            r"(?m)^(\s*)import\s+([A-Za-z_][A-Za-z0-9_]*(?:\s*,\s*[A-Za-z_][A-Za-z0-9_]*)*)$",
            without_time_import,
            program,
        )
        program = re.sub(
            r"zip\(([^(),]+),([^(),]+),\s*strict=True\)",
            r"_krr_strict_zip(\1,\2)",
            program,
        )
        program = "time = _krr_clock\n" + program
        program = program.replace("time.sleep(", "_krr_sleep(")
        # Production pagination deliberately uses one bounded request per
        # page, rereads page one as an anchor, and uses ``--include`` on the
        # terminal page.  Most fixtures predate that contract and model the
        # former ``--paginate --slurp`` response as a list of pages.  Keep
        # those fixtures meaningful by adapting only their returned payload
        # at this test boundary; the extracted production program remains
        # unchanged and still exercises all pagination validation branches.
        compatibility = textwrap.dedent(
            r'''
            import json as _krr_json
            import re as _krr_re
            import subprocess
            import time as _krr_real_time
            from itertools import zip_longest as _krr_zip_longest

            class _KrrFakeClock:
                def __init__(self):
                    self._now = _krr_real_time.time()

                def time(self):
                    return self._now

                def monotonic(self):
                    return self._now

                def sleep(self, seconds):
                    if isinstance(seconds, (int, float)) and seconds >= 0:
                        self._now += seconds

            _krr_clock = _KrrFakeClock()

            def _krr_sleep(seconds):
                _krr_clock.sleep(seconds)

            def _krr_strict_zip(*iterables):
                sentinel = object()
                for values in _krr_zip_longest(*iterables, fillvalue=sentinel):
                    if any(value is sentinel for value in values):
                        raise ValueError("zip() argument lengths must be equal")
                    yield values

            _krr_underlying_run = subprocess.run

            def _krr_run(*args, **kwargs):
                argv = args[0] if args else kwargs.get("args", [])
                if isinstance(argv, (list, tuple)) and argv and argv[0] == "sleep":
                    _krr_clock.sleep(float(argv[1]))
                    return subprocess.CompletedProcess(argv, 0, "", "")
                result = _krr_underlying_run(*args, **kwargs)
                if not isinstance(argv, (list, tuple)) or result.returncode != 0:
                    return result
                endpoint = next((item for item in argv if isinstance(item, str) and item.startswith("repos/")), "")
                page_match = _krr_re.search(r"[?&]page=(\d+)", endpoint)
                page_number = int(page_match.group(1)) if page_match else 1
                is_terminal = "--include" in argv
                is_pull_page = "/pulls?state=open" in endpoint
                is_run_page = "/actions/workflows/" in endpoint and "/runs?" in endpoint
                is_check_page = "/check-runs?" in endpoint
                if not (is_pull_page or is_run_page or is_check_page) or not isinstance(result.stdout, str):
                    return result
                raw = result.stdout
                try:
                    payload = _krr_json.loads(raw)
                except (TypeError, _krr_json.JSONDecodeError):
                    return result
                if is_pull_page and isinstance(payload, list) and payload and all(isinstance(item, list) for item in payload):
                    payload = payload[page_number - 1] if page_number <= len(payload) else []
                elif is_run_page and isinstance(payload, list):
                    if payload and all(isinstance(item, dict) and "workflow_runs" in item for item in payload):
                        payload = payload[page_number - 1] if page_number <= len(payload) else {"total_count": 0, "workflow_runs": []}
                elif is_check_page and isinstance(payload, list):
                    if payload and all(isinstance(item, dict) and "check_runs" in item for item in payload):
                        payload = payload[page_number - 1] if page_number <= len(payload) else {"total_count": 0, "check_runs": []}
                if is_run_page and isinstance(payload, dict) and "workflow_runs" in payload and "total_count" not in payload:
                    payload = {**payload, "total_count": len(payload["workflow_runs"]) if isinstance(payload["workflow_runs"], list) else -1}
                if is_check_page and isinstance(payload, dict) and "check_runs" in payload and "total_count" not in payload:
                    payload = {**payload, "total_count": len(payload["check_runs"]) if isinstance(payload["check_runs"], list) else -1}
                normalized = _krr_json.dumps(payload)
                if is_terminal and not raw.startswith("HTTP/"):
                    normalized = "HTTP/2 200 OK\n\n" + normalized
                return subprocess.CompletedProcess(result.args, result.returncode, normalized, result.stderr)

            '''
        )
        return compatibility + program.replace("subprocess.run(", "_krr_run(")

    def test_empty_paginated_check_run_fixture_remains_fail_closed(self) -> None:
        """The pagination adapter may unwrap a real page, never an empty malformed response."""

        program_match = re.match(
            r"(?s)(.*)",
            """
            response = subprocess.run(["gh", "api", "repos/owner/repository/commits/a/check-runs?per_page=100&page=1"])
            value = _krr_json.loads(response.stdout)
            if not isinstance(value, dict) or not isinstance(value.get("check_runs"), list):
                raise SystemExit("malformed check-run response")
            """,
        )
        assert program_match is not None

        def fake_run(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
            return subprocess.CompletedProcess(arguments, 0, "[]", "")

        with patch.object(subprocess, "run", side_effect=fake_run):
            with self.assertRaises(SystemExit):
                exec(self._workflow_program(program_match), {})

    def _step_if(self, name: str) -> str:
        match = re.search(
            rf"^      - name: {re.escape(name)}\n(?P<body>.*?)(?=^      - name: |\Z)",
            self.workflow, re.MULTILINE | re.DOTALL,
        )
        self.assertIsNotNone(match, name); assert match is not None
        condition = re.search(r"^        if: (?P<value>.+)$", match.group("body"), re.MULTILINE)
        self.assertIsNotNone(condition, name); assert condition is not None
        return condition.group("value")

    @staticmethod
    def _github_if(expression: str, values: dict[str, str]) -> bool:
        """Evaluate the workflow condition using a strict, non-Python subset."""
        token_pattern = re.compile(
            r"(?P<space>\s+)|(?P<operand>steps\.[A-Za-z0-9_-]+\.(?:outputs\.[A-Za-z0-9_-]+|outcome))|"
            r"(?P<string>'[^'\\]*')|(?P<operator>==|!=|&&|\|\||[!()])"
        )
        tokens: list[tuple[str, str]] = []
        position = 0
        while position < len(expression):
            match = token_pattern.match(expression, position)
            if match is None:
                raise AssertionError(f"Invalid workflow if token at offset {position}")
            position = match.end()
            kind = match.lastgroup
            if kind != "space":
                assert kind is not None
                tokens.append((kind, match.group()))

        referenced = {value for kind, value in tokens if kind == "operand"}
        unknown = referenced - set(values)
        if unknown:
            raise AssertionError(f"Unbound workflow if value: {sorted(unknown)}")

        cursor = 0

        def peek() -> tuple[str, str] | None:
            return tokens[cursor] if cursor < len(tokens) else None

        def take(kind: str, value: str | None = None) -> str:
            nonlocal cursor
            token = peek()
            if token is None or token[0] != kind or (value is not None and token[1] != value):
                raise AssertionError("Invalid workflow if grammar")
            cursor += 1
            return token[1]

        def primary() -> str:
            token = peek()
            if token is not None and token[0] == "operand":
                return values[take("operand")]
            if token is not None and token[0] == "string":
                return take("string")[1:-1]
            raise AssertionError("Invalid workflow if operand")

        def comparison() -> bool:
            left = primary()
            token = peek()
            if token is None or token[0] != "operator" or token[1] not in {"==", "!="}:
                raise AssertionError("Workflow if comparison operator is required")
            operator = take("operator")
            right = primary()
            return left == right if operator == "==" else left != right

        def unary() -> bool:
            if peek() == ("operator", "!"):
                take("operator", "!")
                return not unary()
            if peek() == ("operator", "("):
                take("operator", "(")
                result = disjunction_with_unary()
                take("operator", ")")
                return result
            return comparison()

        # Unary negation binds tighter than conjunction.
        def conjunction_with_unary() -> bool:
            result = unary()
            while peek() == ("operator", "&&"):
                take("operator", "&&")
                result = unary() and result
            return result

        def disjunction_with_unary() -> bool:
            result = conjunction_with_unary()
            while peek() == ("operator", "||"):
                take("operator", "||")
                result = conjunction_with_unary() or result
            return result

        if not tokens:
            raise AssertionError("Empty workflow if expression")
        result = disjunction_with_unary()
        if cursor != len(tokens):
            raise AssertionError("Trailing workflow if tokens")
        return result

    def test_all_issue_mutations_and_schedule_reconcile_every_open_pr(self) -> None:
        actions = (
            "opened", "edited", "deleted", "transferred", "pinned", "unpinned", "closed", "reopened",
            "assigned", "unassigned", "labeled", "unlabeled", "locked", "unlocked", "milestoned",
            "demilestoned", "typed", "untyped", "field_added", "field_removed",
        )
        for action in actions:
            self.assertIn(action, self.workflow)
        self.assertIn("schedule:", self.workflow)
        self.assertIn("single arbiter removes the former 256-target matrix/rotation", self.workflow)
        terminal = self.workflow[self.workflow.index("id: dispatch-all-1"):]
        self.assertNotIn("--paginate", terminal)
        self.assertNotIn("--slurp", terminal)
        self.assertEqual(terminal.count("pulls?state=open&per_page=100&page={page_number}"), 4)

    def test_github_if_rejects_python_and_unbound_or_dangling_tokens(self) -> None:
        values = {"steps.check.outputs.ready": "true"}
        with self.assertRaises(AssertionError):
            self._github_if("().__class__.__mro__[1].__subclasses__()", values)
        with self.assertRaises(AssertionError):
            self._github_if("steps.unknown.outputs.ready == 'true'", values)
        with self.assertRaises(AssertionError):
            self._github_if("steps.check.outputs.ready ==", values)
        with self.assertRaises(AssertionError):
            self._github_if("steps.check.outputs.ready == \"true\"", values)

    def test_github_if_supports_only_the_workflow_boolean_subset(self) -> None:
        values = {
            "steps.check.outputs.ready": "true",
            "steps.check.outcome": "success",
        }
        self.assertTrue(self._github_if("(steps.check.outputs.ready == 'true') && ! (steps.check.outcome != 'success')", values))
        self.assertFalse(self._github_if("steps.check.outputs.ready != 'true' || steps.check.outcome != 'success'", values))

    def test_one_event_runs_one_arbiter_after_synchronous_invalidation(self) -> None:
        self.assertIn(
            "concurrency:\n      group: pr-governance-dispatcher-${{ github.repository_id }}\n      cancel-in-progress: ${{ needs.resolve_event.outputs.priority_targets != '[]' }}",
            self.workflow,
        )
        writer = (ROOT / ".github/workflows/pr-governance-status-writer.yml").read_text(encoding="utf-8")
        self.assertIn("group: pr-governance-status-${{ github.repository_id }}", writer)
        self.assertNotIn("group: pr-governance-status-${{ github.repository_id }}", self.workflow)
        self.assertIn("cancel-in-progress: ${{ inputs.scope == 'early' }}", writer)
        self.assertNotIn("Legacy dispatcher early invalidator", self.workflow)
        self.assertIn("Invalidate every current pull request for the all-open writer", self.workflow)
        self.assertIn("status=in_progress", self.workflow)
        self.assertEqual(self.workflow.count("actions/workflows/pr-governance-status-writer.yml/dispatches"), 5)
        self.assertIn("permission-actions: write", self.workflow)
        self.assertIn("permission-checks: write", self.workflow)
        self.assertIn("KRR_GOVERNANCE_APP_BOT_LOGIN", writer)
        self.assertIn("github.triggering_actor == vars.KRR_GOVERNANCE_APP_BOT_LOGIN", writer)

    def test_priority_event_preempts_a_long_reconciliation_and_writer_rebinds_before_secrets(self) -> None:
        self.assertIn(
            "cancel-in-progress: ${{ needs.resolve_event.outputs.priority_targets != '[]' }}",
            self.workflow,
        )
        self.assertIn("PRs may edit a workflow file", self.workflow)
        self.assertIn("Check Run fingerprint fence", self.workflow)
        writer = (ROOT / ".github/workflows/pr-governance-status-writer.yml").read_text(encoding="utf-8")
        writer_program = (ROOT / "scripts/review/pr_governance_status_writer.py").read_text(encoding="utf-8")
        # A queued dispatcher is a generation fence even before its pending
        # Check Run is visible. The writer must inspect the immutable
        # dispatcher-run snapshot before every terminal mutation.
        self.assertIn("def reject_newer_dispatcher_barrier", writer_program)
        self.assertIn("dispatcher_generations(\n            current_generation.workflow_id, current_generation.created_at,", writer_program)
        self.assertIn("Read bounded, exact-workflow generations no older than the source.", writer_program)
        self.assertIn("per_page=100", writer_program)
        self.assertIn("A newer dispatcher generation owns this Check Run head.", writer_program)
        self.assertGreaterEqual(writer_program.count("reject_newer_dispatcher_barrier(head)"), 2)
        self.assertIn("group: pr-governance-status-${{ github.repository_id }}", writer)
        self.assertIn("cancel-in-progress: ${{ inputs.scope == 'early' }}", writer)
        rebind = writer.index("Rebind trusted default branch before token creation")
        check_write_token = writer.index("Create Check Run writer App token")
        self.assertLess(rebind, check_write_token)
        self.assertIn("Trusted default branch advanced while writer was queued.", writer)

    def test_release_generation_fence_excludes_only_a_verified_preflight_noop(self) -> None:
        match = re.search(
            r"- name: Release complete affected-head merge barrier only after full pending coverage.*?"
            r"python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        release = self._workflow_program(match)
        self.assertIn("def reconciles(run):", release)
        self.assertIn('run.get("event") not in {"workflow_run","pull_request_target","issues","issue_comment"}', release)
        self.assertIn('run.get("status")!="completed"', release)
        self.assertIn('run.get("conclusion")!="success"', release)
        self.assertIn('actions/runs/{identifier}/jobs?per_page=100', release)
        self.assertIn('total!=len(entries) or total>=100', release)
        self.assertIn('named_job("Preflight workflow_run governance source")', release)
        self.assertIn('named_job("Establish resolver-failure merge barrier")', release)
        self.assertIn('barrier.get("conclusion")=="skipped"', release)
        self.assertIn('Record verified pull_request_target preflight no-op', release)
        self.assertIn('Record verified Issue preflight no-op', release)
        self.assertIn(
            "candidate>current_generation and reconciles(run)", release,
        )
        self.assertIn("str(posted)!=app_id", self.workflow)

        preflight = self.workflow[
            self.workflow.index("  preflight-workflow-run-source:"):
            self.workflow.index("  establish-resolver-failure-barrier:")
        ]
        self.assertIn("name: Preflight workflow_run governance source", preflight)
        self.assertIn("pull_request_target_noop: ${{ steps.scope.outputs.pull_request_target_noop }}", preflight)
        self.assertIn('output.write("pull_request_target_noop="', preflight)
        self.assertIn("- name: Record verified pull_request_target preflight no-op", preflight)

    def test_preflight_skips_only_stable_nondefault_or_unclaimed_issue_events(self) -> None:
        """No-op classification happens before the shared dispatcher lock."""
        scope_match = re.search(
            r"- name: Exclude unavailable fork sources before dispatcher lock.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(scope_match); assert scope_match is not None
        scope = self._workflow_program(scope_match)
        repository = "owner/repository"
        base = "b" * 40
        head = "a" * 40

        def pull(base_ref: str = "release/v1", body: str = "Fixes #64") -> dict[str, object]:
            return {
                "number": 72, "state": "open",
                "draft": False,
                "base": {"ref": base_ref, "sha": base, "repo": {"id": 101, "full_name": repository}},
                "head": {"sha": head, "repo": {"id": 101, "full_name": repository}},
                "body": body,
            }

        def execute(
            event: dict[str, str], pages: list[list[dict[str, object]]] | tuple[list[list[dict[str, object]]], list[list[dict[str, object]]]],
        ) -> dict[str, str]:
            with tempfile.TemporaryDirectory() as temporary:
                output = Path(temporary) / "scope-output"
                pull_reads = 0
                repository_reads = 0
                page_reads = 0

                def response(value: object) -> subprocess.CompletedProcess[str]:
                    return subprocess.CompletedProcess([], 0, json.dumps(value), "")

                def fake_run(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
                    nonlocal pull_reads, repository_reads, page_reads
                    endpoint = arguments[-1]
                    if endpoint == f"repos/{repository}":
                        repository_reads += 1
                        return response({"id": 101, "full_name": repository, "default_branch": "master"})
                    if endpoint.startswith(f"repos/{repository}/pulls/"):
                        pull_reads += 1
                        number = int(endpoint.rsplit("/", 1)[1])
                        return response({**pull(), "number": number})
                    if endpoint.startswith(f"repos/{repository}/pulls?state=open&per_page=100"):
                        if isinstance(pages, tuple):
                            value = pages[min(page_reads, len(pages) - 1)]
                        else:
                            page_number = int(parse_qs(urlparse(endpoint).query).get("page", ["1"])[0])
                            value = pages[page_number - 1] if page_number <= len(pages) else []
                        page_reads += 1
                        return response(value)
                    raise AssertionError(arguments)

                environment = os.environ | {
                    "GITHUB_REPOSITORY": repository, "GITHUB_OUTPUT": str(output),
                    "PR_NUMBER": "72", "PR_HEAD_SHA": head, "PR_BASE_SHA": base,
                    "PR_BASE_REF": "release/v1", "PR_ACTION": "edited",
                    "PR_PREVIOUS_BASE_REF": "", "ISSUE_NUMBER": "999",
                    "ISSUE_PULL_REQUEST_URL": "", **event,
                }
                with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run):
                    exec(scope, {"__name__": "__main__"})
                values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
                if event["EVENT_NAME"] == "pull_request_target" and values["reconcile"] == "false":
                    self.assertEqual(pull_reads, 2)
                    self.assertEqual(repository_reads, 2)
                if event["EVENT_NAME"] == "issues" and values["reconcile"] == "false":
                    self.assertEqual(page_reads, 4)
                return values

        unrelated = [[pull(body="Fixes #64")]]
        governed = [[pull(base_ref="master", body="Fixes #64")]]
        self.assertEqual(
            execute({"EVENT_NAME": "pull_request_target", "EVENT_ACTION": "edited"}, unrelated)["reconcile"],
            "false",
        )
        self.assertEqual(
            execute({"EVENT_NAME": "pull_request_target", "EVENT_ACTION": "edited", "PR_PREVIOUS_BASE_REF": "master"}, unrelated)["reconcile"],
            "true",
        )
        self.assertEqual(
            execute({"EVENT_NAME": "pull_request_target", "EVENT_ACTION": "edited", "PR_PREVIOUS_BASE_REF": "release/v0"}, unrelated)["reconcile"],
            "false",
        )
        self.assertEqual(
            execute({"EVENT_NAME": "issues", "EVENT_ACTION": "edited"}, unrelated)["reconcile"],
            "false",
        )
        self.assertEqual(
            execute({"EVENT_NAME": "issue_comment", "EVENT_ACTION": "created", "ISSUE_PULL_REQUEST_URL": "https://api.github.com/repos/owner/repository/pulls/999"}, unrelated)["reconcile"],
            "false",
        )
        self.assertEqual(
            execute({"EVENT_NAME": "issues", "EVENT_ACTION": "edited", "ISSUE_NUMBER": "64"}, governed)["reconcile"],
            "true",
        )
        self.assertEqual(
            execute(
                {"EVENT_NAME": "issues", "EVENT_ACTION": "edited"},
                (unrelated, [[pull(base_ref="master", body="Fixes #999")]]),
            )["reconcile"],
            "true",
        )
        self.assertEqual(
            execute(
                {"EVENT_NAME": "issues", "EVENT_ACTION": "edited"},
                ([[pull(base_ref="master", body="Fixes #64")]], []),
            )["reconcile"],
            "true",
        )
        malformed = pull(base_ref="master", body="Fixes #64")
        malformed["head"] = {"sha": head, "repo": {"full_name": repository}}
        values = execute(
            {"EVENT_NAME": "issues", "EVENT_ACTION": "edited"},
            ([[pull(base_ref="master", body="Fixes #64")]], [[malformed]]),
        )
        self.assertEqual(values["reconcile"], "true")
        self.assertEqual(values["valid"], "false")

        for head_repository in (
            {"id": 101, "full_name": "fork/repository"},
            {"id": 202, "full_name": repository},
        ):
            with self.subTest(head_repository=head_repository):
                ambiguous_head = pull(base_ref="master", body="Fixes #64")
                ambiguous_head["head"] = {"sha": head, "repo": head_repository}
                values = execute(
                    {"EVENT_NAME": "issues", "EVENT_ACTION": "edited"},
                    ([[pull(base_ref="master", body="Fixes #64")]], [[ambiguous_head]]),
                )
                self.assertEqual(values["reconcile"], "true")
                self.assertEqual(values["valid"], "false")
        body_none = pull(base_ref="master", body="Fixes #64")
        body_none["body"] = None
        self.assertEqual(
            execute(
                {"EVENT_NAME": "issues", "EVENT_ACTION": "edited"},
                ([[body_none]], [[pull(base_ref="master", body="")]]),
            )["reconcile"],
            "true",
        )
        invalid_base = pull(base_ref="master", body="Fixes #64")
        invalid_base["base"] = {"ref": "master", "sha": base, "repo": {"id": 102, "full_name": "owner/other"}}
        values = execute(
            {"EVENT_NAME": "issues", "EVENT_ACTION": "edited"},
            ([[pull(base_ref="master", body="Fixes #64")]], [[invalid_base]]),
        )
        self.assertEqual(values["reconcile"], "true")
        self.assertEqual(values["valid"], "false")
        for body in (
            "Fixes https://github.com/owner/repository/issues/64",
            "FIXES https://github.com/OWNER/REPOSITORY/issues/64)",
            "Fixes https://github.com/owner/repository/issues/64/",
            "Fixes https://github.com/owner/repository/issues/64?source=pr",
            "Fixes https://github.com/owner/repository/issues/64#fragment",
        ):
            with self.subTest(body=body):
                expected = 64 in canonical_issue_contract.closing_issue_numbers(body, repository)
                values = execute(
                    {"EVENT_NAME": "issues", "EVENT_ACTION": "edited", "ISSUE_NUMBER": "64"},
                    [[pull(base_ref="master", body=body)]],
                )
                self.assertEqual(values["reconcile"], "true" if expected else "false")

    def test_issue_comment_preflight_rejects_unstable_identity_and_skips_stable_out_of_scope_prs(self) -> None:
        """Issue comments use the PR URL identity and fail closed on races."""
        scope_match = re.search(r"- name: Exclude unavailable fork sources before dispatcher lock.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(scope_match); assert scope_match is not None
        scope = self._workflow_program(scope_match); repository = "owner/repository"
        identity = {"id": 101, "full_name": repository}
        def pr(base_ref: str = "master", state: str = "open", head_repo: object = identity, number: int = 999) -> dict[str, object]:
            return {"number": number, "state": state, "base": {"ref": base_ref, "sha": "b" * 40, "repo": identity}, "head": {"sha": "a" * 40, "repo": head_repo}}
        def execute(values: list[dict[str, object]], event_url: str) -> dict[str, str]:
            with tempfile.TemporaryDirectory() as temporary:
                output = Path(temporary) / "scope-output"; reads = 0
                def response(value: object) -> subprocess.CompletedProcess[str]: return subprocess.CompletedProcess([], 0, json.dumps(value), "")
                def fake_run(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
                    nonlocal reads; endpoint = arguments[-1]
                    if endpoint == f"repos/{repository}": return response({"id": 101, "full_name": repository, "default_branch": "master"})
                    if endpoint.startswith(f"repos/{repository}/pulls/"):
                        value = values[min(reads, len(values) - 1)]; reads += 1; return response(value)
                    raise AssertionError(arguments)
                env = os.environ | {"GITHUB_REPOSITORY": repository, "GITHUB_OUTPUT": str(output), "EVENT_NAME": "issue_comment", "EVENT_ACTION": "created", "ISSUE_NUMBER": "999", "ISSUE_PULL_REQUEST_URL": event_url}
                with patch.dict(os.environ, env, clear=True), patch("subprocess.run", side_effect=fake_run): exec(scope, {"__name__": "__main__"})
                return dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
        url = f"https://api.github.com/repos/{repository}/pulls/999"
        mismatched = execute([pr(number=72), pr(number=72)], f"https://api.github.com/repos/{repository}/pulls/72",)
        self.assertEqual(mismatched["reconcile"], "true")
        self.assertEqual(mismatched["valid"], "false")
        for label, source, expected in (("closed", pr(state="closed"), "false"), ("fork", pr(head_repo={"id": 202, "full_name": "fork/repository"}), "false"), ("non-default", pr(base_ref="release/v1"), "false"), ("local-default-open", pr(), "true")):
            with self.subTest(label=label):
                result = execute([source, source], url)
                self.assertEqual(result["reconcile"], expected)
                if expected == "false":
                    self.assertEqual(result["valid"], "true")
                    self.assertEqual(result["issue_event_noop"], "true")
        malformed = pr(); malformed["head"] = {"sha": "a" * 40, "repo": {"id": 101}}
        result = execute([malformed, malformed], url); self.assertEqual(result["reconcile"], "true"); self.assertEqual(result["valid"], "false")
        changed = pr(); changed["head"] = {"sha": "c" * 40, "repo": identity}
        self.assertEqual(execute([pr(), changed], url)["reconcile"], "true")

    def test_release_generation_fence_requires_explicit_pull_request_target_noop_step(self) -> None:
        match = re.search(
            r"- name: Release complete affected-head merge barrier only after full pending coverage.*?"
            r"python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        release = self._workflow_program(match)
        function = re.search(r"^def reconciles\(run\):\n(?P<body>.*?)(?=^observed=)", release, re.MULTILINE | re.DOTALL)
        self.assertIsNotNone(function); assert function is not None
        runs = {"jobs": {}}

        def request(arguments: list[str], *, env: dict[str, str]) -> dict[str, object]:
            self.assertEqual(env, {"GH_TOKEN": "read"})
            self.assertEqual(arguments, ["repos/owner/repository/actions/runs/100/jobs?per_page=100"])
            return runs["jobs"]

        namespace = {"repository": "owner/repository", "request": request, "read_env": {"GH_TOKEN": "read"}}
        exec("def reconciles(run):\n" + function.group("body"), namespace)
        candidate = {"id": 100, "event": "pull_request_target", "status": "completed", "conclusion": "success"}
        base_jobs = [
            {"id": 1, "name": "Preflight workflow_run governance source", "status": "completed", "conclusion": "success"},
            {"id": 2, "name": "Establish resolver-failure merge barrier", "status": "completed", "conclusion": "skipped"},
        ]
        cases = (
            ("verified", [{**base_jobs[0], "steps": [{"number": 1, "name": "Record verified pull_request_target preflight no-op", "status": "completed", "conclusion": "success"}]}, base_jobs[1]], False),
            ("missing", base_jobs, True),
            ("skipped", [{**base_jobs[0], "steps": [{"number": 1, "name": "Record verified pull_request_target preflight no-op", "status": "completed", "conclusion": "skipped"}]}, base_jobs[1]], True),
            ("failure", [{**base_jobs[0], "steps": [{"number": 1, "name": "Record verified pull_request_target preflight no-op", "status": "completed", "conclusion": "failure"}]}, base_jobs[1]], True),
            ("duplicate", [{**base_jobs[0], "steps": [
                {"number": 1, "name": "Record verified pull_request_target preflight no-op", "status": "completed", "conclusion": "success"},
                {"number": 2, "name": "Record verified pull_request_target preflight no-op", "status": "completed", "conclusion": "success"},
            ]}, base_jobs[1]], True),
            ("malformed", [{**base_jobs[0], "steps": [{"number": True, "name": "Record verified pull_request_target preflight no-op", "status": "completed", "conclusion": "success"}]}, base_jobs[1]], True),
        )
        for label, jobs, expected in cases:
            with self.subTest(evidence=label):
                runs["jobs"] = {"total_count": 2, "jobs": jobs}
                self.assertEqual(namespace["reconciles"](candidate), expected)

        issue_candidate = {**candidate, "event": "issues"}
        issue_step = {"number": 1, "name": "Record verified Issue preflight no-op", "status": "completed", "conclusion": "success"}
        for label, steps, expected in (
            ("verified-issue", [issue_step], False),
            ("missing-issue", [], True),
            ("failed-issue", [{**issue_step, "conclusion": "failure"}], True),
        ):
            with self.subTest(evidence=label):
                runs["jobs"] = {"total_count": 2, "jobs": [{**base_jobs[0], "steps": steps}, base_jobs[1]]}
                self.assertEqual(namespace["reconciles"](issue_candidate), expected)

    def test_workflow_run_source_is_strict_before_app_tokens_exist(self) -> None:
        validation = self.workflow[:self.workflow.index("- name: Create dispatcher App token")]
        for text in (
            '"PR governance review sensor"', '"CI"', '"release-preflight"',
            '".github/workflows/test-and-build.yml"', '".github/workflows/release-preflight.yml"',
            'run.get("path")', 'run.get("run_attempt")', 'len(pulls) == 1',
            'workflow_run current default source drifted.',
        ):
            self.assertIn(text, validation)

    def test_workflow_run_accepts_github_at_ref_path_and_rejects_prefix_or_traversal(self) -> None:
        match = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(match); assert match is not None
        base, head = "b" * 40, "a" * 40
        for path, expected in ((".github/workflows/test-and-build.yml@main", 0), (".github/workflows/test-and-build.yml@refs/pull/72/merge", 0), (".github/workflows/test-and-build.yml.evil@main", 1), (".github/workflows/test-and-build.yml@../main", 1)):
            with self.subTest(path=path), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
                source_repo = {"id": 101, "name": "repository", "url": "https://api.github.com/repos/owner/repository"}
                pull = {"number": 72, "state": "open", "base": {"sha": base, "ref": "master", "repo": {"id": 101, "full_name": "owner/repository"}}, "head": {"sha": head, "repo": {"id": 101, "full_name": "owner/repository"}}}
                run = {"name": "CI", "path": path, "event": "pull_request", "status": "completed", "id": 9, "run_number": 1, "run_attempt": 1, "head_sha": head, "repository": {"id": 101, "full_name": "owner/repository"}, "pull_requests": [{"number": 72, "base": {"sha": base, "ref": "master", "repo": source_repo}, "head": {"sha": head, "repo": source_repo}}]}
                fake.write_text(
                    "#!/bin/sh\necho \"$*\" >> \"${CALL_LOG}\"\ncase \"$*\" in\n"
                    "  *'check-runs/101'*) printf '%s' '{\"id\":101,\"app\":{\"id\":42},\"name\":\"KRR / PR governance (trusted check)\",\"head_sha\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"external_id\":\"krr-governance/v1/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/dispatcher-9\",\"status\":\"in_progress\",\"conclusion\":null,\"details_url\":\"https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=0\"}' ;;\n"
                    "  *'/actions/runs/9'*) printf '%s' \"${RUN}\" ;;\n"
                    "  *'/pulls/72'*) printf '%s' \"${PULL}\" ;;\n"
                    "  *'git/ref/heads/master'*) printf '%s' '{\"object\":{\"sha\":\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"}}' ;;\n"
                    "  *'/compare/'*) printf '%s' '{\"status\":\"identical\",\"base_commit\":{\"sha\":\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"},\"merge_base_commit\":{\"sha\":\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"},\"head_commit\":{\"sha\":\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"}}' ;;\n"
                    "  *'/contents/'*) printf '%s' '{\"sha\":\"cccccccccccccccccccccccccccccccccccccccc\"}' ;;\n"
                    "  *'pulls?state=open'*) printf '%s' '[]' ;;\n"
                    "  *'repos/owner/repository'*) printf '%s' '{\"default_branch\":\"master\"}' ;;\n"
                    "  *) exit 91 ;;\nesac\n", encoding="utf-8",
                ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                environment = os.environ | {"EVENT_NAME": "workflow_run", "EVENT_ACTION": "completed", "WORKFLOW_RUN_ID": "9", "WORKFLOW_RUN_ATTEMPT": "1", "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output), "RUN": json.dumps(run), "PULL": json.dumps(pull), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}"}
                result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, expected, result.stderr)

    def test_requested_waiting_and_pending_workflow_run_statuses_reach_invalidation_path(self) -> None:
        match = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(match); assert match is not None
        current = re.search(
            r"- name: Re-enumerate every current local governance pull request.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        invalidator = re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(current); self.assertIsNotNone(invalidator)
        assert current is not None and invalidator is not None
        base, head = "b" * 40, "a" * 40
        pull = {
            "number": 72, "state": "open", "body": "Fixes #64", "draft": False,
            "base": {"sha": base, "ref": "master", "repo": {"id": 101, "full_name": "owner/repository"}},
            "head": {"sha": head, "repo": {"id": 101, "full_name": "owner/repository"}},
        }
        pulls = [[pull]]
        for status, current_base_sha in (("requested", base), ("waiting", base), ("pending", "d" * 40)):
            with self.subTest(status=status, current_base_sha=current_base_sha), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
                source_repository = {"id": 101, "name": "repository", "url": "https://api.github.com/repos/owner/repository"}
                run = {"name": "CI", "path": ".github/workflows/test-and-build.yml@main", "event": "pull_request", "status": status, "id": 9, "run_number": 1, "run_attempt": 1, "head_sha": head, "repository": {"id": 101, "full_name": "owner/repository"}, "pull_requests": [{"number": 72, "base": {"sha": base, "ref": "master", "repo": source_repository}, "head": {"sha": head, "repo": source_repository}}]}
                fake.write_text(
                    "#!/bin/sh\necho \"$*\" >> \"${CALL_LOG}\"\ncase \"$*\" in\n"
                    "  *'/actions/runs/9'*) printf '%s' \"${RUN}\" ;;\n"
                    "  *'git/ref/heads/master'*) printf '%s' \"${REF}\" ;;\n"
                    "  *'/compare/'*) printf '%s' \"${COMPARE}\" ;;\n"
                    "  *'/contents/'*) printf '%s' '{\"sha\":\"cccccccccccccccccccccccccccccccccccccccc\"}' ;;\n"
                    "  *'/pulls/72'*) printf '%s' \"${PULL}\" ;;\n"
                    "  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n"
                    "  *'repos/owner/repository'*) printf '%s' '{\"default_branch\":\"master\"}' ;;\n"
                    "  *) exit 91 ;;\nesac\n", encoding="utf-8",
                ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                current_pull = {**pull, "base": {**pull["base"], "sha": current_base_sha}}
                environment = os.environ | {"EVENT_NAME": "workflow_run", "EVENT_ACTION": "completed", "WORKFLOW_RUN_ID": "9", "WORKFLOW_RUN_ATTEMPT": "1", "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output), "RUN": json.dumps(run), "PULL": json.dumps(current_pull), "PULLS": json.dumps(pulls), "REF": json.dumps({"object": {"sha": current_base_sha}}), "COMPARE": json.dumps({"status": "identical" if current_base_sha == base else "ahead", "base_commit": {"sha": base}, "merge_base_commit": {"sha": base}, "head_commit": {"sha": current_base_sha}}), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}"}
                result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, 0, result.stderr)
                resolved = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
                self.assertEqual(resolved["reconcile"], "true")
                self.assertEqual(resolved["event_targets"], "[72]")
                self.assertEqual(resolved["priority_targets"], "[]")

                selection_output = directory / "selection-output"
                selected = subprocess.run(
                    [sys.executable, "-c", self._workflow_program(current)],
                    env=environment | {
                        "GITHUB_OUTPUT": str(selection_output),
                        "EVENT_TARGETS": resolved["event_targets"],
                        "EVENT_PRIORITY_TARGETS": resolved["priority_targets"],
                    }, capture_output=True, text=True, check=False,
                )
                self.assertEqual(selected.returncode, 0, selected.stderr)
                selection = dict(line.split("=", 1) for line in selection_output.read_text(encoding="utf-8").splitlines())
                self.assertEqual(selection["preinvalidate_targets"], "[]")
                self.assertEqual(selection["all_invalidation_targets"], "[72]")
                self.assertEqual(selection["has_duplicate_governed_heads"], "false")

                posts: list[list[str]] = []
                created: dict[int, dict[str, object]] = {}
                def response(value: object, code: int = 0) -> subprocess.CompletedProcess[str]:
                    return subprocess.CompletedProcess([], code, json.dumps(value), "")
                def fake_run(arguments: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                    endpoint = arguments[-1]
                    if "--method" in arguments and "POST" in arguments:
                        self.assertEqual(kwargs.get("env"), {"GH_TOKEN": "write", "PATH": os.environ["PATH"]})
                        posts.append(arguments)
                        fields = {field.split("=", 1)[0]: field.split("=", 1)[1] for field in arguments if "=" in field}
                        identifier = 500 + len(posts)
                        check = {
                            "id": identifier, "app": {"id": 42}, "name": "KRR / PR governance (trusted check)",
                            "head_sha": fields["head_sha"], "external_id": fields["external_id"],
                            "status": "in_progress", "conclusion": None, "details_url": fields["details_url"],
                        }
                        created[identifier] = check
                        return response(check)
                    if isinstance(endpoint, str) and "/commits/" in endpoint and "check-runs?" in endpoint:
                        return response([{"check_runs": []}])
                    if isinstance(endpoint, str) and "/check-runs/" in endpoint:
                        return response(created[int(endpoint.rsplit("/", 1)[1])])
                    if isinstance(endpoint, str) and endpoint.endswith("/pulls/72"):
                        return response(pull)
                    raise AssertionError(arguments)
                invalidation_output = directory / "invalidation-output"
                invalidation_environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "9",
                    "GH_TOKEN": "read", "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42",
                    "AFFECTED": selection["all_invalidation_targets"],
                    "KNOWN_TARGET_SNAPSHOTS": selection["all_invalidation_target_snapshots"],
                    "EVENT_TARGETS": resolved["event_targets"], "DUPLICATE_GOVERNED_HEADS": "[]",
                    "GITHUB_OUTPUT": str(invalidation_output), "PATH": os.environ["PATH"],
                }
                with patch.dict(os.environ, invalidation_environment, clear=True), patch("subprocess.run", side_effect=fake_run), patch("time.sleep"):
                    exec(self._workflow_program(invalidator), {"__name__": "__main__"})
                self.assertEqual(len(posts), 1)
                manifest = dict(line.split("=", 1) for line in invalidation_output.read_text(encoding="utf-8").splitlines())
                self.assertEqual(json.loads(manifest["check_manifest"]), [[72, 501]])

    def test_workflow_run_prioritizes_review_sensor_but_not_ci_or_release(self) -> None:
        match = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(match); assert match is not None
        base, head = "b" * 40, "a" * 40
        local = {"base": {"ref": "master", "repo": {"id": 101, "full_name": "owner/repository"}}, "head": {"repo": {"id": 101, "full_name": "owner/repository"}}}
        pulls = [[
            {"number": 72, "state": "open", "body": "Fixes #64", **local},
            {"number": 73, "state": "open", "body": "Closes #64", **local},
            {"number": 74, "state": "open", "body": "Fixes #64", "base": local["base"], "head": {"repo": None}},
        ]]
        cases = (
            ("CI", ".github/workflows/test-and-build.yml@master", "pull_request", "[]"),
            ("release-preflight", ".github/workflows/release-preflight.yml@master", "pull_request", "[]"),
            ("PR governance review sensor", ".github/workflows/pr-governance-review-events.yml@master", "pull_request", "[72,73]"),
            ("PR governance review sensor", ".github/workflows/pr-governance-review-events.yml@master", "pull_request_review", "[72,73]"),
        )
        for name, path, event, expected_priority in cases:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
                source_repo = {"id": 101, "name": "repository", "url": "https://api.github.com/repos/owner/repository"}
                current_pull = {"number": 72, "state": "open", "base": {"sha": base, **local["base"]}, "head": {"sha": head, **local["head"]}}
                run = {
                    "name": name, "path": path, "event": event, "status": "completed",
                    "id": 9, "run_number": 1, "run_attempt": 1, "head_sha": head,
                    "repository": {"id": 101, "full_name": "owner/repository"},
                    "pull_requests": [{"number": 72, "base": {"sha": base, "ref": "master", "repo": source_repo}, "head": {"sha": head, "repo": source_repo}}],
                }
                fake.write_text(
                    "#!/bin/sh\ncase \"$*\" in\n"
                    "  *'repos/owner/repository') printf '%s' '{\"id\":101,\"full_name\":\"owner/repository\",\"default_branch\":\"master\"}' ;;\n"
                    "  *'/actions/runs/9'*) printf '%s' \"${RUN}\" ;;\n"
                    "  *'/pulls/72'*) printf '%s' \"${PULL}\" ;;\n"
                    "  *'git/ref/heads/master'*) printf '%s' '{\"object\":{\"sha\":\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"}}' ;;\n"
                    "  *'/compare/'*) printf '%s' '{\"status\":\"identical\",\"base_commit\":{\"sha\":\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"},\"merge_base_commit\":{\"sha\":\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"},\"head_commit\":{\"sha\":\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"}}' ;;\n"
                    "  *'/contents/'*) printf '%s' '{\"sha\":\"cccccccccccccccccccccccccccccccccccccccc\"}' ;;\n"
                    "  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n"
                    "  *'repos/owner/repository'*) printf '%s' '{\"default_branch\":\"master\"}' ;;\n"
                    "  *) exit 91 ;;\nesac\n",
                    encoding="utf-8",
                )
                fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                environment = os.environ | {
                    "EVENT_NAME": "workflow_run", "EVENT_ACTION": "completed", "WORKFLOW_RUN_ID": "9", "WORKFLOW_RUN_ATTEMPT": "1",
                    "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master",
                    "GITHUB_OUTPUT": str(output), "RUN": json.dumps(run), "PULL": json.dumps(current_pull), "PULLS": json.dumps(pulls),
                    "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, 0, result.stderr)
                values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
                self.assertEqual(values["event_targets"], "[72,73]")
                self.assertEqual(values["priority_targets"], expected_priority)

    def test_workflow_run_current_default_binding_matches_preflight_contract(self) -> None:
        """Resolver revalidates the same B<H<T source boundary after the lock."""
        match = re.search(
            r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        base, head, tip = "b" * 40, "a" * 40, "d" * 40
        source_repository = {"id": 101, "name": "repository", "url": "https://api.github.com/repos/owner/repository"}
        run = {
            "name": "CI", "path": ".github/workflows/test-and-build.yml@master", "event": "pull_request",
            "status": "completed", "id": 9, "run_number": 1, "run_attempt": 1, "head_sha": head,
            "repository": {"id": 101, "full_name": "owner/repository"},
            "pull_requests": [{"number": 72, "base": {"sha": base, "ref": "master", "repo": source_repository}, "head": {"sha": head, "repo": source_repository}}],
        }
        local_pull = {
            "number": 72, "state": "open",
            "base": {"sha": tip, "ref": "master", "repo": {"id": 101, "full_name": "owner/repository"}},
            "head": {"sha": head, "repo": {"id": 101, "full_name": "owner/repository"}},
        }

        def execute(
            label: str, pull: dict[str, object], *, comparison: dict[str, object] | None = None,
            tip_blob: str = "c" * 40, final_tip: str | None = None,
            source_run: dict[str, object] | None = None, source_attempt: str = "1",
        ) -> subprocess.CompletedProcess[str]:
            with tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); output = directory / "output"; fake = directory / "gh"
                fake.write_text(
                    "#!/bin/sh\necho \"$*\" >> \"${CALL_LOG}\"\ncase \"$*\" in\n"
                    "  *'/actions/runs/9'*) printf '%s' \"${RUN}\" ;;\n"
                    "  *'/pulls/72'*) printf '%s' \"${PULL}\" ;;\n"
                    "  *'pulls?state=open'*) printf '%s' '[]' ;;\n"
                    "  *'git/ref/heads/master'*) if [ -e \"${REF_STATE}\" ]; then printf '%s' \"${FINAL_REF}\"; else : > \"${REF_STATE}\"; printf '%s' \"${INITIAL_REF}\"; fi ;;\n"
                    "  *'/compare/'*) printf '%s' \"${COMPARE}\" ;;\n"
                    "  *'/contents/'*'ref=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'*) printf '%s' '{\"sha\":\"cccccccccccccccccccccccccccccccccccccccc\"}' ;;\n"
                    "  *'/contents/'*'ref=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'*) printf '%s' '{\"sha\":\"cccccccccccccccccccccccccccccccccccccccc\"}' ;;\n"
                    "  *'/contents/'*) printf '%s' \"${TIP_BLOB}\" ;;\n"
                    "  *'repos/owner/repository'*) printf '%s' '{\"default_branch\":\"master\"}' ;;\n"
                    "  *) exit 91 ;;\nesac\n",
                    encoding="utf-8",
                )
                fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                compare = comparison if comparison is not None else {
                    "status": "ahead", "base_commit": {"sha": base},
                    "merge_base_commit": {"sha": base}, "head_commit": {"sha": tip},
                }
                environment = os.environ | {
                    "EVENT_NAME": "workflow_run", "EVENT_ACTION": "completed", "WORKFLOW_RUN_ID": "9", "WORKFLOW_RUN_ATTEMPT": source_attempt,
                    "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output),
                    "RUN": json.dumps(run if source_run is None else source_run), "PULL": json.dumps(pull), "INITIAL_REF": json.dumps({"object": {"sha": tip}}),
                    "FINAL_REF": json.dumps({"object": {"sha": final_tip if final_tip is not None else tip}}),
                    "COMPARE": json.dumps(compare), "TIP_BLOB": json.dumps({"sha": tip_blob}), "REF_STATE": str(directory / "ref-state"),
                    "CALL_LOG": str(directory / "calls"),
                    "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=environment, capture_output=True, text=True, check=False)
                calls = (directory / "calls").read_text(encoding="utf-8").splitlines()
                ref_indexes = [index for index, call in enumerate(calls) if "/git/ref/heads/master" in call]
                compare_indexes = [index for index, call in enumerate(calls) if "/compare/" in call]
                if compare_indexes:
                    self.assertGreaterEqual(len(ref_indexes), 2, label)
                    self.assertLess(compare_indexes[-1], ref_indexes[-1], label)
                if result.returncode == 0:
                    values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
                    self.assertEqual(values["reconcile"], "true", label)
                return result

        self.assertEqual(execute("base-forward", local_pull).returncode, 0)
        for name, path in (
            ("CI", ".github/workflows/test-and-build.yml@master"),
            ("release-preflight", ".github/workflows/release-preflight.yml@master"),
        ):
            with self.subTest(name=name, source="rerun"):
                retry = {**run, "name": name, "path": path, "run_attempt": 2}
                self.assertEqual(
                    execute(
                        "non-sensor-rerun",
                        local_pull,
                        source_run=retry,
                        source_attempt="2",
                    ).returncode,
                    0,
                )
        rerun_sensor = {
            **run,
            "name": "PR governance review sensor",
            "path": ".github/workflows/pr-governance-review-events.yml@master",
            "event": "pull_request_review",
            "run_attempt": 2,
        }
        rerun_result = execute(
            "review-sensor-rerun",
            local_pull,
            source_run=rerun_sensor,
            source_attempt="2",
        )
        self.assertNotEqual(rerun_result.returncode, 0)
        self.assertIn("workflow_run is not a trusted governance source", rerun_result.stderr)
        rejected = (
            ("rewound-or-diverged", local_pull, {"comparison": {"status": "diverged", "base_commit": {"sha": base}, "merge_base_commit": {"sha": "e" * 40}, "head_commit": {"sha": tip}}}),
            ("current-base-not-tip", {**local_pull, "base": {**local_pull["base"], "sha": "e" * 40}}, {}),
            ("current-workflow-blob", local_pull, {"tip_blob": "e" * 40}),
            ("default-ref-race", local_pull, {"final_tip": "e" * 40}),
            ("head-drift", {**local_pull, "head": {**local_pull["head"], "sha": "e" * 40}}, {}),
            ("head-repository-drift", {**local_pull, "head": {"sha": head, "repo": {"id": 202, "full_name": "fork/repository"}}}, {}),
        )
        for label, pull, options in rejected:
            with self.subTest(source=label):
                self.assertNotEqual(execute(label, pull, **options).returncode, 0)

    def test_issue_and_issue_comment_priority_all_closers_of_the_changed_issue(self) -> None:
        match = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(match); assert match is not None
        pulls = [[
            {"number": 72, "state": "open", "body": "Fixes #64", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"repo": {"full_name": "owner/repository"}}},
            {"number": 73, "state": "open", "body": "Closes https://github.com/owner/repository/issues/64", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"repo": {"full_name": "owner/repository"}}},
        ]]
        for event_name, issue, expected in (
            ("issues", "999", {"reconcile": "true", "event_targets": "[]", "priority_targets": "[]"}),
            ("issues", "64", {"reconcile": "true", "event_targets": "[72,73]", "priority_targets": "[72,73]"}),
            ("issue_comment", "64", {"reconcile": "true", "event_targets": "[72,73]", "priority_targets": "[72,73]"}),
        ):
            with self.subTest(event_name=event_name, issue=issue), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
                fake.write_text("#!/bin/sh\nprintf '%s' \"${PULLS}\"\n", encoding="utf-8"); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                environment = os.environ | {"EVENT_NAME": event_name, "ISSUE_NUMBER": issue, "ISSUE_PULL_REQUEST_URL": "", "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output), "PULLS": json.dumps(pulls), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}"}
                result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, 0, result.stderr)
                values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
                self.assertEqual(values, expected)
        self.assertIn("if: needs.resolve_event.outputs.reconcile == 'true'", self.workflow)
        self.assertIn("if: steps.current-targets.outputs.has_targets == 'true'", self.workflow)

    def test_dispatcher_accepts_optional_colon_in_closing_references(self) -> None:
        match = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(match); assert match is not None
        pulls = [[
            {"number": 72, "state": "open", "body": "Fixes: #64", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"repo": {"full_name": "owner/repository"}}},
            {"number": 73, "state": "open", "body": "Resolves: https://github.com/owner/repository/issues/64", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"repo": {"full_name": "owner/repository"}}},
        ]]
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
            fake.write_text("#!/bin/sh\nprintf '%s' \"${PULLS}\"\n", encoding="utf-8")
            fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            environment = os.environ | {
                "EVENT_NAME": "issues", "ISSUE_NUMBER": "64", "ISSUE_PULL_REQUEST_URL": "",
                "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master",
                "GITHUB_OUTPUT": str(output), "PULLS": json.dumps(pulls),
                "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=environment, capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 0, result.stderr)
            values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
            self.assertEqual(values["event_targets"], "[72,73]")

    def test_dispatcher_url_closers_match_canonical_terminators(self) -> None:
        match = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(match); assert match is not None
        bodies = (
            "Fixes https://github.com/owner/repository/issues/64",
            "Fixes https://github.com/owner/repository/issues/64?source=pr",
            "Fixes https://github.com/owner/repository/issues/64)",
            "Fixes https://github.com/owner/repository/issues/64/foo",
            "Fixes https://github.com/owner/repository/issues/64/",
            "Fixes https://github.com/owner/repository/issues/64-",
            "Fixes https://github.com/owner/repository/issues/64#fragment",
            "Fixes https://github.com/owner/repository/issues/64=other",
            "Fixes https://github.com/other/repository/issues/64",
        )
        expected = [
            index + 72
            for index, body in enumerate(bodies)
            if canonical_issue_contract.closing_issue_numbers(body, "owner/repository") == {64}
        ]
        pulls = [[
            {"number": index + 72, "state": "open", "body": body, "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"repo": {"full_name": "owner/repository"}}}
            for index, body in enumerate(bodies)
        ]]
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
            fake.write_text("#!/bin/sh\nprintf '%s' \"${PULLS}\"\n", encoding="utf-8")
            fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            environment = os.environ | {
                "EVENT_NAME": "issues", "ISSUE_NUMBER": "64", "ISSUE_PULL_REQUEST_URL": "",
                "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master",
                "GITHUB_OUTPUT": str(output), "PULLS": json.dumps(pulls),
                "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=environment, capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 0, result.stderr)
            values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
            self.assertEqual(values["event_targets"], json.dumps(expected, separators=(",", ":")))

    def test_malformed_workflow_source_expands_every_derivable_issue_closure(self) -> None:
        match = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(match); assert match is not None
        base, head = "b" * 40, "a" * 40
        source_repo = {"id": 101, "name": "repository", "url": "https://api.github.com/repos/owner/repository"}
        run = {"name": "CI", "path": ".github/workflows/test-and-build.yml@main", "event": "pull_request", "status": "requested", "id": 9, "run_number": 1, "run_attempt": 1, "head_sha": head, "repository": {"id": 101, "full_name": "owner/repository"}, "pull_requests": [{"number": 72, "base": {"sha": base, "ref": "master", "repo": source_repo}, "head": {"sha": head, "repo": source_repo}}]}
        local = {"base": {"ref": "master", "repo": {"id": 101, "full_name": "owner/repository"}}, "head": {"repo": {"id": 101, "full_name": "owner/repository"}}}
        current_pull = {"number": 72, "state": "open", "base": {"sha": base, **local["base"]}, "head": {"sha": head, **local["head"]}}
        pulls = [[{"number": 72, "state": "open", "body": "Fixes #64; closes #65", **local}, {"number": 73, "state": "open", "body": "Fixes #64", **local}, {"number": 74, "state": "open", "body": "Fixes #65", **local}]]
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
            fake.write_text("#!/bin/sh\ncase \"$*\" in\n  *'/actions/runs/9'*) printf '%s' \"${RUN}\" ;;\n  *'/pulls/72'*) printf '%s' \"${PULL}\" ;;\n  *'git/ref/heads/master'*) printf '%s' '{\"object\":{\"sha\":\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"}}' ;;\n  *'/compare/'*) printf '%s' '{\"status\":\"identical\",\"base_commit\":{\"sha\":\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"},\"merge_base_commit\":{\"sha\":\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"},\"head_commit\":{\"sha\":\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"}}' ;;\n  *'/contents/'*) printf '%s' '{\"sha\":\"cccccccccccccccccccccccccccccccccccccccc\"}' ;;\n  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n  *'repos/owner/repository'*) printf '%s' '{\"default_branch\":\"master\"}' ;;\n  *) exit 91 ;;\nesac\n", encoding="utf-8"); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            environment = os.environ | {"EVENT_NAME": "workflow_run", "EVENT_ACTION": "completed", "WORKFLOW_RUN_ID": "9", "WORKFLOW_RUN_ATTEMPT": "1", "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output), "RUN": json.dumps(run), "PULL": json.dumps(current_pull), "PULLS": json.dumps(pulls), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}"}
            result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=environment, capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("reconcile=true", output.read_text(encoding="utf-8"))

    def test_pull_request_target_revalidates_old_and_new_closures_but_skips_forks(self) -> None:
        match = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(match); assert match is not None
        base, head = "b" * 40, "a" * 40
        local = {"base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"repo": {"full_name": "owner/repository"}}}
        pulls = [[
            {"number": 72, "state": "open", "body": "Fixes #64", **local},
            {"number": 73, "state": "open", "body": "Fixes #65", **local},
        ]]
        current = {"number": 73, "state": "open", "base": {"sha": base, **local["base"]}, "head": {"sha": head, **local["head"]}}
        for action, source, expected in (
            ("opened", current, {"reconcile": "true", "event_targets": "[73,72]", "priority_targets": "[73,72]"}),
            ("edited", current, {"reconcile": "true", "event_targets": "[73,72]", "priority_targets": "[73,72]"}),
            ("closed", {**current, "state": "closed"}, {"reconcile": "true", "event_targets": "[73,72]", "priority_targets": "[73,72]"}),
            ("edited", {**current, "head": {"sha": head, "repo": {"full_name": "fork/repository"}}}, {"reconcile": "true", "event_targets": "[]", "priority_targets": "[]"}),
        ):
            with self.subTest(action=action, source=source["head"]["repo"]["full_name"]), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
                fake.write_text(
                    "#!/bin/sh\ncase \"$*\" in\n  *'/pulls/73'*) printf '%s' \"${SOURCE}\" ;;\n  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n  *) exit 91 ;;\nesac\n",
                    encoding="utf-8",
                ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                environment = os.environ | {
                    "EVENT_NAME": "pull_request_target", "PR_ACTION": action, "PR_NUMBER": "73", "PR_HEAD_SHA": head,
                    "PR_BASE_SHA": base, "PR_BODY": "Fixes #65", "PR_PREVIOUS_BODY": "Fixes #64", "GITHUB_REPOSITORY": "owner/repository",
                    "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output), "PULLS": json.dumps(pulls), "SOURCE": json.dumps(source),
                    "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, 0, result.stderr)
                values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
                self.assertEqual(values, expected)

    def test_pull_request_target_retarget_revalidates_prior_governed_claimants(self) -> None:
        match = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(match); assert match is not None
        base, head = "b" * 40, "a" * 40
        governed = {
            "number": 72,
            "state": "open",
            "body": "Fixes #64",
            "base": {"ref": "master", "repo": {"id": 101, "full_name": "owner/repository"}},
            "head": {"repo": {"id": 101, "full_name": "owner/repository"}},
        }
        retargeted = {
            "number": 73,
            "state": "open",
            "base": {"sha": base, "ref": "release/v1", "repo": {"id": 101, "full_name": "owner/repository"}},
            "head": {"sha": head, "repo": {"id": 101, "full_name": "owner/repository"}},
        }
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
            fake.write_text(
                "#!/bin/sh\ncase \"$*\" in\n"
                "  *'/pulls/73'*) printf '%s' \"${SOURCE}\" ;;\n"
                "  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n"
                "  *) exit 91 ;;\nesac\n",
                encoding="utf-8",
            ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            base_environment = os.environ | {
                "EVENT_NAME": "pull_request_target", "PR_ACTION": "edited", "PR_NUMBER": "73", "PR_HEAD_SHA": head,
                "PR_BASE_SHA": base, "PR_BODY": "Fixes #65", "PR_PREVIOUS_BODY": "Fixes #64",
                "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output),
                "PULLS": json.dumps([[governed, {**retargeted, "body": "Fixes #65"}]]), "SOURCE": json.dumps(retargeted),
                "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            environment = base_environment | {"PR_PREVIOUS_BASE_REF": "master"}
            result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=environment, capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 0, result.stderr)
            values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
            self.assertEqual(values["event_targets"], "[72]")
            self.assertEqual(values["priority_targets"], "[72]")

            missing_output = directory / "output-missing"
            missing = base_environment | {
                "GITHUB_OUTPUT": str(missing_output),
                "PR_PREVIOUS_BASE_REF": "",
            }
            result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=missing, capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 0, result.stderr)
            values = dict(line.split("=", 1) for line in missing_output.read_text(encoding="utf-8").splitlines())
            self.assertEqual(values["event_targets"], "[]")
            self.assertEqual(values["priority_targets"], "[]")

            nondefault_output = directory / "output-nondefault"
            nondefault = base_environment | {
                "GITHUB_OUTPUT": str(nondefault_output),
                "PR_PREVIOUS_BASE_REF": "release/v0",
            }
            result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=nondefault, capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 0, result.stderr)
            values = dict(line.split("=", 1) for line in nondefault_output.read_text(encoding="utf-8").splitlines())
            self.assertEqual(values["event_targets"], "[]")
            self.assertEqual(values["priority_targets"], "[]")

            for label, repo_change in (
                ("base-id-missing", ("base", {"full_name": "owner/repository"})),
                ("base-id-bool", ("base", {"id": True, "full_name": "owner/repository"})),
                ("base-same-name-wrong-id", ("base", {"id": 202, "full_name": "owner/repository"})),
                ("base-same-id-foreign-name", ("base", {"id": 101, "full_name": "other/repository"})),
                ("head-id-missing", ("head", {"full_name": "owner/repository"})),
                ("head-id-bool", ("head", {"id": False, "full_name": "owner/repository"})),
                ("head-same-name-wrong-id", ("head", {"id": 202, "full_name": "owner/repository"})),
                ("head-same-id-foreign-name", ("head", {"id": 101, "full_name": "other/repository"})),
            ):
                with self.subTest(identity=label):
                    malformed = json.loads(json.dumps(retargeted))
                    malformed[repo_change[0]]["repo"] = repo_change[1]
                    malformed_output = directory / f"output-{label}"
                    malformed_environment = base_environment | {"GITHUB_OUTPUT": str(malformed_output), "SOURCE": json.dumps(malformed), "PR_PREVIOUS_BASE_REF": "release/v0"}
                    result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=malformed_environment, capture_output=True, text=True, check=False)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    malformed_values = dict(line.split("=", 1) for line in malformed_output.read_text(encoding="utf-8").splitlines())
                    self.assertEqual(malformed_values["event_targets"], "[]")
                    self.assertEqual(malformed_values["priority_targets"], "[]")

            for previous_base in ("../master", "/master"):
                with self.subTest(previous_base=previous_base):
                    malformed_output = directory / f"output-{len(previous_base)}"
                    malformed = base_environment | {
                        "GITHUB_OUTPUT": str(malformed_output),
                        "PR_PREVIOUS_BASE_REF": previous_base,
                    }
                    result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=malformed, capture_output=True, text=True, check=False)
                    self.assertNotEqual(result.returncode, 0)

    def test_priority_snapshot_accepts_all_related_closers(self) -> None:
        resolver = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        current = re.search(r"- name: Re-enumerate every current local governance pull request.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(resolver); self.assertIsNotNone(current)
        assert resolver is not None and current is not None
        base, head = "b" * 40, "a" * 40
        local_base = {"ref": "master", "repo": {"full_name": "owner/repository"}}
        all_pulls = [
            {
                "number": number,
                "state": "open",
                "body": "Fixes #64" if number in {72, 73} else "Fixes #99",
                "draft": False,
                "base": local_base,
                "head": {"sha": f"{number:040x}", "repo": {"full_name": "owner/repository"}},
            }
            for number in range(1, 106)
        ]
        pulls = [all_pulls[:100], all_pulls[100:]]
        source = {"number": 72, "state": "open", "base": {"sha": base, **local_base}, "head": {"sha": head, "repo": {"full_name": "owner/repository"}}}
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); fake = directory / "gh"; output = directory / "resolve-output"
            fake.write_text(
                "#!/bin/sh\ncase \"$*\" in\n"
                "  *'/pulls/72'*) printf '%s' \"${SOURCE}\" ;;\n"
                "  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n"
                "  *) exit 91 ;;\nesac\n", encoding="utf-8",
            ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            environment = os.environ | {
                "EVENT_NAME": "pull_request_target", "PR_ACTION": "ready_for_review", "PR_NUMBER": "72", "PR_HEAD_SHA": head,
                "PR_BASE_SHA": base, "PR_BODY": "Fixes #64", "PR_PREVIOUS_BODY": "Fixes #64", "GITHUB_REPOSITORY": "owner/repository",
                "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output), "PULLS": json.dumps(pulls), "SOURCE": json.dumps(source),
                "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            result = subprocess.run([sys.executable, "-c", self._workflow_program(resolver)], env=environment, capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 0, result.stderr)
            resolved = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
            self.assertEqual(resolved["priority_targets"], "[72,73]")
            self.assertEqual(json.loads(resolved["event_targets"]), [72, 73])

            fake.write_text(
                "#!/bin/sh\ncase \"$*\" in\n"
                "  *'git/ref/heads/master'*) printf '%s' '{\"object\":{\"sha\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}}' ;;\n"
                "  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n"
                "  *'repos/owner/repository'*) printf '%s' '{\"default_branch\":\"master\"}' ;;\n"
                "  *) exit 91 ;;\nesac\n", encoding="utf-8",
            ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            current_output = directory / "current-output"
            current_environment = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository", "GITHUB_OUTPUT": str(current_output), "PULLS": json.dumps(pulls),
                "EVENT_TARGETS": resolved["event_targets"], "EVENT_PRIORITY_TARGETS": resolved["priority_targets"],
                "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            result = subprocess.run([sys.executable, "-c", self._workflow_program(current)], env=current_environment, capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 0, result.stderr)
            selected = dict(line.split("=", 1) for line in current_output.read_text(encoding="utf-8").splitlines())
            self.assertEqual(json.loads(selected["priority_targets"]), [72, 73])
            self.assertEqual(selected["event_targets"], "[72,73]")
            self.assertEqual(json.loads(selected["event_targets"]), [72, 73])
            invalidations = json.loads(selected["all_invalidation_targets"])
            self.assertNotIn(72, invalidations)
            self.assertNotIn(73, invalidations)
            self.assertEqual(invalidations[0], 1)
            self.assertEqual(len(json.loads(selected["targets"])), 105)
        early = self.workflow.index("Dispatch and bind the early event writer")
        full = self.workflow.index("Invalidate every current pull request for the all-open writer")
        self.assertNotIn("AFFECTED: ${{ steps.current-targets.outputs.priority_targets }}", self.workflow[early:full])
        self.assertIn("AFFECTED: ${{ steps.current-targets.outputs.all_invalidation_chunk_1 }}", self.workflow[full:])

    def test_priority_duplicate_head_skips_early_writer_and_invalidates_every_known_head(self) -> None:
        current = re.search(
            r"- name: Re-enumerate every current local governance pull request.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        invalidator = re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(current); self.assertIsNotNone(invalidator)
        assert current is not None and invalidator is not None
        duplicate_head, unique_head = "a" * 40, "b" * 40
        local_base = {"ref": "master", "repo": {"full_name": "owner/repository"}}
        pulls = [[
            {"number": 72, "state": "open", "body": "Fixes #64", "draft": False, "base": local_base, "head": {"sha": duplicate_head, "repo": {"full_name": "owner/repository"}}},
            {"number": 73, "state": "open", "body": "Fixes #64", "draft": False, "base": local_base, "head": {"sha": duplicate_head.upper(), "repo": {"full_name": "owner/repository"}}},
            {"number": 74, "state": "open", "body": "Fixes #99", "draft": False, "base": local_base, "head": {"sha": unique_head, "repo": {"full_name": "owner/repository"}}},
        ]]
        response = {
            "id": 101, "app": {"id": 42}, "name": "KRR / PR governance (trusted check)",
            "head_sha": duplicate_head, "external_id": f"krr-governance/v1/{duplicate_head}/dispatcher-9",
            "status": "in_progress", "conclusion": None,
            "details_url": "https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=0",
        }
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); fake = directory / "gh"; selection_output = directory / "selection"; posts = directory / "posts"
            fake.write_text(
                "#!/bin/sh\ncase \"$*\" in\n"
                "  *'git/ref/heads/master'*) printf '%s' '{\"object\":{\"sha\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}}' ;;\n"
                "  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n"
                "  *'repos/owner/repository'*) printf '%s' '{\"default_branch\":\"master\"}' ;;\n"
                "  *) exit 91 ;;\nesac\n",
                encoding="utf-8",
            ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            selection_environment = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository", "GITHUB_OUTPUT": str(selection_output),
                "EVENT_TARGETS": "[72,73]", "EVENT_PRIORITY_TARGETS": "[72,73]", "PULLS": json.dumps(pulls),
                "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            selected = subprocess.run(
                [sys.executable, "-c", self._workflow_program(current)],
                env=selection_environment, capture_output=True, text=True, check=False,
            )
            self.assertEqual(selected.returncode, 0, selected.stderr)
            selection = dict(line.split("=", 1) for line in selection_output.read_text(encoding="utf-8").splitlines())
            self.assertEqual(selection["priority_targets"], "[]")
            self.assertEqual(json.loads(selection["preinvalidate_targets"]), [72, 73])
            self.assertEqual(json.loads(selection["all_invalidation_targets"]), [74])

            fake.write_text(
                "#!/bin/sh\ncase \"$*\" in\n"
                f"  *'/pulls/72'|*'/pulls/73'*) printf '%s' '{{\"draft\":false,\"head\":{{\"sha\":\"{duplicate_head}\"}}}}' ;;\n"
                f"  *'/pulls/74'*) printf '%s' '{{\"draft\":false,\"head\":{{\"sha\":\"{unique_head}\"}}}}' ;;\n"
                f"  *'check-runs/101'*) printf '%s' '{json.dumps(response)}' ;;\n"
                f"  *'--method POST'*) echo \"$*\" >> '{posts}'; printf '%s' '{json.dumps(response)}' ;;\n"
                "  *'/dispatches'*) exit 92 ;;\n"
                "  *) exit 91 ;;\nesac\n",
                encoding="utf-8",
            ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            invalidator_program = self._workflow_program(invalidator).replace("time.sleep(delay)", "None").replace("write_clock=[time.monotonic()+8.1]", "write_clock=[time.monotonic()]")
            invalidation_environment = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "9",
                "GH_TOKEN": "read", "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42",
                "DUPLICATE_GOVERNED_HEADS": json.dumps([duplicate_head]),
                "AFFECTED": selection["all_invalidation_targets"],
                "KNOWN_TARGET_SNAPSHOTS": selection["all_invalidation_target_snapshots"],
                "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            invalidated = subprocess.run(
                [sys.executable, "-c", invalidator_program],
                env=invalidation_environment, capture_output=True, text=True, check=False,
            )
            self.assertNotEqual(invalidated.returncode, 0)
            writes = posts.read_text(encoding="utf-8").splitlines()
            self.assertEqual(len(writes), 1)
            self.assertIn(f"head_sha={unique_head}", writes[0])
        early = self.workflow.index("Dispatch and bind the early event writer")
        await_early = self.workflow.index("Await the bound early event writer before all-open invalidation")
        self.assertIn("if: steps.current-targets.outputs.has_priority_targets == 'true'", self.workflow[early:await_early])

    def test_unrelated_duplicate_head_also_suppresses_the_priority_writer(self) -> None:
        current = re.search(
            r"- name: Re-enumerate every current local governance pull request.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(current); assert current is not None
        source_head, duplicate_head = "c" * 40, "a" * 40
        base = {"ref": "master", "repo": {"full_name": "owner/repository"}}
        pulls = [[
            {"number": 72, "state": "open", "body": "Fixes #64", "draft": False, "base": base, "head": {"sha": source_head, "repo": {"full_name": "owner/repository"}}},
            {"number": 73, "state": "open", "body": "Fixes #99", "draft": False, "base": base, "head": {"sha": duplicate_head, "repo": {"full_name": "owner/repository"}}},
            {"number": 74, "state": "open", "body": "Fixes #100", "draft": False, "base": base, "head": {"sha": duplicate_head.upper(), "repo": {"full_name": "owner/repository"}}},
        ]]
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
            fake.write_text(
                "#!/bin/sh\ncase \"$*\" in\n"
                "  *'git/ref/heads/master'*) printf '%s' '{\"object\":{\"sha\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}}' ;;\n"
                "  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n"
                "  *'repos/owner/repository'*) printf '%s' '{\"default_branch\":\"master\"}' ;;\n"
                "  *) exit 91 ;;\nesac\n",
                encoding="utf-8",
            ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            environment = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository", "GITHUB_OUTPUT": str(output),
                "EVENT_TARGETS": "[72]", "EVENT_PRIORITY_TARGETS": "[72]", "PULLS": json.dumps(pulls),
                "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            result = subprocess.run(
                [sys.executable, "-c", self._workflow_program(current)],
                env=environment, capture_output=True, text=True, check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            selection = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
            self.assertEqual(selection["priority_targets"], "[]")
            self.assertEqual(json.loads(selection["preinvalidate_targets"]), [72])
            self.assertEqual(json.loads(selection["all_invalidation_targets"]), [73, 74])

    def test_shared_heads_are_single_generation_and_block_all_writer_before_manifest(self) -> None:
        """Every shared-head shape is fenced once before the all-writer hand-off."""
        current = re.search(
            r"- name: Re-enumerate every current local governance pull request.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(current); assert current is not None

        local_base = {"ref": "master", "repo": {"full_name": "owner/repository"}}
        first, second, third = "a" * 40, "b" * 40, "c" * 40
        cases = (
            (
                "event duplicate",
                [72, 73],
                [72, 73],
                [
                    {"number": 72, "state": "open", "body": "Fixes #64", "draft": False, "base": local_base, "head": {"sha": first, "repo": {"full_name": "owner/repository"}}},
                    {"number": 73, "state": "open", "body": "Fixes #64", "draft": False, "base": local_base, "head": {"sha": first, "repo": {"full_name": "owner/repository"}}},
                    {"number": 74, "state": "open", "body": "Fixes #99", "draft": False, "base": local_base, "head": {"sha": third, "repo": {"full_name": "owner/repository"}}},
                ],
                [72, 73], [74], [first], [first],
            ),
            (
                "event and unrelated share",
                [72],
                [72],
                [
                    {"number": 72, "state": "open", "body": "Fixes #64", "draft": False, "base": local_base, "head": {"sha": first, "repo": {"full_name": "owner/repository"}}},
                    {"number": 73, "state": "open", "body": "Fixes #99", "draft": False, "base": local_base, "head": {"sha": first, "repo": {"full_name": "owner/repository"}}},
                    {"number": 74, "state": "open", "body": "Fixes #100", "draft": False, "base": local_base, "head": {"sha": third, "repo": {"full_name": "owner/repository"}}},
                ],
                [72], [74], [first], [first],
            ),
            (
                "unrelated shared suppresses unique source",
                [72],
                [72],
                [
                    {"number": 72, "state": "open", "body": "Fixes #64", "draft": False, "base": local_base, "head": {"sha": first, "repo": {"full_name": "owner/repository"}}},
                    {"number": 73, "state": "open", "body": "Fixes #99", "draft": False, "base": local_base, "head": {"sha": second, "repo": {"full_name": "owner/repository"}}},
                    {"number": 74, "state": "open", "body": "Fixes #100", "draft": False, "base": local_base, "head": {"sha": second, "repo": {"full_name": "owner/repository"}}},
                ],
                [72], [73, 74], [first], [second],
            ),
        )
        selections: dict[str, dict[str, str]] = {}
        for name, event_targets, priority_targets, pulls, expected_pre, expected_all, expected_pre_heads, expected_duplicate in cases:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); output = directory / "output"; fake = directory / "gh"
                fake.write_text(
                    "#!/bin/sh\ncase \"$*\" in\n"
                    "  *'git/ref/heads/master'*) printf '%s' '{\"object\":{\"sha\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}}' ;;\n"
                    "  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n"
                    "  *'repos/owner/repository'*) printf '%s' '{\"default_branch\":\"master\"}' ;;\n"
                    "  *) exit 91 ;;\nesac\n",
                    encoding="utf-8",
                ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "GITHUB_OUTPUT": str(output),
                    "EVENT_TARGETS": json.dumps(event_targets), "EVENT_PRIORITY_TARGETS": json.dumps(priority_targets),
                    "PULLS": json.dumps([pulls]), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", self._workflow_program(current)], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, 0, result.stderr)
                selected = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
                self.assertEqual(json.loads(selected["preinvalidate_targets"]), expected_pre)
                self.assertEqual(json.loads(selected["preinvalidated_heads"]), expected_pre_heads)
                self.assertEqual(json.loads(selected["duplicate_governed_heads"]), expected_duplicate)
                self.assertEqual(selected["has_duplicate_governed_heads"], "true")
                self.assertEqual(json.loads(selected["all_invalidation_targets"]), expected_all)
                pre_heads = set(json.loads(selected["preinvalidated_heads"]))
                all_heads = {entry[1] for entry in json.loads(selected["all_invalidation_target_snapshots"])}
                self.assertTrue(pre_heads.isdisjoint(all_heads))
                selections[name] = selected

        pre_match = re.search(
            r"- name: Pre-invalidate priority event heads.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        all_match = re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(pre_match); self.assertIsNotNone(all_match)
        assert pre_match is not None and all_match is not None
        pre_program = self._workflow_program(pre_match)
        all_program = self._workflow_program(all_match).replace("time.sleep(delay)", "None").replace("write_clock=[time.monotonic()+8.1]", "write_clock=[time.monotonic()]")

        def snapshot_pulls(selected: dict[str, str]) -> dict[int, dict[str, object]]:
            records: dict[int, dict[str, object]] = {}
            raw = json.loads(selected["preinvalidate_target_snapshots"] or "[]") + json.loads(selected["all_invalidation_target_snapshots"] or "[]")
            for entry in raw:
                base_ref, base_repo, head_repo = (entry[3], entry[4], entry[5]) if len(entry) == 6 else ("master", "owner/repository", "owner/repository")
                records[entry[0]] = {
                    "number": entry[0], "state": "open", "draft": entry[2],
                    "base": {"ref": base_ref, "repo": {"full_name": base_repo}},
                    "head": {"sha": entry[1], "repo": {"full_name": head_repo}},
                }
            return records

        def run_pre(selected: dict[str, str]) -> tuple[list[str], list[str]]:
            records = snapshot_pulls(selected)
            targets = json.loads(selected["preinvalidate_targets"])
            posts: list[str] = []; rereads: list[str] = []; created: dict[int, dict[str, object]] = {}
            def response(value: object, code: int = 0) -> subprocess.CompletedProcess[str]:
                return subprocess.CompletedProcess([], code, json.dumps(value), "")
            def fake_run(arguments: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                endpoint = next((item for item in arguments if isinstance(item, str) and item.startswith("repos/")), arguments[-1])
                if isinstance(endpoint, str) and "/pulls/" in endpoint:
                    self.assertEqual(kwargs.get("env"), {"GH_TOKEN": "read", "PATH": os.environ["PATH"]})
                    return response(records[int(endpoint.rsplit("/", 1)[1])])
                if "--method" in arguments and "POST" in arguments:
                    self.assertEqual(kwargs.get("env"), {"GH_TOKEN": "write", "PATH": os.environ["PATH"]})
                    fields = {item.split("=", 1)[0]: item.split("=", 1)[1] for item in arguments if "=" in item}
                    identifier = 700 + len(posts) + 1
                    check = {"id": identifier, "app": {"id": 42}, "name": "KRR / PR governance (trusted check)", "head_sha": fields["head_sha"], "external_id": fields["external_id"], "status": "in_progress", "conclusion": None, "details_url": fields["details_url"]}
                    created[identifier] = check; posts.append(fields["head_sha"])
                    return response(check)
                if isinstance(endpoint, str) and "/check-runs/" in endpoint:
                    self.assertEqual(kwargs.get("env"), {"GH_TOKEN": "read", "PATH": os.environ["PATH"]})
                    identifier = int(endpoint.rsplit("/", 1)[1]); rereads.append(str(identifier)); return response(created[identifier])
                raise AssertionError(arguments)
            environment = os.environ | {"GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GH_TOKEN": "read", "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "TARGETS": selected["preinvalidate_targets"], "TARGET_SNAPSHOTS": selected["preinvalidate_target_snapshots"], "DEFAULT_BRANCH": "master", "DISPATCHER_RUN_ID": "9", "GITHUB_OUTPUT": str(Path(tempfile.mkdtemp()) / "output"), "PATH": os.environ["PATH"]}
            with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run), patch("time.sleep"):
                exec(pre_program, {"__name__": "__main__"})
            self.assertEqual(len(posts), len(set(posts)))
            return posts, rereads

        def run_all(selected: dict[str, str], expected_returncode: int) -> tuple[list[str], list[list[int]]]:
            records = snapshot_pulls(selected)
            targets = json.loads(selected["all_invalidation_targets"])
            posts: list[str] = []; writer_dispatches: list[list[str]] = []; created: dict[int, dict[str, object]] = {}; writer_runs: list[dict[str, object]] = []; output = Path(tempfile.mkdtemp()) / "output"
            def response(value: object, code: int = 0) -> subprocess.CompletedProcess[str]:
                return subprocess.CompletedProcess([], code, json.dumps(value), "")
            def fake_run(arguments: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                endpoint = next((item for item in arguments if isinstance(item, str) and item.startswith("repos/")), arguments[-1])
                if isinstance(endpoint, str) and "/commits/" in endpoint and "check-runs?" in endpoint:
                    return response([{"check_runs": []}])
                if "--method" in arguments and "POST" in arguments and isinstance(endpoint, str) and endpoint.endswith("/check-runs"):
                    self.assertEqual(kwargs.get("env"), {"GH_TOKEN": "write", "PATH": os.environ["PATH"]})
                    fields = {item.split("=", 1)[0]: item.split("=", 1)[1] for item in arguments if "=" in item}
                    identifier = 800 + len(posts) + 1
                    check = {"id": identifier, "app": {"id": 42}, "name": "KRR / PR governance (trusted check)", "head_sha": fields["head_sha"], "external_id": fields["external_id"], "status": "in_progress", "conclusion": None, "details_url": fields["details_url"]}
                    created[identifier] = check; posts.append(fields["head_sha"]); return response(check)
                if isinstance(endpoint, str) and "/check-runs/" in endpoint:
                    return response(created[int(endpoint.rsplit("/", 1)[1])])
                if isinstance(endpoint, str) and "/pulls/" in endpoint:
                    return response(records[int(endpoint.rsplit("/", 1)[1])])
                if isinstance(endpoint, str) and endpoint == "repos/owner/repository":
                    return response({"default_branch": "master"})
                if isinstance(endpoint, str) and endpoint.endswith("/git/ref/heads/master"):
                    return response({"object": {"sha": "a" * 40}})
                if isinstance(endpoint, str) and endpoint.endswith("/actions/runs/9"):
                    return response({"id": 9, "name": "PR governance dispatcher", "event": "issues", "head_branch": "master", "head_sha": "a" * 40, "repository": {"full_name": "owner/repository"}, "run_number": 1, "run_attempt": 1, "status": "in_progress", "created_at": "2026-09-01T00:00:00Z"})
                if isinstance(endpoint, str) and "pulls?state=open" in endpoint:
                    return response(list(records.values()))
                if isinstance(endpoint, str) and "pr-governance-status-writer.yml/runs?" in endpoint:
                    return response({"total_count": len(writer_runs), "workflow_runs": writer_runs})
                if "--method" in arguments and "POST" in arguments and any(isinstance(item, str) and "/dispatches" in item for item in arguments):
                    writer_dispatches.append(arguments)
                    writer_runs.append({"id": 901, "name": "PR governance status writer", "display_title": "source=9 scope=all segment=1", "path": ".github/workflows/pr-governance-status-writer.yml@master", "event": "workflow_dispatch", "repository": {"full_name": "owner/repository"}, "head_branch": "master", "head_sha": "a" * 40, "status": "queued", "run_number": 1, "run_attempt": 1})
                    return response({})
                raise AssertionError(arguments)
            environment = os.environ | {"GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "9", "GH_TOKEN": "read", "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "DUPLICATE_GOVERNED_HEADS": selected["duplicate_governed_heads"], "AFFECTED": selected["all_invalidation_targets"], "KNOWN_TARGET_SNAPSHOTS": selected["all_invalidation_target_snapshots"], "EVENT_TARGETS": selected["event_targets"], "GITHUB_OUTPUT": str(output), "PATH": os.environ["PATH"]}
            with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run), patch("time.sleep"):
                try:
                    exec(all_program, {"__name__": "__main__"})
                    code = 0
                except SystemExit:
                    code = 1
            self.assertEqual(code, expected_returncode)
            manifest = [] if not output.exists() else json.loads(dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines()).get("check_manifest", "[]"))
            dispatch_if = self._step_if("Dispatch one repository-wide governance arbiter segment")
            eligibility = self._github_if(dispatch_if, {
                "steps.current-targets.outputs.has_targets": "true",
                "steps.current-targets.outputs.has_duplicate_governed_heads": "true" if expected_returncode else "false",
            })
            if expected_returncode:
                self.assertFalse(eligibility)
                self.assertEqual(writer_dispatches, [])
            else:
                self.assertTrue(eligibility)
                dispatch_program = re.search(r"python3 - <<'PY'\n(.*?)\n          PY", re.search(r"^      - name: Dispatch one repository-wide governance arbiter segment\n(?P<body>.*?)(?=^      - name: |\Z)", self.workflow, re.MULTILINE | re.DOTALL).group("body"), re.DOTALL)
                self.assertIsNotNone(dispatch_program); assert dispatch_program is not None
                snapshots = [[number, records[number]["head"]["sha"], records[number]["draft"]] for number in targets]
                dispatch_env = environment | {
                    "DEFAULT_BRANCH": "master", "WRITER_HEAD": "a" * 40, "DISPATCHER_RUN_ID": "9", "WRITER_SCOPE": "all", "WRITER_TARGETS": selected["event_targets"],
                    "WRITER_ALL_OPEN_TARGETS": json.dumps(targets, separators=(",", ":")), "WRITER_ALL_OPEN_SNAPSHOTS": json.dumps(snapshots, separators=(",", ":")),
                    "WRITER_PRESERVED_TARGETS": "[]", "PRESERVED_WRITER_RUN_ID": "0", "WRITER_PREINVALIDATE_TARGETS": "[]", "WRITER_PRE_CHECK_MANIFEST_1": "[]", "WRITER_PRE_CHECK_MANIFEST_2": "[]",
                    "WRITER_TAIL_CHECK_MANIFEST_1": json.dumps(manifest, separators=(",", ":")), "WRITER_TAIL_CHECK_MANIFEST_2": "[]", "WRITER_PRESERVED_CHECK_MANIFEST": "[]", "WRITER_CARRY_TARGET_NUMBERS_1": "[]", "WRITER_CARRY_TARGET_NUMBERS_2": "[]", "GITHUB_OUTPUT": str(Path(tempfile.mkdtemp()) / "dispatch-output"),
                }
                with patch.dict(os.environ, dispatch_env, clear=True), patch("subprocess.run", side_effect=fake_run):
                    exec(self._workflow_program(dispatch_program), {"__name__": "__main__"})
                self.assertEqual(len(writer_dispatches), 1)
            return posts, manifest

        for name, _, _, _, expected_pre, expected_all, expected_pre_heads, expected_duplicate in cases:
            selected = selections[name]
            pre_posts, pre_rereads = run_pre(selected)
            self.assertEqual(set(pre_posts), set(expected_pre_heads))
            self.assertEqual(len(pre_rereads), len(pre_posts))
            all_posts, manifest = run_all(selected, 1)
            expected_all_heads = [entry[1] for entry in json.loads(selected["all_invalidation_target_snapshots"])]
            self.assertEqual(set(all_posts), set(expected_all_heads))
            self.assertEqual(len(all_posts), len(set(all_posts)))
            self.assertEqual(manifest, [])

        no_duplicate = {
            "preinvalidate_target_snapshots": "[]",
            "all_invalidation_targets": "[72,74]",
            "all_invalidation_target_snapshots": json.dumps([[72, first, False], [74, third, False]], separators=(",", ":")),
            "duplicate_governed_heads": "[]", "event_targets": "[72,74]",
        }
        no_duplicate_posts, no_duplicate_manifest = run_all(no_duplicate, 0)
        self.assertEqual(set(no_duplicate_posts), {first, third})
        self.assertEqual(len(no_duplicate_posts), 2)
        self.assertEqual([entry[0] for entry in no_duplicate_manifest], [72, 74])

        all_step = self.workflow.index("- name: Invalidate every current pull request for the all-open writer")
        dispatch_step = self.workflow.index("- name: Dispatch one repository-wide governance arbiter")
        all_header = self.workflow[all_step: self.workflow.index("run: |", all_step)]
        dispatch_header = self.workflow[dispatch_step: self.workflow.index("run: |", dispatch_step)]
        invalidator = self._workflow_program(re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        ))
        self.assertIn("DUPLICATE_GOVERNED_HEADS:", all_header)
        self.assertIn("DUPLICATE_GOVERNED_HEADS:", all_header)
        self.assertIn("has_all_invalidation_chunk_1 == 'true'", all_header)
        self.assertIn("if duplicate_heads:", invalidator)
        self.assertIn('raise SystemExit("Duplicate governed pull request head SHA.")', invalidator)
        self.assertIn('external_id=f"krr-governance/v1/{head.lower()}/dispatcher-{dispatcher}"', invalidator)
        self.assertRegex(dispatch_header, r"has_duplicate_governed_heads (?:!= 'true'|== 'false')")
        self.assertNotIn("has_duplicate_governed_heads == 'true'", dispatch_header)

    def test_pull_request_target_rejects_source_head_or_state_race(self) -> None:
        match = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(match); assert match is not None
        base, head = "b" * 40, "a" * 40
        source = {"number": 72, "state": "closed", "base": {"sha": base, "ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": head, "repo": {"full_name": "owner/repository"}}}
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
            fake.write_text("#!/bin/sh\ncase \"$*\" in\n  *'/pulls/72'*) printf '%s' \"${SOURCE}\" ;;\n  *'pulls?state=open'*) printf '%s' '[]' ;;\n  *) exit 91 ;;\nesac\n", encoding="utf-8"); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            environment = os.environ | {"EVENT_NAME": "pull_request_target", "PR_ACTION": "closed", "PR_NUMBER": "72", "PR_HEAD_SHA": head, "PR_BASE_SHA": base, "PR_BODY": "Fixes #64", "PR_PREVIOUS_BODY": "", "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output), "SOURCE": json.dumps({**source, "head": {"sha": "c" * 40, "repo": {"full_name": "owner/repository"}}}), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}"}
            result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=environment, capture_output=True, text=True, check=False)
            self.assertNotEqual(result.returncode, 0)

    def test_pull_request_target_accepts_only_a_monotonic_historical_default_base(self) -> None:
        """A queued default-base event may advance, but every other race fails closed."""
        match = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(match); assert match is not None
        source_base, tip, head = "b" * 40, "d" * 40, "a" * 40
        local = {"full_name": "owner/repository"}
        initial = {"number": 72, "state": "open", "base": {"sha": tip, "ref": "master", "repo": local}, "head": {"sha": head, "repo": local}}
        pulls = [[
            {"number": 72, "state": "open", "body": "Fixes #64", "base": {"ref": "master", "repo": local}, "head": {"repo": local}},
            {"number": 73, "state": "open", "body": "Fixes #64", "base": {"ref": "master", "repo": local}, "head": {"repo": local}},
        ]]
        cases = {
            "accepted": ({}, 0),
            "rewind": ({"COMPARE": {"status": "behind", "base_commit": {"sha": source_base}, "merge_base_commit": {"sha": source_base}, "head_commit": {"sha": tip}}}, 1),
            "diverge": ({"COMPARE": {"status": "diverged", "base_commit": {"sha": source_base}, "merge_base_commit": {"sha": "c" * 40}, "head_commit": {"sha": tip}}}, 1),
            "final-head": ({"FINAL": {**initial, "head": {"sha": "c" * 40, "repo": local}}}, 1),
            "final-repository": ({"FINAL": {**initial, "head": {"sha": head, "repo": {"full_name": "fork/repository"}}}}, 1),
            "final-ref": ({"FINAL": {**initial, "base": {"sha": tip, "ref": "release/v1", "repo": local}}}, 1),
            "historical-ref": ({"PR_BASE_REF": "release/v1"}, 1),
            "final-tip": ({"FINAL_TIP": "e" * 40}, 1),
            "workflow": ({"TIP_BLOB": {"sha": "e" * 40}}, 1),
            "initial-number-boolean": ({"INITIAL": {**initial, "number": True}}, 1),
            "final-number-boolean": ({"FINAL": {**initial, "number": True}}, 1),
        }
        for name, (override, expected) in cases.items():
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
                source_number = 1 if name.endswith("number-boolean") else 72
                case_initial = {**initial, "number": source_number}
                case_pulls = [[{**pulls[0][0], "number": source_number}, pulls[0][1]]]
                fake.write_text(
                    "#!/bin/sh\ncase \"$*\" in\n"
                    "  *'/pulls/'\"${PR_NUMBER}\"*) if [ -e \"${PULL_STATE}\" ]; then printf '%s' \"${FINAL}\"; else : > \"${PULL_STATE}\"; printf '%s' \"${INITIAL}\"; fi ;;\n"
                    "  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n"
                    "  *'/git/ref/heads/master'*) if [ -e \"${REF_STATE}\" ]; then printf '%s' \"${FINAL_REF}\"; else : > \"${REF_STATE}\"; printf '%s' \"${INITIAL_REF}\"; fi ;;\n"
                    "  *'/compare/'*) printf '%s' \"${COMPARE}\" ;;\n"
                    "  *'/contents/'*'ref=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'*) printf '%s' \"${SOURCE_BLOB}\" ;;\n"
                    "  *'/contents/'*'ref=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'*) printf '%s' \"${HEAD_BLOB}\" ;;\n"
                    "  *'/contents/'*) printf '%s' \"${TIP_BLOB}\" ;;\n"
                    "  *'repos/owner/repository'*) printf '%s' '{\"default_branch\":\"master\"}' ;;\n"
                    "  *) exit 91 ;;\nesac\n",
                    encoding="utf-8",
                ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                compare = override.get("COMPARE", {"status": "ahead", "base_commit": {"sha": source_base}, "merge_base_commit": {"sha": source_base}, "head_commit": {"sha": tip}})
                initial_payload = override.get("INITIAL", case_initial)
                final = override.get("FINAL", case_initial)
                final_tip = override.get("FINAL_TIP", tip)
                environment = os.environ | {
                    "EVENT_NAME": "pull_request_target", "PR_ACTION": "opened", "PR_NUMBER": str(source_number), "PR_HEAD_SHA": head,
                    "PR_BASE_SHA": source_base, "PR_BASE_REF": override.get("PR_BASE_REF", "master"), "PR_BODY": "Fixes #64", "PR_PREVIOUS_BODY": "", "GITHUB_REPOSITORY": "owner/repository",
                    "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output), "PULLS": json.dumps(case_pulls), "INITIAL": json.dumps(initial_payload), "FINAL": json.dumps(final),
                    "INITIAL_REF": json.dumps({"object": {"sha": tip}}), "FINAL_REF": json.dumps({"object": {"sha": final_tip}}), "COMPARE": json.dumps(compare),
                    "SOURCE_BLOB": json.dumps({"sha": "c" * 40}), "HEAD_BLOB": json.dumps({"sha": "c" * 40}), "TIP_BLOB": json.dumps(override.get("TIP_BLOB", {"sha": "c" * 40})),
                    "PULL_STATE": str(directory / "pull-state"), "REF_STATE": str(directory / "ref-state"), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, expected, result.stderr)
                if expected == 0:
                    values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
                    self.assertEqual(values["event_targets"], "[72,73]")

    def test_closed_local_pull_request_target_accepts_only_a_stable_monotonic_default_base(self) -> None:
        """A delayed local close event is a no-op only after every reread agrees."""
        match = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(match); assert match is not None
        source_base, tip, head = "b" * 40, "d" * 40, "a" * 40
        local = {"id": 1, "name": "repository", "full_name": "owner/repository", "url": "https://api.github.com/repos/owner/repository"}
        initial = {"number": 72, "state": "closed", "base": {"sha": tip, "ref": "master", "repo": local}, "head": {"sha": head, "repo": local}}
        cases = {
            "accepted": ({}, 0),
            "rewind": ({"COMPARE": {"status": "behind", "base_commit": {"sha": source_base}, "merge_base_commit": {"sha": source_base}, "head_commit": {"sha": tip}}}, 1),
            "workflow": ({"TIP_BLOB": {"sha": "e" * 40}}, 1),
            "retarget": ({"FINAL": {**initial, "base": {"sha": tip, "ref": "release/v1", "repo": local}}}, 1),
            "final-tip": ({"FINAL_TIP": "e" * 40}, 1),
        }
        for name, (override, expected) in cases.items():
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
                fake.write_text(
                    "#!/bin/sh\ncase \"$*\" in\n"
                    "  *'pulls?state=open'*) printf '%s' '[[]]' ;;\n"
                    "  *'/pulls/72'*) if [ -e \"${PULL_STATE}\" ]; then printf '%s' \"${FINAL}\"; else : > \"${PULL_STATE}\"; printf '%s' \"${INITIAL}\"; fi ;;\n"
                    "  *'/git/ref/heads/master'*) if [ -e \"${REF_STATE}\" ]; then printf '%s' \"${FINAL_REF}\"; else : > \"${REF_STATE}\"; printf '%s' \"${INITIAL_REF}\"; fi ;;\n"
                    "  *'/compare/'*) printf '%s' \"${COMPARE}\" ;;\n"
                    "  *'/contents/'*'ref=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'*) printf '%s' \"${SOURCE_BLOB}\" ;;\n"
                    "  *'/contents/'*'ref=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'*) printf '%s' \"${HEAD_BLOB}\" ;;\n"
                    "  *'/contents/'*) printf '%s' \"${TIP_BLOB}\" ;;\n"
                    "  *'repos/owner/repository'*) printf '%s' \"${REPOSITORY}\" ;;\n"
                    "  *) exit 91 ;;\nesac\n",
                    encoding="utf-8",
                )
                fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                compare = override.get("COMPARE", {"status": "ahead", "base_commit": {"sha": source_base}, "merge_base_commit": {"sha": source_base}, "head_commit": {"sha": tip}})
                environment = os.environ | {
                    "EVENT_NAME": "pull_request_target", "PR_ACTION": "closed", "PR_NUMBER": "72", "PR_HEAD_SHA": head,
                    "PR_BASE_SHA": source_base, "PR_BASE_REF": "master", "PR_BODY": "Fixes #64", "PR_PREVIOUS_BODY": "",
                    "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output),
                    "INITIAL": json.dumps(initial), "FINAL": json.dumps(override.get("FINAL", initial)), "REPOSITORY": json.dumps({**local, "default_branch": "master"}),
                    "INITIAL_REF": json.dumps({"object": {"sha": tip}}), "FINAL_REF": json.dumps({"object": {"sha": override.get("FINAL_TIP", tip)}}),
                    "COMPARE": json.dumps(compare), "SOURCE_BLOB": json.dumps({"sha": "c" * 40}), "HEAD_BLOB": json.dumps({"sha": "c" * 40}), "TIP_BLOB": json.dumps(override.get("TIP_BLOB", {"sha": "c" * 40})),
                    "PULL_STATE": str(directory / "pull-state"), "REF_STATE": str(directory / "ref-state"), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, expected, result.stderr)
                if expected == 0:
                    values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
                    self.assertEqual(values["reconcile"], "true")

    def test_pull_request_target_historical_fork_base_is_a_stable_no_op(self) -> None:
        """A historical default-base event must not pin the barrier for an explicit fork."""
        match = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(match); assert match is not None
        source_base, tip, head = "b" * 40, "d" * 40, "a" * 40
        local = {"full_name": "owner/repository"}

        def run_case(
            label: str, initial_head_repository: object, final_head_repository: object,
            final_base: dict[str, object] | None = None, mode: str = "",
            expected: int = 0, final_tip: str | None = None,
            expected_state_files: tuple[str, ...] = (), number: int = 72,
            final_number: object | None = None,
        ) -> None:
            with self.subTest(case=label), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); fake = directory / "gh"; output = directory / "output"; log = directory / "gh.log"
                initial = {
                    "number": number, "state": "open",
                    "base": {"sha": tip, "ref": "master", "repo": local},
                    "head": {"sha": head, "repo": initial_head_repository},
                }
                final = {
                    **initial,
                    "number": number if final_number is None else final_number,
                    "base": final_base if final_base is not None else initial["base"],
                    "head": {"sha": head, "repo": final_head_repository},
                }
                fake.write_text(
                    "#!/bin/sh\ncase \"$*\" in\n"
                    # Keep the exact source binding ahead of the paginated
                    # open-PR pattern, then mark its final re-read separately.
                    "  *'/pulls/'\"${PR_NUMBER}\"*) if [ -e \"${PULL_STATE}\" ]; then : > \"${FINAL_PULL_STATE}\"; [ \"${MODE}\" = final-pull-failure ] && exit 7; printf '%s' \"${FINAL}\"; else : > \"${PULL_STATE}\"; printf '%s' \"${INITIAL}\"; fi ;;\n"
                    "  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n"
                    # REF_STATE drives the first T and final U ref reads.
                    "  *'/git/ref/heads/master'*) [ \"${MODE}\" = ref-failure ] && exit 7; if [ -e \"${REF_STATE}\" ]; then : > \"${FINAL_REF_STATE}\"; printf '%s' \"${FINAL_REF}\"; else : > \"${REF_STATE}\"; printf '%s' \"${INITIAL_REF}\"; fi ;;\n"
                    "  *'/contents/'*|*'/compare/'*) echo unexpected-workflow-proof >> \"${GH_LOG}\"; exit 91 ;;\n"
                    # This endpoint is reached only by the final repository re-read.
                    "  *'repos/owner/repository'*) : > \"${FINAL_REPOSITORY_STATE}\"; [ \"${MODE}\" = final-repository-failure ] && exit 7; if [ \"${MODE}\" = final-default-branch-drift ]; then printf '%s' '{\"default_branch\":\"release/v1\"}'; else printf '%s' '{\"default_branch\":\"master\"}'; fi ;;\n"
                    "  *) exit 92 ;;\nesac\n",
                    encoding="utf-8",
                )
                fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                final_tip_value = final_tip if final_tip is not None else (final["base"]["sha"] if isinstance(final["base"], dict) else tip)
                environment = os.environ | {
                    "EVENT_NAME": "pull_request_target", "PR_ACTION": "opened", "PR_NUMBER": str(number), "PR_HEAD_SHA": head,
                    "PR_BASE_SHA": source_base, "PR_BASE_REF": "master", "PR_BODY": "Fixes #64", "PR_PREVIOUS_BODY": "",
                    "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output),
                    "INITIAL": json.dumps(initial), "FINAL": json.dumps(final), "PULLS": "[[]]",
                    "INITIAL_REF": json.dumps({"object": {"sha": tip}}), "FINAL_REF": json.dumps({"object": {"sha": final_tip_value}}),
                    "PULL_STATE": str(directory / "pull-state"), "FINAL_PULL_STATE": str(directory / "final-pull-state"),
                    "REF_STATE": str(directory / "ref-state"), "FINAL_REF_STATE": str(directory / "final-ref-state"),
                    "FINAL_REPOSITORY_STATE": str(directory / "final-repository-state"), "GH_LOG": str(log), "MODE": mode,
                    "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, expected, result.stderr)
                for state_file in expected_state_files:
                    self.assertTrue((directory / state_file).exists(), state_file)
                if expected == 0:
                    values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
                    self.assertEqual(values["reconcile"], "true")
                    self.assertEqual(values["event_targets"], "[]")
                    self.assertEqual(values["priority_targets"], "[]")
                    self.assertFalse(log.exists(), log.read_text(encoding="utf-8") if log.exists() else "")
                    self.assertTrue((directory / "final-pull-state").exists())
                    self.assertTrue((directory / "final-ref-state").exists())
                    self.assertTrue((directory / "final-repository-state").exists())

        for label, head_repository in (
            ("fork", {"full_name": "fork/repository", "id": 202}),
            ("deleted-fork", None),
        ):
            run_case(label, head_repository, head_repository)

        run_case("malformed-fork", {"full_name": "fork/repository"}, {"full_name": "fork/repository"}, expected=1)
        run_case("fork-head-race", {"full_name": "fork/repository", "id": 202}, {"full_name": "fork/repository", "id": 203}, expected=1)
        run_case("default-ref-race", {"full_name": "fork/repository", "id": 202}, {"full_name": "fork/repository", "id": 202}, {"sha": tip, "ref": "release/v1", "repo": local}, expected=1)
        final_reads = ("final-pull-state", "final-ref-state", "final-repository-state")
        run_case("default-tip-race", {"full_name": "fork/repository", "id": 202}, {"full_name": "fork/repository", "id": 202}, final_tip="e" * 40, expected=1, expected_state_files=final_reads)
        run_case("final-repository-api-failure", {"full_name": "fork/repository", "id": 202}, {"full_name": "fork/repository", "id": 202}, mode="final-repository-failure", expected=1, expected_state_files=("final-repository-state",))
        run_case("final-repository-default-branch-drift", {"full_name": "fork/repository", "id": 202}, {"full_name": "fork/repository", "id": 202}, mode="final-default-branch-drift", expected=1, expected_state_files=final_reads)
        run_case("final-pull-api-failure", {"full_name": "fork/repository", "id": 202}, {"full_name": "fork/repository", "id": 202}, mode="final-pull-failure", expected=1, expected_state_files=("final-repository-state", "final-pull-state"))
        run_case("default-tip-api-failure", {"full_name": "fork/repository", "id": 202}, {"full_name": "fork/repository", "id": 202}, mode="ref-failure", expected=1)
        run_case("final-number-boolean", {"full_name": "fork/repository", "id": 202}, {"full_name": "fork/repository", "id": 202}, expected=1, number=1, final_number=True)

    def test_dispatcher_rejects_duplicate_foreign_pr_across_pages(self) -> None:
        match = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(match); assert match is not None
        fork = {"number": 73, "state": "open", "body": "Fixes #64", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"repo": {"full_name": "fork/repository"}}}
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
            fake.write_text("#!/bin/sh\nprintf '%s' \"${PULLS}\"\n", encoding="utf-8"); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            first_page = [{**fork, "number": number} for number in range(1, 101)]
            environment = os.environ | {"EVENT_NAME": "issues", "ISSUE_NUMBER": "64", "ISSUE_PULL_REQUEST_URL": "", "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output), "PULLS": json.dumps([first_page, [fork]]), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}"}
            result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=environment, capture_output=True, text=True, check=False)
            self.assertNotEqual(result.returncode, 0)

    def test_only_dispatcher_can_issue_synchronous_pending_invalidation(self) -> None:
        self.assertIn("external_id=f\"krr-governance/v1/{head.lower()}/dispatcher-{dispatcher}\"", self.workflow)
        self.assertNotIn("/statuses/", self.workflow)

    def test_relevant_event_is_read_only_until_singleton_reconciles_every_current_local_pr(self) -> None:
        resolver = self.workflow[
            self.workflow.index("  resolve_event:"):
            self.workflow.index("  reconcile-all-open:")
        ]
        self.assertIn(
            "concurrency:\n      group: pr-governance-dispatcher-${{ github.repository_id }}\n      cancel-in-progress: ${{ needs.establish-resolver-failure-barrier.outputs.priority == 'true' }}",
            resolver,
        )
        self.assertIn("reconcile: ${{ steps.targets.outputs.reconcile }}", resolver)
        self.assertIn("if: needs.resolve_event.outputs.reconcile == 'true'", self.workflow)
        self.assertIn("group: pr-governance-dispatcher-${{ github.repository_id }}", self.workflow)
        self.assertIn("AFFECTED: ${{ steps.current-targets.outputs.all_invalidation_chunk_1 }}", self.workflow)
        reconcile_start = self.workflow.index("  reconcile-all-open:")
        reconcile_job = self.workflow[reconcile_start:self.workflow.index("    concurrency:", reconcile_start)]
        self.assertIn(
            "if: needs.resolve_event.outputs.reconcile == 'true' && github.run_attempt == 1",
            reconcile_job,
        )
        self.assertLess(
            reconcile_job.index("github.run_attempt == 1"),
            reconcile_job.index("environment: pr-governance"),
        )
        match = re.search(
            r"- name: Re-enumerate every current local governance pull request.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        local_base = {"ref": "master", "repo": {"full_name": "owner/repository"}}
        pulls = [[
            {"number": 64, "state": "open", "body": "Fixes #1", "draft": False, "base": local_base, "head": {"sha": "d" * 40, "repo": {"full_name": "owner/repository"}}},
            {"number": 65, "state": "open", "body": "Fixes #2", "draft": False, "base": local_base, "head": {"sha": "e" * 40, "repo": {"full_name": "owner/repository"}}},
            {"number": 66, "state": "open", "body": "Fixes #3", "draft": False, "base": local_base, "head": {"repo": {"full_name": "fork/repository"}}},
            # Deleted/unavailable fork metadata is outside this repository's
            # governance domain and must not fail the all-open local scan.
            {"number": 67, "state": "open", "body": "Fixes #4", "draft": False, "base": local_base, "head": {"repo": None}},
        ]]
        duplicate_page = [pulls[0][0]] + [{**pulls[0][0], "number": number} for number in range(100, 199)]
        for pages, expected in ((pulls, 0), ([duplicate_page, [pulls[0][0]]], 1)):
            with self.subTest(duplicate=expected == 1), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
                fake.write_text(
                    "#!/bin/sh\ncase \"$*\" in\n"
                    "  *'git/ref/heads/master'*) printf '%s' '{\"object\":{\"sha\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}}' ;;\n"
                    "  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n"
                    "  *'repos/owner/repository'*) printf '%s' '{\"default_branch\":\"master\"}' ;;\n"
                    "  *) exit 91 ;;\nesac\n",
                    encoding="utf-8",
                ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "GITHUB_OUTPUT": str(output),
                    "EVENT_TARGETS": "[64,65]", "EVENT_PRIORITY_TARGETS": "[64]", "PULLS": json.dumps(pages),
                    "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, expected, result.stderr)
                if expected == 0:
                    values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
                    self.assertEqual(values["has_targets"], "true")
                    self.assertEqual(json.loads(values["targets"]), [64, 65])
                    self.assertEqual(json.loads(values["event_targets"]), [64, 65])
                    self.assertEqual(json.loads(values["priority_targets"]), [64])
                    self.assertEqual(json.loads(values["preinvalidate_targets"]), [64])
                    self.assertEqual(json.loads(values["preinvalidate_chunk_1"]), [64])
                    self.assertEqual(json.loads(values["preinvalidate_chunk_2"]), [])
                    self.assertEqual(json.loads(values["preinvalidated_heads"]), ["d" * 40])
                    self.assertEqual(json.loads(values["all_invalidation_targets"]), [65])
                    self.assertEqual(json.loads(values["all_invalidation_chunk_1"]), [65])
                    self.assertEqual(json.loads(values["all_invalidation_chunk_2"]), [])
                    self.assertEqual(values["duplicate_governed_heads"], "[]")
                    self.assertEqual(values["writer_head"], "a" * 40)
                    self.assertEqual(values["default_branch"], "master")

    def test_workflow_run_fork_scope_is_resolved_before_barrier_mutation(self) -> None:
        """Fork workflow_run sources must be a pre-barrier no-op, not a late failure."""
        scope_match = re.search(
            r"- name: Exclude unavailable fork sources before dispatcher lock.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(scope_match); assert scope_match is not None
        preflight = self.workflow[
            self.workflow.index("  preflight-workflow-run-source:"):
            self.workflow.index("  establish-resolver-failure-barrier:")
        ]
        self.assertNotIn("environment:", preflight)
        self.assertNotIn("secrets.", preflight)
        self.assertNotIn("concurrency:", preflight)
        self.assertRegex(
            preflight,
            r"permissions:\n      actions: read\n      contents: read\n      pull-requests: read",
        )
        establish = self.workflow[
            self.workflow.index("  establish-resolver-failure-barrier:"):
            self.workflow.index("  resolve_event:")
        ]
        self.assertIn("needs: preflight-workflow-run-source", establish)
        self.assertIn("needs.preflight-workflow-run-source.outputs.reconcile == 'true'", establish)
        self.assertIn("needs.preflight-workflow-run-source.outputs.valid", establish)
        self.assertLess(establish.index("concurrency:"), establish.index("Create resolver-failure barrier marker write token"))
        self.assertLess(establish.index("Activate resolver-failure merge barrier"), establish.index("Fail closed after classification barrier activation"))
        self.assertIn("EVENT_SOURCE_VALID: ${{ needs.preflight-workflow-run-source.outputs.valid }}", establish)
        self.assertEqual(preflight.count("WORKFLOW_RUN_ATTEMPT: ${{ github.event.workflow_run.run_attempt }}"), 1)
        self.assertIn("needs: establish-resolver-failure-barrier", self.workflow[self.workflow.index("  resolve_event:"):])
        self.assertIn("needs: resolve_event", self.workflow[self.workflow.index("  reconcile-all-open:"):])
        marker = self.workflow.index("- name: Create resolver-failure barrier marker write token")
        scope = self.workflow.index("- name: Exclude unavailable fork sources before dispatcher lock")
        self.assertLess(scope, marker)
        resolver = self.workflow[
            self.workflow.index("  resolve_event:"):
            self.workflow.index("  reconcile-all-open:")
        ]
        self.assertIn("DEFAULT_BRANCH: ${{ needs.establish-resolver-failure-barrier.outputs.default_branch }}", resolver)
        self.assertNotIn("DEFAULT_BRANCH: ${{ github.event.repository.default_branch }}", resolver)

        def run_scope(
            run: dict[str, object], pull: dict[str, object] | None,
            expected_reconcile: str, expected_valid: str, mode: str = "",
            expected_priority: str = "true", trigger_action: str = "completed",
            source_attempt: str = "1", base_blob: str | None = None, head_blob: str | None = None,
            tip_blob: str | None = None, default_tip: str | None = None,
            final_tip: str | None = None, comparison: str | None = None,
        ) -> None:
            with tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary)
                output = directory / "scope-output"
                fake = directory / "gh"
                current_base = pull.get("base") if isinstance(pull, dict) else None
                current_base_sha = current_base.get("sha") if isinstance(current_base, dict) else None
                initial_tip = default_tip if default_tip is not None else (current_base_sha if isinstance(current_base_sha, str) else "b" * 40)
                self.assertIsInstance(initial_tip, str)
                assert isinstance(initial_tip, str)
                stable_tip = final_tip if final_tip is not None else initial_tip
                compare_payload = comparison if comparison is not None else json.dumps({
                    "status": "identical" if initial_tip == "b" * 40 else "ahead",
                    "base_commit": {"sha": "b" * 40},
                    "merge_base_commit": {"sha": "b" * 40},
                    "head_commit": {"sha": initial_tip},
                })
                fake.write_text(
                    "#!/bin/sh\ncase \"$*\" in\n"
                    "  *'/actions/runs/9'*) [ \"${MODE}\" = api-failure ] && exit 7; [ \"${MODE}\" = invalid-json ] && printf '%s' '{'; printf '%s' \"${RUN}\" ;;\n"
                    "  *'/pulls/'*) [ \"${MODE}\" = api-failure ] && exit 7; [ \"${MODE}\" = invalid-json ] && printf '%s' '{'; printf '%s' \"${PULL}\" ;;\n"
                    "  *'/git/ref/heads/master'*) if [ -e \"${REF_STATE}\" ]; then printf '%s' \"${FINAL_TIP}\"; else : > \"${REF_STATE}\"; printf '%s' \"${INITIAL_TIP}\"; fi ;;\n"
                    "  *'/compare/'*) printf '%s' \"${COMPARISON}\" ;;\n"
                    "  *'/contents/'*'ref=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'*) [ \"${MODE}\" = api-failure ] && exit 7; printf '%s' \"${BASE_BLOB}\" ;;\n"
                    "  *'/contents/'*'ref=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'*) [ \"${MODE}\" = api-failure ] && exit 7; printf '%s' \"${HEAD_BLOB}\" ;;\n"
                    "  *'/contents/'*) [ \"${MODE}\" = api-failure ] && exit 7; printf '%s' \"${TIP_BLOB}\" ;;\n"
                    "  *'repos/owner/repository'*) printf '%s' '{\"default_branch\":\"master\"}' ;;\n"
                    "  *) exit 91 ;;\n"
                    "esac\n",
                    encoding="utf-8",
                )
                fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository",
                    "EVENT_NAME": "workflow_run",
                    "EVENT_ACTION": trigger_action,
                    "WORKFLOW_RUN_ID": "9",
                    "WORKFLOW_RUN_ATTEMPT": source_attempt,
                    "DEFAULT_BRANCH": "master",
                    "GITHUB_OUTPUT": str(output),
                    "RUN": json.dumps(run),
                    "PULL": json.dumps(pull),
                    "BASE_BLOB": json.dumps({"sha": "c" * 40}) if base_blob is None else base_blob,
                    "HEAD_BLOB": json.dumps({"sha": "c" * 40}) if head_blob is None else head_blob,
                    "TIP_BLOB": json.dumps({"sha": "c" * 40}) if tip_blob is None else tip_blob,
                    "INITIAL_TIP": json.dumps({"object": {"sha": initial_tip}}),
                    "FINAL_TIP": json.dumps({"object": {"sha": stable_tip}}),
                    "COMPARISON": compare_payload,
                    "REF_STATE": str(directory / "ref-state"),
                    "MODE": mode,
                    "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run(
                    [sys.executable, "-c", self._workflow_program(scope_match)],
                    env=environment,
                    capture_output=True,
                    text=True,
                    check=False,
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
                self.assertEqual(values["reconcile"], expected_reconcile)
                self.assertEqual(values["valid"], expected_valid)
                self.assertEqual(values["priority"], expected_priority)
                self.assertNotIn("\\n", output.read_text(encoding="utf-8"))

        def workflow_run(
            name: str, event: str, status: str = "completed",
            head_repository: dict[str, str] | None = None, deleted_head: bool = False,
        ) -> dict[str, object]:
            paths = {
                "PR governance review sensor": ".github/workflows/pr-governance-review-events.yml",
                "CI": ".github/workflows/test-and-build.yml",
                "release-preflight": ".github/workflows/release-preflight.yml",
            }
            base = {
                "ref": "master", "sha": "b" * 40,
                "repo": {"id": 101, "name": "repository", "url": "https://api.github.com/repos/owner/repository"},
            }
            current_head_repository = {"full_name": "owner/repository", "id": 101} if head_repository is None else head_repository
            source_head_repository = None if deleted_head else {
                "id": current_head_repository["id"],
                "name": current_head_repository["full_name"].rsplit("/", 1)[1],
                "url": f"https://api.github.com/repos/{current_head_repository['full_name']}",
            }
            head = {
                "sha": "a" * 40,
                "repo": source_head_repository,
            }
            return {
                "name": name,
                "path": paths[name] + "@master",
                "event": event,
                "status": status,
                "id": 9,
                "run_number": 1,
                "run_attempt": 1,
                "head_sha": "a" * 40,
                "repository": {"id": 101, "full_name": "owner/repository"},
                "pull_requests": [{"number": 72, "base": base, "head": head}],
            }

        local_pull = {
            "number": 72,
            "state": "open",
            "base": {"ref": "master", "sha": "b" * 40, "repo": {"full_name": "owner/repository", "id": 101}},
            "head": {"sha": "a" * 40, "repo": {"full_name": "owner/repository", "id": 101}},
        }
        fork_pull = {
            **local_pull,
            "head": {"sha": "a" * 40, "repo": {"full_name": "fork/repository", "id": 202}},
        }
        unavailable_fork_pull = {**local_pull, "head": {"sha": "a" * 40, "repo": None}}

        for name, event in (
            ("CI", "pull_request"),
            ("release-preflight", "pull_request"),
            ("PR governance review sensor", "pull_request"),
            ("PR governance review sensor", "pull_request_review"),
            ("PR governance review sensor", "pull_request_review_comment"),
        ):
            with self.subTest(name=name, event=event, source="fork"):
                run_scope(workflow_run(name, event, head_repository={"full_name": "fork/repository", "id": 202}), fork_pull, "false", "true")
            with self.subTest(name=name, event=event, source="deleted-fork"):
                run_scope(workflow_run(name, event, deleted_head=True), unavailable_fork_pull, "false", "true")
            with self.subTest(name=name, event=event, source="local"):
                expected_priority = "false" if name in {"CI", "release-preflight"} else "true"
                run_scope(workflow_run(name, event), local_pull, "true", "true", expected_priority=expected_priority)

        rerun_sensor = workflow_run("PR governance review sensor", "pull_request_review")
        rerun_sensor["run_attempt"] = 2
        run_scope(
            rerun_sensor,
            local_pull,
            "true",
            "false",
            source_attempt="2",
        )
        for name in ("CI", "release-preflight"):
            with self.subTest(name=name, source="rerun"):
                retry = workflow_run(name, "pull_request")
                retry["run_attempt"] = 2
                run_scope(
                    retry,
                    local_pull,
                    "true",
                    "true",
                    expected_priority="false",
                    source_attempt="2",
                )

        # A valid local CI/release run is classified before either shared
        # dispatcher lock. The trigger action remains the identity, while the
        # API re-read may already have moved to a later lifecycle status.
        # Every allowed pair must join the serial lane without cancellation.
        for name in ("CI", "release-preflight"):
            for trigger_action in ("requested", "in_progress", "completed"):
                for status in ("requested", "queued", "waiting", "pending", "in_progress", "completed"):
                    with self.subTest(name=name, trigger_action=trigger_action, status=status, source="local"):
                        run_scope(
                            workflow_run(name, "pull_request", status), local_pull,
                            "true", "true", expected_priority="false", trigger_action=trigger_action,
                        )
        advanced_base_pull = {
            **local_pull,
            "base": {**local_pull["base"], "sha": "d" * 40},
        }
        run_scope(
            workflow_run("CI", "pull_request"), advanced_base_pull,
            "true", "true", expected_priority="false", trigger_action="completed",
        )
        # Unknown lifecycle metadata must retain preemption instead of silently
        # joining the normal lane on an unverified source classification.
        run_scope(
            workflow_run("CI", "pull_request", "unknown"), local_pull,
            "true", "false", expected_priority="true", trigger_action="completed",
        )
        run_scope(
            workflow_run("CI", "pull_request", "completed"), local_pull,
            "true", "false", expected_priority="true", trigger_action="unknown",
        )

        # A normal-lane classification is safe only after the same source
        # binding as the resolver. Each drift must arm the failure barrier and
        # retain preemption rather than bypassing a paced reconciliation.
        normal = workflow_run("CI", "pull_request")
        drift_cases = (
            ("path", {**normal, "path": ".github/workflows/other.yml@master"}, local_pull, {}),
            ("repository", {**normal, "repository": {"full_name": "fork/repository"}}, local_pull, {}),
            ("run-id", {**normal, "id": 10}, local_pull, {}),
            ("run-attempt", {**normal, "run_attempt": 2}, local_pull, {}),
            ("head", {**normal, "head_sha": "d" * 40}, local_pull, {}),
            ("workflow-blob", normal, local_pull, {"head_blob": json.dumps({"sha": "d" * 40})}),
            ("current-default-blob", normal, advanced_base_pull, {"default_tip": "d" * 40, "tip_blob": json.dumps({"sha": "e" * 40})}),
            ("current-pr-base-not-tip", normal, local_pull, {"default_tip": "d" * 40}),
            ("base-no-longer-reaches-tip", normal, local_pull, {"comparison": json.dumps({
                "status": "diverged", "base_commit": {"sha": "b" * 40},
                "merge_base_commit": {"sha": "c" * 40}, "head_commit": {"sha": "d" * 40},
            }), "default_tip": "d" * 40}),
            ("default-ref-race", normal, local_pull, {"final_tip": "d" * 40}),
        )
        for label, run, pull, options in drift_cases:
            with self.subTest(source=label):
                run_scope(run, pull, "true", "false", expected_priority="true", **options)

        # Ambiguous or malformed metadata arms the barrier but marks the
        # source invalid; the later fence must fail closed before resolution.
        malformed_sources = (
            ("missing-pull-requests", {key: value for key, value in workflow_run("CI", "pull_request").items() if key != "pull_requests"}, local_pull),
            ("empty-pull-requests", {**workflow_run("CI", "pull_request"), "pull_requests": []}, local_pull),
            ("multiple-pull-requests", {**workflow_run("CI", "pull_request"), "pull_requests": [{"number": 72}, {"number": 73}]}, local_pull),
            ("missing-source-number", {**workflow_run("CI", "pull_request"), "pull_requests": [{}]}, local_pull),
            ("missing-head", workflow_run("CI", "pull_request"), {key: value for key, value in local_pull.items() if key != "head"}),
            ("head-not-object", workflow_run("CI", "pull_request"), {**local_pull, "head": "invalid"}),
            ("head-repo-key-missing", workflow_run("CI", "pull_request"), {**local_pull, "head": {"sha": "a" * 40}}),
            ("head-repo-wrong-type", workflow_run("CI", "pull_request"), {**local_pull, "head": {"sha": "a" * 40, "repo": "invalid"}}),
            ("malformed-local-head", workflow_run("CI", "pull_request"), {**local_pull, "head": {"sha": "a" * 40, "repo": {}}}),
            ("empty-foreign-name", workflow_run("CI", "pull_request"), {**local_pull, "head": {"sha": "a" * 40, "repo": {"full_name": ""}}}),
            ("whitespace-foreign-name", workflow_run("CI", "pull_request"), {**local_pull, "head": {"sha": "a" * 40, "repo": {"full_name": " "}}}),
            ("nul-foreign-name", workflow_run("CI", "pull_request"), {**local_pull, "head": {"sha": "a" * 40, "repo": {"full_name": "fork/\x00repo"}}}),
            ("extra-segment-foreign-name", workflow_run("CI", "pull_request"), {**local_pull, "head": {"sha": "a" * 40, "repo": {"full_name": "owner/repo/extra"}}}),
            ("invalid-slug-foreign-name", workflow_run("CI", "pull_request"), {**local_pull, "head": {"sha": "a" * 40, "repo": {"full_name": "fork repo"}}}),
            ("boolean-source-number", {**workflow_run("CI", "pull_request"), "pull_requests": [{"number": True}]}, local_pull),
            ("boolean-pull-number", {**workflow_run("CI", "pull_request"), "pull_requests": [{"number": 1}]}, {**local_pull, "number": True}),
            ("non-default-base", workflow_run("CI", "pull_request"), {**local_pull, "base": {"ref": "release/v0.4", "repo": {"full_name": "owner/repository"}}}),
        )
        for label, run, pull in malformed_sources:
            with self.subTest(source=label):
                run_scope(run, pull, "true", "false")
        for mode in ("api-failure", "invalid-json"):
            with self.subTest(source=mode):
                run_scope(workflow_run("CI", "pull_request"), local_pull, "true", "false", mode)

    def test_closed_local_workflow_run_is_reread_before_prebarrier_noop(self) -> None:
        """Only a stable, local, closed source may skip the resolver barrier."""
        scope_match = re.search(
            r"- name: Exclude unavailable fork sources before dispatcher lock.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(scope_match); assert scope_match is not None
        repository = "owner/repository"; base = "b" * 40; head = "a" * 40
        api_repository = {
            "id": 101, "name": "repository",
            "url": "https://api.github.com/repos/owner/repository",
        }
        run_repository = {"id": 101, "full_name": repository}
        source_base = {"ref": "master", "sha": base, "repo": api_repository}
        source_head = {"sha": head, "repo": api_repository}
        initial_run = {
            "id": 9, "name": "CI", "path": ".github/workflows/test-and-build.yml@master",
            "event": "pull_request", "status": "completed", "run_number": 1, "run_attempt": 1,
            "head_sha": head, "repository": run_repository,
            "pull_requests": [{"number": 72, "base": source_base, "head": source_head}],
        }
        initial_pull = {
            "number": 72, "state": "closed",
            "base": {"ref": "master", "sha": base, "repo": {"id": 101, "full_name": repository}},
            "head": {"sha": head, "repo": {"id": 101, "full_name": repository}},
        }

        cases = (
            ("stable", {}, {}, {}, "false", "true"),
            ("final-run-attempt-race", {"run_attempt": 2}, {}, {}, "true", "false"),
            ("final-run-head-race", {"head_sha": "c" * 40}, {}, {}, "true", "false"),
            ("final-source-repository-race", {"pull_requests": [{"number": 72, "base": source_base, "head": {"sha": head, "repo": {**api_repository, "id": 102}}}]}, {}, {}, "true", "false"),
            ("final-pull-head-race", {}, {"head": {"sha": "c" * 40, "repo": {"id": 101, "full_name": repository}}}, {}, "true", "false"),
            ("retargeted-pull", {}, {"base": {"ref": "release/v0.4", "sha": base, "repo": {"id": 101, "full_name": repository}}}, {}, "true", "false"),
            ("reopened-pull", {}, {"state": "open"}, {}, "true", "false"),
            ("default-branch-race", {}, {}, {"default_branch": "release/v0.4"}, "true", "false"),
            ("boolean-final-source-number", {"pull_requests": [{"number": True, "base": source_base, "head": source_head}]}, {}, {}, "true", "false"),
            ("boolean-final-pull-number", {}, {"number": True}, {}, "true", "false"),
        )
        for label, run_change, pull_change, final_repository, expected_reconcile, expected_valid in cases:
            with self.subTest(case=label), tempfile.TemporaryDirectory() as temporary:
                output = Path(temporary) / "scope-output"; run_reads = 0; pull_reads = 0; repository_reads = 0
                number = 1 if label.startswith("boolean-") else 72
                case_run = {**initial_run, "pull_requests": [{"number": number, "base": source_base, "head": source_head}]}
                case_pull = {**initial_pull, "number": number}
                final_run = {**case_run, **run_change}
                final_pull = {**case_pull, **pull_change}

                def response(value: object) -> subprocess.CompletedProcess[str]:
                    return subprocess.CompletedProcess([], 0, json.dumps(value), "")

                def fake_run(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
                    nonlocal run_reads, pull_reads, repository_reads
                    endpoint = arguments[-1]
                    if endpoint == f"repos/{repository}":
                        repository_reads += 1
                        return response({"default_branch": "master"} if repository_reads == 1 else {"default_branch": "master"} | final_repository)
                    if endpoint == f"repos/{repository}/actions/runs/9":
                        run_reads += 1
                        return response(case_run if run_reads == 1 else final_run)
                    if endpoint == f"repos/{repository}/pulls/{number}":
                        pull_reads += 1
                        return response(case_pull if pull_reads == 1 else final_pull)
                    raise AssertionError(arguments)

                environment = os.environ | {
                    "GITHUB_REPOSITORY": repository, "EVENT_NAME": "workflow_run", "EVENT_ACTION": "completed",
                    "WORKFLOW_RUN_ID": "9", "WORKFLOW_RUN_ATTEMPT": "1", "GITHUB_OUTPUT": str(output),
                }
                with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run):
                    exec(self._workflow_program(scope_match), {"__name__": "__main__"})
                values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
                self.assertEqual(values["reconcile"], expected_reconcile)
                self.assertEqual(values["valid"], expected_valid)
                self.assertEqual(values["priority"], "true")
                self.assertEqual(values["pull_request_target_noop"], "false")
                self.assertEqual(run_reads, 2)
                self.assertEqual(pull_reads, 2)
                self.assertEqual(repository_reads, 2)

    def test_closed_foreign_workflow_run_is_reread_before_prebarrier_noop(self) -> None:
        """Stable closed fork identities are out of scope; every drift stays fail-closed."""
        scope_match = re.search(
            r"- name: Exclude unavailable fork sources before dispatcher lock.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(scope_match); assert scope_match is not None
        repository = "owner/repository"; base = "b" * 40; head = "a" * 40
        base_repository = {"id": 101, "name": "repository", "url": "https://api.github.com/repos/owner/repository"}
        run_repository = {"id": 101, "full_name": repository}
        foreign_source = {"id": 202, "name": "repository", "url": "https://api.github.com/repos/fork/repository"}
        foreign_pull = {"id": 202, "full_name": "fork/repository"}

        def make_run(
            source_head_repository: object, name: str = "CI", event: str = "pull_request",
        ) -> dict[str, object]:
            paths = {
                "CI": ".github/workflows/test-and-build.yml",
                "PR governance review sensor": ".github/workflows/pr-governance-review-events.yml",
            }
            return {
                "id": 9, "name": name, "path": paths[name] + "@master",
                "event": event, "status": "completed", "run_number": 1, "run_attempt": 1,
                "head_sha": head, "repository": run_repository,
                "pull_requests": [{
                    "number": 72,
                    "base": {"ref": "master", "sha": base, "repo": base_repository},
                    "head": {"sha": head, "repo": source_head_repository},
                }],
            }

        def make_pull(head_repository: object) -> dict[str, object]:
            return {
                "number": 72, "state": "closed",
                "base": {"ref": "master", "sha": base, "repo": {"id": 101, "full_name": repository}},
                "head": {"sha": head, "repo": head_repository},
            }

        cases = (
            ("foreign-stable", foreign_source, foreign_pull, foreign_source, foreign_pull, {}, {}, {}, "false", "true"),
            ("deleted-stable", None, None, None, None, {}, {}, {}, "false", "true"),
            ("foreign-target-id-collision", {**foreign_source, "id": 101}, {**foreign_pull, "id": 101}, {**foreign_source, "id": 101}, {**foreign_pull, "id": 101}, {}, {}, {}, "true", "false"),
            ("foreign-source-identity-drift", foreign_source, foreign_pull, {**foreign_source, "id": 203}, foreign_pull, {}, {}, {}, "true", "false"),
            ("foreign-pull-identity-drift", foreign_source, foreign_pull, foreign_source, {"id": 202, "full_name": "fork/other"}, {}, {}, {}, "true", "false"),
            ("deleted-identity-drift", None, None, None, foreign_pull, {}, {}, {}, "true", "false"),
            ("run-race", foreign_source, foreign_pull, foreign_source, foreign_pull, {"run_attempt": 2}, {}, {}, "true", "false"),
            ("review-sensor-rerun", foreign_source, foreign_pull, foreign_source, foreign_pull, {"run_attempt": 2}, {}, {}, "true", "false"),
            ("repo-drift", foreign_source, foreign_pull, foreign_source, foreign_pull, {"repository": {"id": 101, "full_name": "owner/other"}}, {}, {}, "true", "false"),
            ("path-ref-drift-foreign", foreign_source, foreign_pull, foreign_source, foreign_pull, {"path": ".github/workflows/test-and-build.yml@release/v0.4"}, {}, {}, "true", "false"),
            ("path-ref-drift-deleted", None, None, None, None, {"path": ".github/workflows/test-and-build.yml@release/v0.4"}, {}, {}, "true", "false"),
            ("review-sensor-event-drift-foreign", foreign_source, foreign_pull, foreign_source, foreign_pull, {"event": "pull_request_review"}, {}, {}, "true", "false"),
            ("review-sensor-event-drift-deleted", None, None, None, None, {"event": "pull_request_review"}, {}, {}, "true", "false"),
            ("head-race", foreign_source, foreign_pull, foreign_source, foreign_pull, {}, {"head": {"sha": "c" * 40, "repo": foreign_pull}}, {}, "true", "false"),
            ("base-race", foreign_source, foreign_pull, foreign_source, foreign_pull, {}, {"base": {"ref": "master", "sha": "c" * 40, "repo": {"id": 101, "full_name": repository}}}, {}, "true", "false"),
            ("default-branch-race", foreign_source, foreign_pull, foreign_source, foreign_pull, {}, {}, {"default_branch": "release/v0.4"}, "true", "false"),
            ("state-race", foreign_source, foreign_pull, foreign_source, foreign_pull, {}, {"state": "open"}, {}, "true", "false"),
        )
        for label, initial_source_head, initial_pull_head, final_source_head, final_pull_head, run_change, pull_change, final_repository, expected_reconcile, expected_valid in cases:
            with self.subTest(case=label), tempfile.TemporaryDirectory() as temporary:
                output = Path(temporary) / "scope-output"; run_reads = 0; pull_reads = 0; repository_reads = 0
                run_name = "PR governance review sensor" if label.startswith("review-sensor-") else "CI"
                initial_run = make_run(initial_source_head, run_name)
                final_run = {**make_run(final_source_head, run_name), **run_change}
                initial_pull = make_pull(initial_pull_head)
                final_pull = {**make_pull(final_pull_head), **pull_change}

                def response(value: object) -> subprocess.CompletedProcess[str]:
                    return subprocess.CompletedProcess([], 0, json.dumps(value), "")

                def fake_run(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
                    nonlocal run_reads, pull_reads, repository_reads
                    endpoint = arguments[-1]
                    if endpoint == f"repos/{repository}":
                        repository_reads += 1
                        return response({"default_branch": "master"} if repository_reads == 1 else {"default_branch": "master"} | final_repository)
                    if endpoint == f"repos/{repository}/actions/runs/9":
                        run_reads += 1
                        return response(initial_run if run_reads == 1 else final_run)
                    if endpoint == f"repos/{repository}/pulls/72":
                        pull_reads += 1
                        return response(initial_pull if pull_reads == 1 else final_pull)
                    raise AssertionError(arguments)

                environment = os.environ | {
                    "GITHUB_REPOSITORY": repository, "EVENT_NAME": "workflow_run", "EVENT_ACTION": "completed",
                    "WORKFLOW_RUN_ID": "9", "WORKFLOW_RUN_ATTEMPT": "1", "GITHUB_OUTPUT": str(output),
                }
                with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run):
                    exec(self._workflow_program(scope_match), {"__name__": "__main__"})
                values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
                self.assertEqual(values["reconcile"], expected_reconcile)
                self.assertEqual(values["valid"], expected_valid)
                self.assertEqual(values["priority"], "true")
                self.assertEqual(values["pull_request_target_noop"], "false")
                expected_reads = 1 if label == "foreign-target-id-collision" else 2
                self.assertEqual(run_reads, expected_reads)
                self.assertEqual(pull_reads, expected_reads)
                self.assertEqual(repository_reads, expected_reads)

    def test_unchanged_fork_pull_request_target_is_excluded_before_barrier_mutation(self) -> None:
        """Only a fully bound, unchanged foreign/deleted fork may skip the shared lock."""
        scope_match = re.search(
            r"- name: Exclude unavailable fork sources before dispatcher lock.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(scope_match); assert scope_match is not None
        preflight = self.workflow[
            self.workflow.index("  preflight-workflow-run-source:"):
            self.workflow.index("  establish-resolver-failure-barrier:")
        ]
        self.assertIn("github.event_name != 'workflow_run'", preflight)
        self.assertIn("PR_BASE_SHA: ${{ github.event.pull_request.base.sha }}", preflight)
        self.assertIn("PR_BASE_REF: ${{ github.event.pull_request.base.ref }}", preflight)
        self.assertNotIn("concurrency:", preflight)
        establish = self.workflow[
            self.workflow.index("  establish-resolver-failure-barrier:"):
            self.workflow.index("  resolve_event:")
        ]
        self.assertIn("needs.preflight-workflow-run-source.outputs.reconcile == 'true'", establish)

        source_base, advanced_base, head = "b" * 40, "d" * 40, "a" * 40

        def run_scope(
            label: str,
            head_repository: object,
            current_base: str,
            expected_reconcile: str,
            expected_valid: str,
            expected_pull_request_target_noop: str,
        ) -> None:
            with self.subTest(label=label), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); output = directory / "scope-output"; fake = directory / "gh"
                if isinstance(head_repository, dict) and isinstance(head_repository.get("full_name"), str):
                    head_repository = {"name": head_repository["full_name"].rsplit("/", 1)[-1], "url": f"https://api.github.com/repos/{head_repository['full_name']}", **head_repository}
                source = {
                    "number": 72,
                    "state": "open",
                    "base": {"sha": current_base, "ref": "master", "repo": {"id": 101, "name": "repository", "full_name": "owner/repository", "url": "https://api.github.com/repos/owner/repository"}},
                    "head": {"sha": head, "repo": head_repository},
                }
                fake.write_text(
                    "#!/bin/sh\ncase \"$*\" in\n"
                    "  *'repos/owner/repository') printf '%s' '{\"id\":101,\"full_name\":\"owner/repository\",\"default_branch\":\"master\"}' ;;\n"
                    "  *'/pulls/72'*) printf '%s' \"${SOURCE}\" ;;\n"
                    "  *) printf '%s' '{\"id\":101,\"full_name\":\"owner/repository\",\"default_branch\":\"master\"}' ;;\nesac\n",
                    encoding="utf-8",
                )
                fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "EVENT_NAME": "pull_request_target", "EVENT_ACTION": "opened",
                    "PR_ACTION": "opened", "PR_NUMBER": "72", "PR_HEAD_SHA": head, "PR_BASE_SHA": source_base,
                    "PR_BASE_REF": "master", "GITHUB_OUTPUT": str(output), "SOURCE": json.dumps(source),
                    "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", self._workflow_program(scope_match)], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, 0, result.stderr)
                values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
                self.assertEqual(values["reconcile"], expected_reconcile)
                self.assertEqual(values["valid"], expected_valid)
                self.assertEqual(values["priority"], "true")
                self.assertEqual(values["pull_request_target_noop"], expected_pull_request_target_noop)

        run_scope("foreign-unchanged", {"full_name": "fork/repository", "name": "repository", "id": 202}, source_base, "false", "true", "true")
        run_scope("deleted-unchanged", None, source_base, "false", "true", "true")
        run_scope("local-unchanged", {"full_name": "owner/repository", "name": "repository", "id": 101}, source_base, "true", "true", "false")
        run_scope("malformed-foreign", {"full_name": "fork/repository"}, source_base, "true", "false", "false")
        run_scope("foreign-historical-base", {"full_name": "fork/repository", "id": 202}, advanced_base, "true", "true", "false")

    def test_workflow_run_out_of_scope_race_keeps_reconciliation_without_workflow_blob(self) -> None:
        """A resolver re-read may observe a deleted fork after scope accepted its local source."""
        scope_match = re.search(
            r"- name: Exclude unavailable fork sources before dispatcher lock.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        resolver_match = re.search(
            r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(scope_match); self.assertIsNotNone(resolver_match)
        assert scope_match is not None and resolver_match is not None
        scope_program = self._workflow_program(scope_match)
        resolver_program = self._workflow_program(resolver_match)
        base = {"sha": "b" * 40, "ref": "master", "repo": {"full_name": "owner/repository", "id": 101}}
        source_base = {"sha": "b" * 40, "ref": "master", "repo": {"id": 101, "name": "repository", "url": "https://api.github.com/repos/owner/repository"}}
        local_head = {"sha": "a" * 40, "repo": {"full_name": "owner/repository", "id": 101}}
        local_source = {"number": 72, "state": "open", "base": base, "head": local_head}
        resolver_source = {"number": 72, "base": source_base, "head": {"sha": "a" * 40, "repo": None}}
        resolver_current = {"number": 72, "state": "open", "base": base, "head": {"sha": "a" * 40, "repo": None}}
        resolver_run = {
            "name": "CI", "event": "pull_request", "status": "completed", "id": 9,
            "run_number": 1, "run_attempt": 1, "head_sha": "a" * 40,
            "path": ".github/workflows/test-and-build.yml@master",
            "repository": {"id": 101, "full_name": "owner/repository"}, "pull_requests": [resolver_source],
        }
        all_open = [[
            {**local_source, "body": "Fixes #64", "draft": False, "head": {"sha": "a" * 40, "repo": None}},
            {"number": 73, "state": "open", "body": "Fixes #65", "draft": False, "base": base, "head": {"sha": "c" * 40, "repo": {"full_name": "owner/repository", "id": 101}}},
        ]]

        def execute(program: str, run: dict[str, object], output: Path, directory: Path) -> subprocess.CompletedProcess[str]:
            return subprocess.run(
                [sys.executable, "-c", program],
                env=os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "EVENT_NAME": "workflow_run", "EVENT_ACTION": "completed",
                    "WORKFLOW_RUN_ID": "9", "WORKFLOW_RUN_ATTEMPT": "1", "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output),
                    "RUN": json.dumps(run), "PULL": json.dumps(resolver_current if program == resolver_program else local_source), "PULLS": json.dumps(all_open),
                    "GH_LOG": str(directory / "gh.log"), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                },
                capture_output=True, text=True, check=False,
            )

        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            fake = directory / "gh"
            fake.write_text(
                "#!/bin/sh\ncase \"$*\" in\n"
                "  *'/actions/runs/9'*) printf '%s' \"${RUN}\" ;;\n"
                "  *'/pulls/72'*) printf '%s' \"${PULL}\" ;;\n"
                "  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n"
                "  *'repos/owner/repository'*) printf '%s' '{\"default_branch\":\"master\"}' ;;\n"
                "  *'/contents/'*) echo blob >> \"${GH_LOG}\"; exit 92 ;;\n"
                "  *'--method POST'*|*'--method PATCH'*|*'/protection/'*) echo mutation >> \"${GH_LOG}\"; exit 93 ;;\n"
                "  *) exit 91 ;;\nesac\n",
                encoding="utf-8",
            )
            fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            scope_output = directory / "scope-output"
            scope_run = {"id": 9, "pull_requests": [{"number": 72}]}
            scope = execute(scope_program, scope_run, scope_output, directory)
            self.assertEqual(scope.returncode, 0, scope.stderr)
            self.assertIn("reconcile=true", scope_output.read_text(encoding="utf-8"))

            resolver_output = directory / "resolver-output"
            resolved = execute(resolver_program, resolver_run, resolver_output, directory)
            self.assertEqual(resolved.returncode, 0, resolved.stderr)
            values = dict(line.split("=", 1) for line in resolver_output.read_text(encoding="utf-8").splitlines())
            self.assertEqual(values["reconcile"], "true")
            self.assertEqual(values["event_targets"], "[]")
            self.assertEqual(values["priority_targets"], "[]")
            log = (directory / "gh.log").read_text(encoding="utf-8") if (directory / "gh.log").exists() else ""
            self.assertNotIn("blob", log)
            self.assertNotIn("mutation", log)

            for label, malformed_head in (
                ("missing-head", None),
                ("head-not-object", "invalid"),
                ("repo-key-missing", {"sha": "a" * 40}),
                ("repo-wrong-type", {"sha": "a" * 40, "repo": "invalid"}),
            ):
                with self.subTest(shape=label):
                    malformed_source = dict(resolver_source)
                    if malformed_head is None:
                        malformed_source.pop("head")
                    else:
                        malformed_source["head"] = malformed_head
                    result = execute(resolver_program, {**resolver_run, "pull_requests": [malformed_source]}, directory / f"{label}-output", directory)
                    self.assertNotEqual(result.returncode, 0)
                    log = (directory / "gh.log").read_text(encoding="utf-8") if (directory / "gh.log").exists() else ""
                    self.assertNotIn("mutation", log)

    def test_priority_event_preempts_the_current_reconciler_and_preserves_affected_order(self) -> None:
        # sourceを持つeventだけが全件走査を中断する。通常の全件走査は
        # current snapshotを取り直すが、event由来のcloser集合はwriterへ順序を渡す。
        self.assertIn("needs: resolve_event", self.workflow)
        self.assertIn("if: needs.resolve_event.outputs.reconcile == 'true'", self.workflow)
        self.assertIn("Re-enumerate every current local governance pull request", self.workflow)
        self.assertNotIn("steps.targets.outputs.affected", self.workflow)
        self.assertIn("AFFECTED: ${{ steps.current-targets.outputs.all_invalidation_chunk_1 }}", self.workflow)
        self.assertIn("cancel-in-progress: ${{ needs.resolve_event.outputs.priority_targets != '[]' }}", self.workflow)
        self.assertIn("WRITER_TARGETS: ${{ steps.current-targets.outputs.event_targets }}", self.workflow)

    def test_every_governance_snapshot_has_explicit_nullable_fork_boundary(self) -> None:
        """Every embedded snapshot program executes null-fork and malformed-shape cases."""
        step_names = (
            "Resolve current open pull requests from the trusted default branch",
            "Re-enumerate every current local governance pull request",
            "Release complete affected-head merge barrier only after full pending coverage",
            "Dispatch one repository-wide governance arbiter segment",
            "Dispatch second repository-wide governance arbiter segment",
            "Dispatch third repository-wide governance arbiter segment",
            "Dispatch fourth repository-wide governance arbiter segment",
        )
        programs: dict[str, str] = {}
        for name in step_names:
            match = re.search(
                rf"- name: {re.escape(name)}.*?python3 - <<'PY'\n(.*?)\n          PY",
                self.workflow,
                re.DOTALL,
            )
            self.assertIsNotNone(match, name); assert match is not None
            programs[name] = self._workflow_program(match)

        all_targets = list(range(1, 601))
        heads = {number: f"{number:040x}" for number in all_targets}
        snapshots = [[number, heads[number], False] for number in all_targets]
        manifest = [[number, 100_000 + number] for number in all_targets]

        def pages(
            head: object, branch: str, count: int, *, append_malformed: bool,
        ) -> list[list[dict[str, object]]]:
            values = [[
                {
                    "number": number,
                    "state": "open",
                    "draft": False,
                    "base": {"ref": branch, "repo": {"full_name": "owner/repository"}},
                    "head": {"sha": heads[number], "repo": {"full_name": "owner/repository"}},
                }
                for number in range(start, min(start + 100, count + 1))
            ] for start in range(1, count + 1, 100)]
            # Keep malformed-shape cases explicit even at the 600-target
            # boundary; the nullable fork case itself must not create a
            # seventh page that would be rejected before fork filtering.
            if count < len(all_targets) or append_malformed:
                values[-1].append({
                "number": 1001,
                "state": "open",
                "draft": False,
                "base": {"ref": branch, "repo": {"full_name": "owner/repository"}},
                "head": head,
                })
            return values

        fake_gh = """#!/usr/bin/env python3
import json, os, re, sys

arguments = sys.argv[1:]
joined = " ".join(arguments)
with open(os.path.join(os.path.dirname(__file__), "fixture.json"), encoding="utf-8") as fixture_file:
    fixture = json.load(fixture_file)
repository = os.environ.get("GITHUB_REPOSITORY", fixture["repository"])
branch = os.environ.get("DEFAULT_BRANCH", fixture["branch"])
log = fixture["log"]
state = fixture["state"]
head = "a" * 40
context = "KRR / PR governance affected-head barrier"
app_id = 4_766_933
bot = "katana-rust-pr-governance-hf[bot]"

def record(value):
    with open(log, "a", encoding="utf-8") as output:
        output.write(value + "\\n")

def load_state():
    try:
        with open(state, encoding="utf-8") as source:
            return json.load(source)
    except FileNotFoundError:
        return {"dispatched": False, "barrier": True}

def save_state(value):
    with open(state, "w", encoding="utf-8") as output:
        json.dump(value, output)

def emit(value):
    print(json.dumps(value, separators=(",", ":")))
    raise SystemExit(0)

source = {"id": 9, "name": "PR governance dispatcher", "path": ".github/workflows/pr-governance.yml@" + branch, "event": "issues", "status": "in_progress", "run_attempt": 1, "run_number": 1, "head_branch": branch, "head_sha": head, "repository": {"full_name": repository}, "created_at": "2026-09-01T00:00:00Z"}

if "pulls?state=open" in joined:
    if "--paginate" in arguments:
        emit(fixture["pulls"])
    page_match = re.search(r"[?&]page=(\\d+)", joined)
    page = int(page_match.group(1)) if page_match else 1
    value = fixture["pulls"][page - 1] if page <= len(fixture["pulls"]) else []
    if "--include" in arguments:
        print("HTTP/2 200 OK\\n\\n" + json.dumps(value, separators=(",", ":")))
        raise SystemExit(0)
    emit(value)
if joined.endswith("/git/ref/heads/" + branch):
    emit({"object": {"sha": head}})
if joined.endswith("repos/" + repository):
    emit({"default_branch": branch})
if "/contents/" in joined:
    emit({"sha": "b" * 40})
if "/check-runs/" in joined:
    identifier = int(joined.rsplit("/", 1)[1])
    number = identifier - 100_000
    pull_head = f"{number:040x}"
    emit({"id": identifier, "name": "KRR / PR governance (trusted check)", "head_sha": pull_head, "external_id": f"krr-governance/v1/{pull_head}/dispatcher-9", "status": "in_progress", "conclusion": None, "app": {"id": app_id}, "details_url": f"https://github.com/{repository}/actions/runs/9?dispatcher_run_id=9&carry_pending=0"})
if "/branches/" in joined and "/protection" in joined:
    value = load_state()
    if "--method DELETE" in joined:
        record("release")
        value["barrier"] = False
        save_state(value)
        emit([])
    if "--method POST" in joined:
        record("protection-post")
        value["barrier"] = True
        save_state(value)
        emit([context])
    checks = [{"context": context, "app_id": app_id}] if value["barrier"] else []
    status_checks_url = f"https://api.github.com/repos/{repository}/branches/{branch}/protection/required_status_checks"
    emit({"required_status_checks": {"url": status_checks_url, "contexts_url": status_checks_url + "/contexts", "checks": checks, "contexts": [item["context"] for item in checks], "strict": True}})
if "/actions/runs/" in joined:
    identifier = int(joined.rsplit("/", 1)[1])
    if identifier == 9:
        emit(source)
    index = identifier - 90_000
    emit({"id": identifier, "name": "PR governance status writer", "display_title": f"source=9 scope=all segment={index}", "path": ".github/workflows/pr-governance-status-writer.yml@" + branch, "event": "workflow_dispatch", "repository": {"full_name": repository}, "head_branch": branch, "head_sha": head, "status": "completed", "conclusion": "success", "run_number": index, "run_attempt": 1, "actor": {"login": bot, "type": "Bot"}, "triggering_actor": {"login": bot, "type": "Bot"}})
if "/actions/workflows/pr-governance.yml/runs?" in joined:
    emit([{"workflow_runs": [source]}])
if "/actions/workflows/pr-governance-status-writer.yml/runs?" in joined:
    value = load_state()
    completed = json.loads(os.environ.get("COMPLETED_WRITER_RUN_IDS", fixture["completed"]))
    runs = [{"id": identifier, "name": "PR governance status writer", "display_title": f"source=9 scope=all segment={identifier - 90_000}", "path": ".github/workflows/pr-governance-status-writer.yml@" + branch, "event": "workflow_dispatch", "repository": {"full_name": repository}, "head_branch": branch, "head_sha": head, "status": "completed", "conclusion": "success", "run_number": identifier - 90_000, "run_attempt": 1, "actor": {"login": bot, "type": "Bot"}, "triggering_actor": {"login": bot, "type": "Bot"}} for identifier in completed]
    if value["dispatched"]:
        index = int(os.environ.get("CONTINUATION_INDEX", fixture["continuation_index"]))
        runs.append({"id": 90_000 + index, "name": "PR governance status writer", "display_title": f"source=9 scope=all segment={index}", "path": ".github/workflows/pr-governance-status-writer.yml@" + branch, "event": "workflow_dispatch", "repository": {"full_name": repository}, "head_branch": branch, "head_sha": head, "status": "queued", "run_number": index, "run_attempt": 1})
    emit({"total_count": len(runs), "workflow_runs": runs})
if "/dispatches" in joined and "--method POST" in joined:
    record("dispatch")
    value = load_state()
    value["dispatched"] = True
    save_state(value)
    emit({})
if "/check-runs" in joined and "--method POST" in joined:
    record("check-run-terminal")
    emit({})
raise SystemExit(91)
"""

        def environment_for(name: str, branch: str) -> dict[str, str]:
            common = {
                "GITHUB_REPOSITORY": "owner/repository",
                "DEFAULT_BRANCH": branch,
                "WRITER_HEAD": "a" * 40,
                "DEFAULT_HEAD": "a" * 40,
                "DISPATCHER_RUN_ID": "9",
                "GITHUB_SERVER_URL": "https://github.com",
                "CHECK_APP_ID": "4766933",
                "READ_TOKEN": "read",
                "ADMIN_TOKEN": "admin",
                "WORKFLOW_REF": f"owner/repository/.github/workflows/pr-governance.yml@refs/heads/{branch}",
                "WORKFLOW_SHA": "a" * 40,
                "EVENT_TARGETS": "[]",
                "EVENT_PRIORITY_TARGETS": "[]",
            }
            if name == step_names[0]:
                return common | {"EVENT_NAME": "schedule"}
            if name == step_names[1]:
                return common
            if name == step_names[2]:
                barrier_targets = [1]
                barrier_snapshots = [snapshots[0]]
                barrier_manifest = [manifest[0]]
                return common | {
                    "TARGETS": json.dumps(barrier_targets, separators=(",", ":")),
                    "TARGET_SNAPSHOTS": json.dumps(barrier_snapshots, separators=(",", ":")),
                    "PRE_MANIFEST_1": "[]", "PRE_MANIFEST_2": "[]",
                    "TAIL_MANIFEST_1": json.dumps(barrier_manifest, separators=(",", ":")),
                    "TAIL_MANIFEST_2": "[]", "DUPLICATE_GOVERNED_HEADS": "[]",
                }
            index = step_names.index(name) - 3 + 1
            completed = list(range(90_001, 90_000 + index))
            common |= {
                "WRITER_TARGETS": "[]",
                "WRITER_ALL_OPEN_SNAPSHOTS": json.dumps(snapshots, separators=(",", ":")),
                "WRITER_PRESERVED_TARGETS": "[]", "PRESERVED_WRITER_RUN_ID": "0",
                "WRITER_CHECK_MANIFEST": json.dumps(manifest, separators=(",", ":")),
                "WRITER_TERMINAL_ORDER": json.dumps(all_targets, separators=(",", ":")),
                "COMPLETED_WRITER_RUN_IDS": json.dumps(completed, separators=(",", ":")),
                "TERMINAL_BATCH": json.dumps(all_targets[(index - 1) * 150:index * 150], separators=(",", ":")),
                "CONTINUATION_INDEX": str(index), "APP_BOT_LOGIN": "katana-rust-pr-governance-hf[bot]",
            }
            if index == 1:
                return common | {
                    "WRITER_SCOPE": "all", "WRITER_ALL_OPEN_TARGETS": json.dumps(all_targets, separators=(",", ":")),
                    "WRITER_PREINVALIDATE_TARGETS": "[]", "WRITER_PRE_CHECK_MANIFEST_1": "[]", "WRITER_PRE_CHECK_MANIFEST_2": "[]",
                    "WRITER_TAIL_CHECK_MANIFEST_1": json.dumps(manifest, separators=(",", ":")), "WRITER_TAIL_CHECK_MANIFEST_2": "[]",
                    "WRITER_PRESERVED_CHECK_MANIFEST": "[]", "WRITER_CARRY_TARGET_NUMBERS_1": "[]", "WRITER_CARRY_TARGET_NUMBERS_2": "[]",
                }
            return common

        missing_head = object()
        malformed_heads: tuple[tuple[str, object], ...] = (
            ("missing-head", missing_head),
            ("head-not-object", "invalid"),
            ("repo-key-missing", {"sha": "f" * 40}),
            ("repo-wrong-type", {"sha": "f" * 40, "repo": "invalid"}),
        )

        def execute(name: str, head: object, branch: str = "master") -> tuple[subprocess.CompletedProcess[str], list[str], str]:
            with tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary)
                fake = directory / "gh"; log = directory / "gh.log"; state = directory / "gh.state"; output = directory / "output"
                fake.write_text(fake_gh, encoding="utf-8")
                fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                unavailable = {"sha": "f" * 40, "repo": None}
                count = 1 if name == step_names[2] else len(all_targets)
                if head is missing_head:
                    fixture_pages = pages(unavailable, branch, count, append_malformed=True)
                    del fixture_pages[0][-1]["head"]
                else:
                    fixture_pages = pages(
                        unavailable if head is None else head,
                        branch,
                        count,
                        append_malformed=head is not None,
                    )
                fixture = directory / "fixture.json"
                fixture.write_text(json.dumps({
                    "repository": "owner/repository", "branch": branch, "log": str(log), "state": str(state),
                    "pulls": fixture_pages, "completed": environment_for(name, branch).get("COMPLETED_WRITER_RUN_IDS", "[]"),
                    "continuation_index": environment_for(name, branch).get("CONTINUATION_INDEX", "1"),
                }), encoding="utf-8")
                environment = os.environ | environment_for(name, branch) | {
                    "GH_LOG": str(log), "GH_STATE": str(state),
                    "GITHUB_OUTPUT": str(output), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                # Keep the 600-PR fixture on disk: Linux rejects a single
                # execve environment string at roughly 128 KiB (E2BIG).
                self.assertNotIn("PULLS", environment)
                self.assertTrue(all(
                    len(os.fsencode(key)) + len(os.fsencode(value)) < 128 * 1024
                    for key, value in environment.items()
                ))
                result = subprocess.run([sys.executable, "-c", programs[name]], env=environment, capture_output=True, text=True, check=False)
                mutations = log.read_text(encoding="utf-8").splitlines() if log.exists() else []
                return result, mutations, output.read_text(encoding="utf-8") if output.exists() else ""

        for branch in ("master", "release/v0.5"):
            for name in step_names:
                with self.subTest(step=name, branch=branch, shape="nullable-fork"):
                    result, mutations, output = execute(name, None, branch)
                    self.assertEqual(result.returncode, 0, f"{result.stderr}\nmutations={mutations}")
                    if name == step_names[0]:
                        self.assertIn("reconcile=true", output)
                    if name == step_names[1]:
                        values = dict(line.split("=", 1) for line in output.splitlines())
                        self.assertEqual(json.loads(values["targets"]), all_targets)
                    if name == step_names[2]:
                        self.assertEqual(mutations, ["release"])
                    else:
                        self.assertNotIn("release", mutations)
                        if name in step_names[3:]:
                            self.assertEqual(mutations, ["dispatch"])
                        else:
                            self.assertEqual(mutations, [])

                for label, malformed in malformed_heads:
                    with self.subTest(step=name, branch=branch, shape=label):
                        result, mutations, _output = execute(name, malformed, branch)
                        self.assertNotEqual(result.returncode, 0)
                        self.assertEqual(mutations, [], result.stderr)

    def test_priority_preinvalidation_precedes_drain_and_normal_drain_preserves_token_boundaries(self) -> None:
        preinvalidate = self.workflow.index("Pre-invalidate priority event heads")
        drain = self.workflow.index("Drain authoritative writer before the next governance hand-off")
        invalidate = self.workflow.index("Invalidate every current pull request for the all-open writer")
        dispatch = self.workflow.index("Dispatch one repository-wide governance arbiter")
        # Priority traffic gets a synchronous pending Check Run before drain;
        # the drain then completes before the early writer is dispatched.
        self.assertLess(preinvalidate, drain)
        self.assertLess(drain, invalidate)
        self.assertLess(invalidate, dispatch)
        next_step = self.workflow.index("- name: Dispatch and bind the early event writer", drain)
        section = self.workflow[drain:next_step]
        self.assertIn("if: steps.current-targets.outputs.has_targets == 'true'", self.workflow[drain:next_step])
        self.assertNotIn("has_preinvalidate_targets != 'true'", self.workflow[drain:next_step])
        self.assertIn("GH_TOKEN: ${{ steps.dispatcher-token.outputs.token }}", section)
        self.assertNotIn("CHECK_WRITE_TOKEN", section)
        self.assertNotIn('"--paginate", "--slurp"', section)
        self.assertIn('urlencode({"event": "workflow_dispatch", "branch": branch, "status": status, "per_page": "100"})', section)
        self.assertIn('f"repos/{repository}/actions/workflows/{workflow_id}/runs?{query}"', section)
        self.assertIn('total > 100 or len(entries) != total', section)
        self.assertIn("class ActiveWriterSnapshotChanged(Exception):", section)
        self.assertIn("def active_writer_snapshot():", section)
        self.assertIn("active_snapshot_attempts = 4", section)
        self.assertIn("Governance writer active run list did not stabilize.", section)
        self.assertIn('f"repos/{repository}/actions/runs/{identifier}/cancel"', section)
        self.assertIn('active = ("requested", "queued", "pending", "waiting", "in_progress")', section)
        self.assertIn("for _ in range(150):", section)
        self.assertIn('run.get("status") != "completed"', section)
        self.assertIn("Governance writer run identity is invalid.", section)

    def test_event_writer_is_terminal_before_full_snapshot_invalidation(self) -> None:
        dispatch = self.workflow.index("Dispatch and bind the early event writer")
        await_early = self.workflow.index("Await the bound early event writer before all-open invalidation")
        all_open = self.workflow.index("Invalidate every current pull request for the all-open writer")
        self.assertLess(dispatch, await_early)
        self.assertLess(await_early, all_open)
        wait_section = self.workflow[await_early:all_open]
        for value in (
            "DEFAULT_BRANCH: ${{ steps.current-targets.outputs.default_branch }}",
            "WRITER_HEAD: ${{ steps.current-targets.outputs.writer_head }}",
            "DISPATCHER_RUN_ID: ${{ github.run_id }}",
            "CHECK_READ_TOKEN: ${{ steps.early-check-read-token.outputs.token }}",
            'run.get("display_title")!=title', 'run.get("head_sha")!=head',
            'run.get("status")=="completed"', 'run.get("conclusion")!="success"',
        ):
            self.assertIn(value, wait_section)
        self.assertNotIn("CHECK_READ_TOKEN: ${{ steps.invalidator-token.outputs.token }}", wait_section)
        self.assertIn('env={"GH_TOKEN":check_token,"PATH":os.environ["PATH"]}', wait_section)
        read_token = self.workflow[self.workflow.index("Create first priority invalidator read token"):self.workflow.index("Dispatch and bind the early event writer")]
        self.assertIn("permission-checks: read", read_token)

    def test_early_dispatch_binds_exact_new_writer_or_fails_closed(self) -> None:
        match = re.search(
            r"- name: Dispatch and bind the early event writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        base_program = self._workflow_program(match).replace("time.sleep(2)", "None")
        valid = {
            "id": 71, "name": "PR governance status writer", "display_title": "source=99 scope=early segment=0",
            "path": ".github/workflows/pr-governance-status-writer.yml@master", "event": "workflow_dispatch",
            "repository": {"full_name": "owner/repository"}, "head_branch": "master", "head_sha": "a" * 40,
            "status": "queued", "run_number": 1, "run_attempt": 1,
        }
        for mode, expected in (("exact", 0), ("ambiguous", 1), ("bad-path", 1), ("bad-attempt", 1), ("timeout", 1)):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); fake = directory / "gh"; state = directory / "state"; output = directory / "output"
                fake.write_text(
                    "#!/usr/bin/env python3\n"
                    "import json, os, sys\n"
                    "arguments = ' '.join(sys.argv[1:]); state = os.environ['STATE']\n"
                    "count = int(open(state).read()) if os.path.exists(state) else 0\n"
                    "if '/runs?per_page=100' in arguments:\n"
                    "    open(state, 'w').write(str(count + 1))\n"
                    "    if count < 2 or os.environ['MODE'] == 'timeout': print(json.dumps([{'workflow_runs': []}]))\n"
                    "    else:\n"
                    "        run = json.loads(os.environ['RUN'])\n"
                    "        if os.environ['MODE'] == 'bad-path': run['path'] = '.github/workflows/other.yml@master'\n"
                    "        if os.environ['MODE'] == 'bad-attempt': run['run_attempt'] = True\n"
                    "        runs = [run] if os.environ['MODE'] != 'ambiguous' else [run, dict(run, id=72)]\n"
                    "        print(json.dumps([{'workflow_runs': runs}]))\n"
                    "elif '/dispatches' in arguments:\n"
                    "    if 'inputs[scope]=early' not in arguments or 'inputs[target_numbers]=[72,73]' not in arguments or 'inputs[preserved_target_numbers]=[]' not in arguments or 'inputs[preserved_writer_run_id]=0' not in arguments or 'inputs[terminal_order_numbers]=[]' not in arguments or 'inputs[completed_writer_run_ids]=[]' not in arguments: raise SystemExit(92)\n"
                    "else: raise SystemExit(91)\n",
                    encoding="utf-8",
                ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                sleeper = directory / "sleep"
                sleeper.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
                sleeper.chmod(sleeper.stat().st_mode | stat.S_IXUSR)
                program = base_program.replace('subprocess.run(["sleep", str(min(5, remaining))], check=False)', "None")
                if mode == "timeout":
                    program = program.replace("range(60)", "range(2)")
                environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "WRITER_HEAD": "a" * 40,
                    "DISPATCHER_RUN_ID": "99", "TARGETS": "[72,73]", "MODE": mode, "STATE": str(state),
                    "RUN": json.dumps(valid), "GITHUB_OUTPUT": str(output), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", program], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, expected, result.stderr)
                if expected == 0:
                    self.assertEqual(output.read_text(encoding="utf-8"), "writer_run_id=71\n")

    def test_early_writer_wait_rejects_identity_drift_and_non_success_terminal(self) -> None:
        match = re.search(
            r"- name: Await the bound early event writer before all-open invalidation.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        base_program = self._workflow_program(match).replace("time.sleep(2)", "None")
        valid = {
            "id": 71, "name": "PR governance status writer", "display_title": "source=99 scope=early segment=0",
            "path": ".github/workflows/pr-governance-status-writer.yml@master", "event": "workflow_dispatch",
            "repository": {"full_name": "owner/repository"}, "head_branch": "master", "head_sha": "a" * 40,
            "status": "completed", "conclusion": "success", "run_number": 1, "run_attempt": 1,
            "actor": {"login": "katana-rust-pr-governance-hf[bot]", "type": "Bot"},
            "triggering_actor": {"login": "katana-rust-pr-governance-hf[bot]", "type": "Bot"},
        }
        for mutate, expected in ((lambda run: None, 0), (lambda run: run.update(conclusion="failure"), 1), (lambda run: run.update(head_sha="b" * 40), 1), (lambda run: run.update(run_attempt=True), 1)):
            with self.subTest(expected=expected), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); fake = directory / "gh"
                run = dict(valid); mutate(run)
                check = {
                    "id": 701, "name": "KRR / PR governance (trusted check)",
                    "head_sha": "a" * 40,
                    "external_id": "krr-governance/v1/" + "a" * 40 + "/writer-71",
                    "app": {"id": 42}, "status": "completed", "conclusion": "success",
                    "details_url": "https://github.com/owner/repository/actions/runs/71?source_run_id=99",
                }
                token_log = directory / "check-read-token"
                fake.write_text(
                    "#!/bin/sh\ncase \"${GH_TOKEN}:$*\" in\n"
                    f"  checks-read:*'/pulls/72'*) printf '%s' '{{\"head\":{{\"sha\":\"{'a' * 40}\"}}}}' ;;\n"
                    f"  checks-read:*) printf '%s' \"${{GH_TOKEN}}\" > '{token_log}'; printf '%s' '{json.dumps([{'check_runs': [check]}])}' ;;\n"
                    "  *) printf '%s' \"${RUN}\" ;;\nesac\n",
                    encoding="utf-8",
                )
                fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "WRITER_RUN_ID": "71", "DEFAULT_BRANCH": "master",
                    "WRITER_HEAD": "a" * 40, "DISPATCHER_RUN_ID": "99", "RUN": json.dumps(run),
                    "CHECK_APP_ID": "42",
                    "CHECK_READ_TOKEN": "checks-read", "GH_TOKEN": "actions-write", "TARGETS": "[72]",
                    "APP_BOT_LOGIN": "katana-rust-pr-governance-hf[bot]",
                    "GITHUB_SERVER_URL": "https://github.com", "GITHUB_OUTPUT": str(directory / "output"),
                    "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", base_program], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, expected, result.stderr)
                if expected == 0:
                    self.assertEqual(token_log.read_text(encoding="utf-8"), "checks-read")

    def test_priority_preinvalidation_is_synchronous_unique_and_fail_closed(self) -> None:
        """Priority heads receive a newer pending generation before any writer can finish."""
        match = re.search(
            r"- name: Pre-invalidate priority event heads.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = self._workflow_program(match)
        # The stable snapshot is produced at the same boundary as the current
        # priority list.  A fresh PR GET alone cannot distinguish a head that
        # changed after that boundary from the event which is being fenced.
        self.assertIn("preinvalidate_target_snapshots", self.workflow)
        self.assertIn("TARGET_SNAPSHOTS", self.workflow)
        self.assertIn("CHECK_WRITE_TOKEN", self.workflow)
        preinvalidate = self.workflow.index("- name: Pre-invalidate priority event heads")
        early_dispatch = self.workflow.index("- name: Dispatch and bind the early event writer")
        await_early = self.workflow.index("- name: Await the bound early event writer before all-open invalidation")
        drain = self.workflow.index("- name: Drain authoritative writer before the next governance hand-off")
        self.assertLess(preinvalidate, drain)
        self.assertLess(drain, early_dispatch)
        self.assertLess(early_dispatch, await_early)
        preinvalidate_step = self.workflow[preinvalidate:self.workflow.index("- name: Dispatch and bind the early event writer", preinvalidate)]
        self.assertIn("GH_TOKEN: ${{ steps.pre-invalidator-read-1.outputs.token }}", preinvalidate_step)
        self.assertIn("CHECK_WRITE_TOKEN: ${{ steps.pre-invalidator-write-1.outputs.token }}", preinvalidate_step)
        self.assertIn('read_env={"GH_TOKEN":read_token,"PATH":os.environ["PATH"]}', preinvalidate_step)
        self.assertIn("env=read_env", preinvalidate_step)
        self.assertNotIn("READ_TOKEN:", preinvalidate_step)
        for value in (
            "len(entry)!=6", "entry[0]!=number", "entry[3]!=branch",
            "entry[4]!=repository", "entry[5]!=repository", "current_number!=number",
            "current_base_ref!=entry[3]", "current_base_repo!=entry[4]",
            "current_head_repo!=entry[5]", 'pull.get("state")!="open"',
        ):
            self.assertIn(value, preinvalidate_step)

        first, second = "a" * 40, "b" * 40
        def execute(heads: dict[int, str], snapshots: list[list[object]], mode: str = "valid") -> tuple[int, list[list[str]], list[str], list[str]]:
            posts: list[list[str]] = []; rereads: list[str] = []; sleeps: list[float] = []; created: dict[int, dict[str, object]] = {}
            def response(value: object, code: int = 0) -> subprocess.CompletedProcess[str]:
                return subprocess.CompletedProcess([], code, json.dumps(value), "")
            def fake_run(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
                endpoint = arguments[-1]
                if isinstance(endpoint, str) and "/pulls/" in endpoint:
                    self.assertEqual(_kwargs.get("env"), {"GH_TOKEN": "read", "PATH": os.environ["PATH"]})
                    number = int(endpoint.rsplit("/", 1)[1])
                    value = heads[number]
                    if mode == "head-drift" and number == 72:
                        value = "c" * 40
                    current: dict[str, object] = {
                        "number": number,
                        "state": "open",
                        "draft": False,
                        "base": {"ref": "master", "repo": {"full_name": "owner/repository"}},
                        "head": {"sha": value, "repo": {"full_name": "owner/repository"}},
                    }
                    if mode == "number-drift" and number == 72:
                        current["number"] = 73
                    if mode == "state-drift" and number == 72:
                        current["state"] = "closed"
                    if mode == "draft-drift" and number == 72:
                        current["draft"] = True
                    if mode == "base-ref-drift" and number == 72:
                        current["base"] = {"ref": "release", "repo": {"full_name": "owner/repository"}}
                    if mode == "base-repo-drift" and number == 72:
                        current["base"] = {"ref": "master", "repo": {"full_name": "other/repository"}}
                    if mode == "head-repo-drift" and number == 72:
                        current["head"] = {"sha": value, "repo": {"full_name": "fork/repository"}}
                    return response(current)
                if "--method" in arguments and "POST" in arguments:
                    self.assertEqual(_kwargs.get("env"), {"GH_TOKEN": "write", "PATH": os.environ["PATH"]})
                    posts.append(arguments)
                    if mode == "post-failure" and len(posts) == 2:
                        return response({}, 7)
                    fields = {field.split("=", 1)[0]: field.split("=", 1)[1] for field in arguments if "=" in field}
                    identifier = 100 + len(created)
                    check: dict[str, object] = {
                        "id": identifier, "app": {"id": 42}, "name": "KRR / PR governance (trusted check)",
                        "head_sha": fields["head_sha"], "external_id": fields["external_id"],
                        "status": "in_progress", "conclusion": None, "details_url": fields["details_url"],
                    }
                    created[identifier] = check
                    return response("malformed" if mode == "malformed-post" else check)
                if isinstance(endpoint, str) and "/check-runs/" in endpoint:
                    self.assertEqual(_kwargs.get("env"), {"GH_TOKEN": "read", "PATH": os.environ["PATH"]})
                    rereads.append(endpoint)
                    identifier = int(endpoint.rsplit("/", 1)[1]); current = dict(created[identifier])
                    if mode == "stale-reread": current["status"] = "completed"; current["conclusion"] = "success"
                    if mode == "wrong-reread-app": current["app"] = {"id": 7}
                    if mode == "wrong-reread-id": current["id"] = 999
                    if mode == "wrong-reread-name": current["name"] = "other"
                    if mode == "wrong-reread-head": current["head_sha"] = "c" * 40
                    if mode == "wrong-reread-external": current["external_id"] = "krr-governance/v1/" + "c" * 40 + "/dispatcher-9"
                    if mode == "wrong-reread-details": current["details_url"] = "https://example.invalid/other"
                    return response(current)
                raise AssertionError(arguments)
            environment = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com",
                "GH_TOKEN": "read", "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "TARGETS": json.dumps(sorted(heads)),
                "TARGET_SNAPSHOTS": json.dumps(snapshots), "DEFAULT_BRANCH": "master", "DISPATCHER_RUN_ID": "9",
                "GITHUB_OUTPUT": str(Path(tempfile.mkdtemp()) / "output"),
                "PATH": os.environ["PATH"],
            }
            with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run), patch("time.sleep", side_effect=lambda seconds: sleeps.append(seconds)):
                try:
                    exec(program, {"__name__": "__main__"})
                    return 0, posts, sleeps, rereads
                except SystemExit:
                    return 1, posts, sleeps, rereads

        snapshot = lambda number, head, draft=False: [number, head, draft, "master", "owner/repository", "owner/repository"]
        result, posts, sleeps, rereads = execute({72: first, 73: second}, [snapshot(72, first), snapshot(73, second)])
        self.assertEqual(result, 0)
        self.assertEqual(len(posts), 2)
        self.assertEqual(len(rereads), 2)
        # ``_workflow_program`` replaces wall-clock sleeps with deterministic
        # fake-clock advancement, so patch("time.sleep") no longer observes
        # the pacing call.  Verify the adapted program retains that semantic
        # boundary while the production-source assertion above covers the
        # original API spelling.
        self.assertIn("_krr_sleep(delay)", program)
        for expected_head, post in zip((first, second), posts):
            self.assertIn(f"head_sha={expected_head}", post)
            self.assertIn(f"external_id=krr-governance/v1/{expected_head}/dispatcher-9", post)
            self.assertIn("status=in_progress", post)
        # A shared head is one immutable namespace: it gets exactly one new
        # dispatcher generation and cannot let an old writer success win.
        result, duplicate_posts, _, duplicate_rereads = execute({72: first, 73: first}, [snapshot(72, first), snapshot(73, first)])
        self.assertEqual(result, 0)
        self.assertEqual(len(duplicate_posts), 1)
        self.assertEqual(len(duplicate_rereads), 1)
        for mode in ("head-drift", "number-drift", "state-drift", "draft-drift", "base-ref-drift", "base-repo-drift", "head-repo-drift", "malformed-post", "stale-reread", "wrong-reread-app", "wrong-reread-id", "wrong-reread-name", "wrong-reread-head", "wrong-reread-external", "wrong-reread-details"):
            with self.subTest(mode=mode):
                result, _, _, _ = execute({72: first, 73: second}, [snapshot(72, first), snapshot(73, second)], mode)
                self.assertEqual(result, 1)
        # Failure is recorded per head, but a later head is still made pending
        # before the batch exits.  This prevents a partial API outage from
        # leaving an unrelated affected closer with an older success.
        third = "c" * 40
        result, partial_posts, _, _ = execute(
            {72: first, 73: second, 74: third},
            [snapshot(72, first), snapshot(73, second), snapshot(74, third)], "post-failure",
        )
        self.assertEqual(result, 1)
        self.assertEqual(len(partial_posts), 3)
        self.assertIn(f"head_sha={third}", partial_posts[-1])
        for snapshots in (
            [snapshot(71, first), snapshot(73, second)],
            [snapshot(72, "c" * 40), snapshot(73, second)],
            [snapshot(72, first, True), snapshot(73, second)],
        ):
            with self.subTest(snapshots=snapshots):
                result, _, _, _ = execute({72: first, 73: second}, snapshots)
                self.assertEqual(result, 1)

    def test_static_barrier_is_atomic_and_requires_fresh_complete_recovery(self) -> None:
        """Execute the trusted YAML programs against atomic context API state transitions."""
        def program(name: str) -> str:
            match = re.search(rf"- name: {re.escape(name)}.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
            self.assertIsNotNone(match, name); assert match is not None
            return self._workflow_program(match)

        source = program("Verify default-branch governance source before barrier credentials")
        activate = program("Activate complete affected-head merge barrier")
        release = program("Release complete affected-head merge barrier only after full pending coverage")
        marker = program("Publish periodic static affected-head barrier App marker")
        barrier = "KRR / PR governance affected-head barrier"; head, other = "a" * 40, "b" * 40
        self.assertLess(self.workflow.index("Activate complete affected-head merge barrier"), self.workflow.index("Pre-invalidate priority event heads"))
        self.assertLess(self.workflow.index("Release complete affected-head merge barrier only after full pending coverage"), self.workflow.index("Dispatch one repository-wide governance arbiter segment"))
        self.assertNotIn("required_status_checks\",\"--input\",\"-\"", self.workflow)
        self.assertIn("required_status_checks", activate)
        self.assertIn("required_status_checks/contexts", release)
        self.assertNotIn("required_status_checks/contexts", activate)
        self.assertNotIn("actions/checkout", self.workflow)
        marker_condition = "(github.event_name == 'schedule' || github.event_name == 'workflow_dispatch' || steps.current-targets.outputs.has_preinvalidate_targets == 'true' || steps.current-targets.outputs.invalidation_head_cap_exceeded == 'true') && steps.barrier-source.outcome == 'success'"
        self.assertEqual(self.workflow.count(marker_condition), 3)
        marker_steps = self.workflow[self.workflow.index("Create periodic affected-head barrier marker write token"):self.workflow.index("Create affected-head barrier branch-protection token")]
        self.assertIn("has_preinvalidate_targets", marker_steps)
        status_checks_url = "https://api.github.com/repos/owner/repository/branches/master/protection/required_status_checks"
        contexts_url = status_checks_url + "/contexts"
        baseline = {
            "required_status_checks": {"url": status_checks_url, "contexts_url": contexts_url, "strict": True, "contexts": ["CI / test"], "checks": [{"context": "CI / test", "app_id": None}]},
            "enforce_admins": {"enabled": True}, "required_conversation_resolution": {"enabled": True},
        }
        state: dict[str, object] = {
            "protection": json.loads(json.dumps(baseline)), "mutations": [], "uncertain_delete": False,
            "restore_failures": 0, "restore_attempts": 0,
            # The external App-only ruleset remains the merge authority.  The
            # dynamic required-context barrier is defense-in-depth and must
            # not be treated as a replacement for that admission boundary.
            "app_only_admission": {"actor": "external-integration-app", "merge_authority": True},
            "pulls": [
                {"number": 72, "state": "open", "draft": False, "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": head, "repo": {"full_name": "owner/repository"}}},
                {"number": 73, "state": "open", "draft": False, "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": other, "repo": {"full_name": "owner/repository"}}},
                {"number": 74, "state": "open", "draft": False, "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": "c" * 40, "repo": None}},
            ],
        }
        run = {"id": 99, "name": "PR governance dispatcher", "path": ".github/workflows/pr-governance.yml@master", "event": "issue_comment", "repository": {"full_name": "owner/repository"}, "head_branch": "master", "head_sha": head, "run_number": 1, "run_attempt": 1, "status": "in_progress", "created_at": "2026-08-30T00:00:00Z"}
        state["runs"] = [run]
        state["manifest_checks"] = {
            801: {"id": 801, "name": "KRR / PR governance (trusted check)", "head_sha": head, "external_id": f"krr-governance/v1/{head}/dispatcher-99", "status": "in_progress", "conclusion": None, "details_url": "https://github.com/owner/repository/actions/runs/99?dispatcher_run_id=99&carry_pending=0", "app": {"id": 4_766_933}},
            802: {"id": 802, "name": "KRR / PR governance (trusted check)", "head_sha": other, "external_id": f"krr-governance/v1/{other}/dispatcher-99", "status": "in_progress", "conclusion": None, "details_url": "https://github.com/owner/repository/actions/runs/99?dispatcher_run_id=99&carry_pending=0", "app": {"id": 4_766_933}},
        }
        state["late_event_without_run_list"] = None
        state["late_event_observed_at_release"] = False
        state["dispatcher_jobs"] = {}

        def completed(value: object, code: int = 0) -> subprocess.CompletedProcess[str]:
            return subprocess.CompletedProcess([], code, json.dumps(value), "")

        def route(arguments: list[str]) -> str:
            values = [value for value in arguments if isinstance(value, str) and value.startswith("repos/")]
            self.assertEqual(len(values), 1, arguments)
            return values[0]

        def protection_records() -> list[dict[str, object]]:
            protection = state["protection"]; self.assertIsInstance(protection, dict)
            required = protection["required_status_checks"]; self.assertIsInstance(required, dict)
            checks = required["checks"]; self.assertIsInstance(checks, list)
            return checks

        def fake_run(arguments: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
            self.assertEqual(arguments[:4], ["gh", "api", "--hostname", "github.com"])
            endpoint = route(arguments); environment = kwargs.get("env"); self.assertIsInstance(environment, dict)
            token = environment.get("GH_TOKEN")  # type: ignore[union-attr]
            if endpoint == "repos/owner/repository":
                self.assertIn(token, {"source", "read"}); return completed({"default_branch": "master"})
            if endpoint == "repos/owner/repository/git/ref/heads/master":
                self.assertIn(token, {"source", "read"}); return completed({"object": {"sha": head}})
            if endpoint.endswith("/contents/.github/workflows/pr-governance.yml?ref=" + head):
                self.assertIn(token, {"source", "read"}); return completed({"sha": "c" * 40})
            if endpoint == "repos/owner/repository/branches/master/protection":
                self.assertEqual(token, "admin"); return completed(state["protection"])
            if endpoint == "repos/owner/repository/branches/master/protection/required_status_checks":
                self.assertEqual(token, "admin"); method = arguments[arguments.index("--method") + 1]
                self.assertIn(method, {"PUT", "PATCH"})
                required = state["protection"]["required_status_checks"]  # type: ignore[index]
                self.assertIsInstance(required, dict)
                expected = json.loads(str(kwargs["input"]))
                if method == "PATCH" and barrier not in required["contexts"]:
                    state["restore_attempts"] = int(state["restore_attempts"]) + 1
                    if int(state["restore_failures"]) > 0:
                        state["restore_failures"] = int(state["restore_failures"]) - 1
                        return completed({}, 1)
                self.assertEqual(expected, {
                    "strict": required["strict"],
                    "checks": [*required["checks"], {"context": barrier, "app_id": 4_766_933}],
                })
                mutations = state["mutations"]; self.assertIsInstance(mutations, list); mutations.append("ACTIVATE")
                required["checks"].append({"context": barrier, "app_id": 4_766_933}); required["contexts"].append(barrier)
                return completed({"url": required["url"], "contexts_url": required["contexts_url"], "strict": required["strict"], "checks": required["checks"], "contexts": required["contexts"]})
            if endpoint == "repos/owner/repository/branches/master/protection/required_status_checks/contexts":
                self.assertEqual(token, "admin"); method = arguments[arguments.index("--method") + 1]
                self.assertEqual(method, "DELETE"); self.assertEqual(json.loads(str(kwargs["input"])), {"contexts": [barrier]})
                records = protection_records(); required = state["protection"]["required_status_checks"]  # type: ignore[index]
                self.assertIsInstance(required, dict); mutations = state["mutations"]; self.assertIsInstance(mutations, list); mutations.append(method)
                if method == "DELETE":
                    records[:] = [item for item in records if item["context"] != barrier]; required["contexts"][:] = [name for name in required["contexts"] if name != barrier]
                    late_event = state["late_event_without_run_list"]
                    if late_event is not None:
                        runs = state["runs"]; checks = state["manifest_checks"]
                        self.assertIsInstance(late_event, dict); self.assertIsInstance(runs, list); self.assertIsInstance(checks, dict)
                        self.assertNotIn(late_event["id"], [item["id"] for item in runs])
                        self.assertNotIn(barrier, required["contexts"])
                        self.assertTrue(all(item["status"] == "in_progress" and item["conclusion"] is None for item in checks.values()))
                        state["late_event_observed_at_release"] = True
                    if state["uncertain_delete"]: return completed([], 1)
                return completed(required["contexts"])
            if endpoint.startswith("repos/owner/repository/pulls?state=open&per_page=100"):
                self.assertEqual(token, "read"); return completed([state["pulls"]])
            if endpoint == "repos/owner/repository/actions/runs/99":
                self.assertEqual(token, "read"); return completed(run)
            if endpoint == "repos/owner/repository/actions/runs/100/jobs?per_page=100":
                self.assertEqual(token, "read"); jobs = state["dispatcher_jobs"]
                self.assertIsInstance(jobs, dict); return completed(jobs[100])
            if endpoint.startswith("repos/owner/repository/actions/workflows/pr-governance.yml/runs?per_page=100"):
                self.assertEqual(token, "read"); return completed([{"workflow_runs": state["runs"]}])
            if endpoint == "repos/owner/repository/check-runs":
                self.assertEqual(token, "marker-write"); return completed({"id": 501, "name": barrier, "head_sha": head, "external_id": f"krr-governance-affected-head-barrier/v1/{head}/scheduler-99", "status": "completed", "conclusion": "success", "details_url": "https://github.com/owner/repository/actions/runs/99?barrier_marker=periodic", "app": {"id": 4_766_933}})
            if endpoint == "repos/owner/repository/check-runs/501":
                self.assertEqual(token, "marker-read"); return completed({"id": 501, "name": barrier, "head_sha": head, "external_id": f"krr-governance-affected-head-barrier/v1/{head}/scheduler-99", "status": "completed", "conclusion": "success", "details_url": "https://github.com/owner/repository/actions/runs/99?barrier_marker=periodic", "app": {"id": 4_766_933}})
            if endpoint in {"repos/owner/repository/check-runs/801", "repos/owner/repository/check-runs/802"}:
                self.assertEqual(token, "read"); identifier = int(endpoint.rsplit("/", 1)[1]); checks = state["manifest_checks"]
                self.assertIsInstance(checks, dict); return completed(checks[identifier])
            raise AssertionError(arguments)

        def execute(code: str, environment: dict[str, str]) -> int:
            with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run):
                try:
                    exec(code, {"__name__": "__main__"}); return 0
                except SystemExit:
                    return 1

        def outputs(path: Path) -> dict[str, str]:
            return dict(line.split("=", 1) for line in path.read_text(encoding="utf-8").splitlines())

        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); output = directory / "output"
            common = {"GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "PATH": os.environ["PATH"], "DEFAULT_BRANCH": "master", "DEFAULT_HEAD": head, "DISPATCHER_RUN_ID": "99", "CHECK_APP_ID": "4766933", "WORKFLOW_REF": "owner/repository/.github/workflows/pr-governance.yml@refs/heads/master", "WORKFLOW_SHA": head}
            self.assertEqual(execute(source, common | {"GH_TOKEN": "source"}), 0)
            self.assertEqual(execute(marker, common | {"CHECK_WRITE_TOKEN": "marker-write", "CHECK_READ_TOKEN": "marker-read"}), 0)
            activate_env = common | {"ADMIN_TOKEN": "admin", "GITHUB_OUTPUT": str(output)}
            self.assertEqual(execute(activate, activate_env | {"PRIORITY": "false"}), 0)
            self.assertEqual(outputs(output)["active"], "false"); self.assertEqual(state["mutations"], [])
            self.assertEqual(execute(activate, activate_env | {"PRIORITY": "true"}), 0)
            self.assertEqual(state["mutations"], ["ACTIVATE"])
            old_success = {"CI / test"}
            self.assertNotEqual({item["context"] for item in protection_records()}, old_success, "atomic POST blocks old success before paced writes")
            recovery = directory / "recovery"
            self.assertEqual(execute(activate, activate_env | {"PRIORITY": "false", "GITHUB_OUTPUT": str(recovery)}), 0)
            self.assertEqual(outputs(recovery)["active"], "true")

            def release_env(targets: str, snapshots: str, manifest_1: str, manifest_2: str = "[]") -> dict[str, str]:
                return common | {"READ_TOKEN": "read", "ADMIN_TOKEN": "admin", "TARGETS": targets, "TARGET_SNAPSHOTS": snapshots, "PRE_MANIFEST_1": manifest_1, "PRE_MANIFEST_2": manifest_2, "TAIL_MANIFEST_1": "[]", "TAIL_MANIFEST_2": "[]", "DUPLICATE_GOVERNED_HEADS": "[]"}

            self.assertEqual(execute(release, release_env("[72,73]", f'[[72,"{head}",false],[73,"{other}",false]]', "[[72,801]]")), 1)
            self.assertEqual(state["mutations"], ["ACTIVATE"])
            pulls = state["pulls"]; self.assertIsInstance(pulls, list); pulls[0] = {**pulls[0], "head": {"sha": "c" * 40, "repo": {"full_name": "owner/repository"}}}
            self.assertEqual(execute(release, release_env("[72,73]", f'[[72,"{head}",false],[73,"{other}",false]]', "[[72,801]]", "[[73,802]]")), 1)
            self.assertEqual(state["mutations"], ["ACTIVATE"]); pulls[0] = {**pulls[0], "head": {"sha": head, "repo": {"full_name": "owner/repository"}}}
            runs = state["runs"]; self.assertIsInstance(runs, list); runs.append({**run, "id": 100, "created_at": "2026-08-30T00:00:01Z"})
            self.assertEqual(execute(release, release_env("[72,73]", f'[[72,"{head}",false],[73,"{other}",false]]', "[[72,801]]", "[[73,802]]")), 1)
            self.assertEqual(state["mutations"], ["ACTIVATE"]); runs.pop()
            jobs = state["dispatcher_jobs"]; self.assertIsInstance(jobs, dict)
            workflow_run = {**run, "id": 100, "event": "workflow_run", "status": "completed", "conclusion": "success", "created_at": "2026-08-30T00:00:01Z"}
            jobs[100] = {"total_count": 2, "jobs": [
                {"id": 1, "name": "Preflight workflow_run governance source", "status": "completed", "conclusion": "success"},
                {"id": 2, "name": "Establish resolver-failure merge barrier", "status": "completed", "conclusion": "success"},
            ]}
            runs.append(workflow_run)
            self.assertEqual(execute(release, release_env("[72,73]", f'[[72,"{head}",false],[73,"{other}",false]]', "[[72,801]]", "[[73,802]]")), 1)
            self.assertEqual(state["mutations"], ["ACTIVATE"]); runs.pop()
            jobs[100]["jobs"][1]["conclusion"] = "skipped"  # type: ignore[index]
            runs.append(workflow_run)
            self.assertEqual(execute(release, release_env("[72,73]", f'[[72,"{head}",false],[73,"{other}",false]]', "[[72,801]]", "[[73,802]]")), 0)
            self.assertEqual(state["mutations"], ["ACTIVATE", "DELETE"]); runs.pop()
            self.assertEqual(execute(activate, activate_env | {"PRIORITY": "true"}), 0)
            self.assertEqual(state["mutations"], ["ACTIVATE", "DELETE", "ACTIVATE"])
            state["uncertain_delete"] = True
            self.assertEqual(execute(release, release_env("[72,73]", f'[[72,"{head}",false],[73,"{other}",false]]', "[[72,801]]", "[[73,802]]")), 1)
            self.assertEqual(state["mutations"], ["ACTIVATE", "DELETE", "ACTIVATE", "DELETE", "ACTIVATE"]); state["uncertain_delete"] = False
            runs.insert(0, {**run, "id": 98, "status": "completed", "created_at": "2026-08-29T23:59:59Z"})
            state["late_event_without_run_list"] = {**run, "id": 100, "created_at": "2026-08-30T00:00:02Z"}
            self.assertEqual(execute(release, release_env("[72,73]", f'[[72,"{head}",false],[73,"{other}",false]]', "[[72,801]]", "[[73,802]]")), 0)
            self.assertEqual(state["mutations"], ["ACTIVATE", "DELETE", "ACTIVATE", "DELETE", "ACTIVATE", "DELETE"]); runs.pop(0)
            self.assertTrue(state["late_event_observed_at_release"])
            manifest_checks = state["manifest_checks"]; self.assertIsInstance(manifest_checks, dict)
            self.assertTrue(all(item["status"] == "in_progress" and item["conclusion"] is None for item in manifest_checks.values()))
            state["late_event_without_run_list"] = None
            self.assertEqual({item["context"] for item in protection_records()}, old_success)

            # A dispatcher rerun is not a new first-attempt generation.  It
            # must not turn a safe recovery into a permanent barrier, while
            # malformed historical records and a rerun of the current source
            # remain fail-closed.
            rerun = {**run, "id": 100, "run_attempt": 2, "created_at": "2026-08-30T00:00:02Z"}
            runs.append(rerun)
            self.assertEqual(execute(activate, activate_env | {"PRIORITY": "true"}), 0)
            self.assertEqual(execute(release, release_env("[72,73]", f'[[72,"{head}",false],[73,"{other}",false]]', "[[72,801]]", "[[73,802]]")), 0)
            runs.pop()
            for malformed_attempt in (True, "2", None):
                with self.subTest(malformed_attempt=malformed_attempt):
                    self.assertEqual(execute(activate, activate_env | {"PRIORITY": "true"}), 0)
                    runs.append({**run, "id": 100, "run_attempt": malformed_attempt, "created_at": "2026-08-30T00:00:02Z"})
                    self.assertEqual(execute(release, release_env("[72,73]", f'[[72,"{head}",false],[73,"{other}",false]]', "[[72,801]]", "[[73,802]]")), 1)
                    runs.pop()
            original_attempt = run["run_attempt"]
            run["run_attempt"] = 2
            self.assertEqual(execute(release, release_env("[72,73]", f'[[72,"{head}",false],[73,"{other}",false]]', "[[72,801]]", "[[73,802]]")), 1)
            run["run_attempt"] = original_attempt

            self.assertEqual(execute(activate, activate_env | {"PRIORITY": "true"}), 0)
            pulls[:] = []
            self.assertEqual(execute(release, release_env("[]", "[]", "[]")), 0, "a later schedule recovers a static barrier even after all PRs close")

            # P2 regression: DELETE is applied server-side but returns an
            # uncertain response, and both bounded PATCH recovery attempts
            # fail persistently. The embedded release program must fail
            # closed without producing a success output that could hand off
            # to the dispatcher. The independent external App-only
            # admission boundary remains unchanged and authoritative.
            pulls[:] = [
                {"number": 72, "state": "open", "draft": False, "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": head, "repo": {"full_name": "owner/repository"}}},
                {"number": 73, "state": "open", "draft": False, "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": other, "repo": {"full_name": "owner/repository"}}},
                {"number": 74, "state": "open", "draft": False, "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": "c" * 40, "repo": None}},
            ]
            self.assertEqual(execute(activate, activate_env | {"PRIORITY": "true"}), 0)
            state["uncertain_delete"] = True
            state["restore_failures"] = 2
            state["restore_attempts"] = 0
            failed_release_output = directory / "uncertain-release-output"
            failed_release = execute(
                release,
                release_env("[72,73]", f'[[72,"{head}",false],[73,"{other}",false]]', "[[72,801]]", "[[73,802]]")
                | {"GITHUB_OUTPUT": str(failed_release_output)},
            )
            self.assertNotEqual(failed_release, 0)
            self.assertEqual(state["restore_attempts"], 2)
            self.assertEqual(state["restore_failures"], 0)
            self.assertFalse(failed_release_output.exists())
            self.assertEqual(state["app_only_admission"], {"actor": "external-integration-app", "merge_authority": True})
            release_step = self.workflow[self.workflow.index("- name: Release complete affected-head merge barrier only after full pending coverage"):self.workflow.index("- name: Create fresh all-writer dispatcher token")]
            self.assertNotIn("continue-on-error: true", release_step)

    def test_fresh_priority_event_publishes_app_barrier_before_context_only_binding(self) -> None:
        """The first priority generation must seed the App marker before adding its context."""
        marker_match = re.search(
            r"^      - name: Publish periodic static affected-head barrier App marker\n(?P<body>.*?)(?=^      - name: )",
            self.workflow,
            re.MULTILINE | re.DOTALL,
        )
        self.assertIsNotNone(marker_match); assert marker_match is not None
        marker_step = marker_match.group("body")
        condition = re.search(r"^        if: (?P<value>.+)$", marker_step, re.MULTILINE)
        self.assertIsNotNone(condition); assert condition is not None

        # A fresh issue/review/CI event may be the first event after bootstrap;
        # schedule-only marker publication leaves context-only activation
        # unbound to the App marker on that first priority path.
        self.assertIn("steps.current-targets.outputs.has_preinvalidate_targets == 'true'", condition.group("value"))
        self.assertIn("steps.barrier-source.outcome == 'success'", condition.group("value"))
        self.assertIn("context=\"KRR / PR governance affected-head barrier\"", marker_step)
        self.assertIn("app_id=4_766_933", marker_step)
        self.assertIn("posted[\"app\"].get(\"id\")!=app_id", marker_step)

        activate_match = re.search(
            r"^      - name: Activate complete affected-head merge barrier\n(?P<body>.*?)(?=^      - name: )",
            self.workflow,
            re.MULTILINE | re.DOTALL,
        )
        self.assertIsNotNone(activate_match); assert activate_match is not None
        activate_step = activate_match.group("body")
        self.assertIn("required_status_checks", activate_step)
        self.assertNotIn("required_status_checks/contexts", activate_step)
        self.assertRegex(activate_step, r'mutate\("(?:PUT|PATCH)"\)')
        self.assertIn('if matches==[(context,app_id)]', activate_step)
        self.assertIn(
            "steps.current-targets.outputs.has_preinvalidate_targets == 'true' || steps.current-targets.outputs.invalidation_head_cap_exceeded == 'true'",
            activate_step,
        )

        # Zero-target reconciliation must not manufacture a fresh marker or
        # mutate branch protection; priority is the only additional trigger.
        self.assertNotIn("steps.current-targets.outputs.has_preinvalidate_targets == 'false'", condition.group("value"))
        self.assertLess(self.workflow.index("Publish periodic static affected-head barrier App marker"), self.workflow.index("Activate complete affected-head merge barrier"))

    def test_app_bound_barrier_activation_updates_required_status_checks_fail_closed(self) -> None:
        """Both activation paths must preserve the complete check binding atomically."""
        barrier = "KRR / PR governance affected-head barrier"
        protection_endpoint = "repos/owner/repository/branches/master/protection"
        update_endpoint = protection_endpoint + "/required_status_checks"
        activation_names = ("Activate resolver-failure merge barrier", "Activate complete affected-head merge barrier")

        def program(name: str) -> str:
            match = re.search(rf"- name: {re.escape(name)}.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
            self.assertIsNotNone(match, name); assert match is not None
            return self._workflow_program(match)

        for name in activation_names:
            with self.subTest(activation=name):
                code = program(name)
                self.assertNotIn("required_status_checks/contexts", code)
                self.assertRegex(code, r"required_status_checks")
                self.assertIn('"strict"', code)
                self.assertIn('"checks"', code)

                for mode in ("success", "null_app", "wrong_app", "missing_url", "missing_contexts_url", "mismatched_url", "extra_url", "existing_loss", "api_failure", "invalid_response", "response_mismatch", "after_mismatch"):
                    with self.subTest(mode=mode):
                        baseline_checks = [
                            {"context": "CI / test", "app_id": None},
                            {"context": "KRR / PR governance review latch", "app_id": 15368},
                        ]
                        baseline_contexts = [item["context"] for item in baseline_checks]
                        if mode in {"null_app", "wrong_app"}:
                            barrier_app = None if mode == "null_app" else 15368
                            baseline_checks.append({"context": barrier, "app_id": barrier_app})
                            baseline_contexts.append(barrier)
                        status_checks_url = "https://api.github.com/repos/owner/repository/branches/master/protection/required_status_checks"
                        required_state: dict[str, object] = {
                            "url": status_checks_url,
                            "contexts_url": status_checks_url + "/contexts",
                            "strict": True,
                            "contexts": baseline_contexts,
                            "checks": baseline_checks,
                        }
                        if mode == "missing_url":
                            required_state.pop("url")
                        elif mode == "missing_contexts_url":
                            required_state.pop("contexts_url")
                        elif mode == "mismatched_url":
                            required_state["contexts_url"] = status_checks_url + "/wrong"
                        elif mode == "extra_url":
                            required_state["unexpected_url"] = status_checks_url + "/unexpected"
                        state: dict[str, object] = {
                            "protection": {
                                "required_status_checks": required_state,
                            },
                            "updates": 0,
                            "reads": 0,
                        }

                        def clone(value: object) -> object:
                            return json.loads(json.dumps(value))

                        def response(value: object, code_value: int = 0) -> subprocess.CompletedProcess[str]:
                            return subprocess.CompletedProcess([], code_value, json.dumps(value), "")

                        def fake_run(arguments: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                            self.assertEqual(arguments[:4], ["gh", "api", "--hostname", "github.com"])
                            endpoints = [value for value in arguments if isinstance(value, str) and value.startswith("repos/")]
                            self.assertEqual(len(endpoints), 1, arguments)
                            endpoint = endpoints[0]
                            environment = kwargs.get("env"); self.assertIsInstance(environment, dict)
                            self.assertEqual(environment.get("GH_TOKEN"), "admin")  # type: ignore[union-attr]
                            if endpoint == protection_endpoint:
                                state["reads"] = int(state["reads"]) + 1
                                return response(clone(state["protection"]))
                            if endpoint == protection_endpoint + "/required_status_checks/contexts":
                                raise AssertionError("activation must not use the context-only endpoint")
                            self.assertEqual(endpoint, update_endpoint)
                            method = arguments[arguments.index("--method") + 1]
                            self.assertIn(method, {"PUT", "PATCH"})
                            if mode in {"null_app", "wrong_app", "missing_url", "missing_contexts_url", "mismatched_url", "extra_url"}:
                                state["updates"] = int(state["updates"]) + 1
                                return response({}, 1)
                            payload = json.loads(str(kwargs["input"]))
                            required = state["protection"]["required_status_checks"]  # type: ignore[index]
                            self.assertIsInstance(required, dict)
                            expected = {
                                "strict": required["strict"],
                                "checks": [*required["checks"], {"context": barrier, "app_id": 4_766_933}],
                            }
                            self.assertEqual(payload, expected)
                            state["updates"] = int(state["updates"]) + 1
                            if mode == "api_failure":
                                return response({}, 1)
                            if mode == "invalid_response":
                                return response({"strict": True, "checks": "invalid"})
                            if mode == "existing_loss":
                                applied = {"strict": required["strict"], "checks": [{"context": barrier, "app_id": 4_766_933}]}
                            else:
                                applied = expected
                            returned = {
                                "url": required["url"],
                                "contexts_url": required["contexts_url"],
                                "strict": applied["strict"],
                                "checks": applied["checks"],
                                "contexts": [item["context"] for item in applied["checks"]],
                            }
                            required["strict"] = applied["strict"]
                            required["checks"] = applied["checks"]
                            required["contexts"] = [item["context"] for item in applied["checks"]]
                            if mode == "after_mismatch":
                                required["strict"] = False
                            if mode == "response_mismatch":
                                returned["strict"] = False
                            return response(returned)

                        output_fd, output_name = tempfile.mkstemp()
                        os.close(output_fd)
                        environment = {
                            "GITHUB_REPOSITORY": "owner/repository", "PATH": os.environ["PATH"],
                            "ADMIN_TOKEN": "admin", "DEFAULT_BRANCH": "master", "CHECK_APP_ID": "4766933",
                            "PRIORITY": "true", "DEFAULT_HEAD": "a" * 40, "DISPATCHER_RUN_ID": "99",
                            "GITHUB_OUTPUT": output_name,
                        }
                        try:
                            with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run):
                                try:
                                    exec(code, {"__name__": "__main__"})
                                except SystemExit:
                                    result = 1
                                else:
                                    result = 0
                        finally:
                            os.unlink(output_name)
                        if mode == "success":
                            self.assertEqual(result, 0)
                            self.assertEqual(state["updates"], 1)
                            self.assertEqual(state["reads"], 2)
                            required = state["protection"]["required_status_checks"]  # type: ignore[index]
                            self.assertEqual(required["strict"], True)
                            self.assertEqual(required["contexts"], [item["context"] for item in baseline_checks] + [barrier])
                            self.assertEqual(required["checks"][-1], {"context": barrier, "app_id": 4_766_933})
                            self.assertEqual(required["checks"][:-1], baseline_checks)
                        else:
                            self.assertEqual(result, 1)
                            if mode in {"null_app", "wrong_app", "missing_url", "missing_contexts_url", "mismatched_url", "extra_url"}:
                                self.assertEqual(state["updates"], 0)

    def test_resolver_snapshot_failure_keeps_an_app_bound_global_barrier_and_stops_handoff(self) -> None:
        """The fallible resolver cannot run before an App-bound merge fence."""
        job = self.workflow[
            self.workflow.index("  establish-resolver-failure-barrier:"):
            self.workflow.index("  resolve_event:")
        ]
        resolver = re.search(
            r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        activate = re.search(
            r"- name: Activate resolver-failure merge barrier.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        source = re.search(
            r"- name: Bind resolver-failure barrier to trusted default workflow source.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(resolver); self.assertIsNotNone(activate); self.assertIsNotNone(source)
        assert resolver is not None and activate is not None and source is not None
        self.assertIn("needs: establish-resolver-failure-barrier", self.workflow)
        self.assertIn("if: needs.resolve_event.outputs.reconcile == 'true'", self.workflow)
        resolver_job = self.workflow[
            self.workflow.index("  resolve_event:"):
            self.workflow.index("  reconcile-all-open:")
        ]
        reconciler_job = self.workflow[self.workflow.index("  reconcile-all-open:"):]
        preflight_generation_lock = (
            "concurrency:\n      group: pr-governance-dispatcher-${{ github.repository_id }}\n"
            "      cancel-in-progress: ${{ needs.preflight-workflow-run-source.outputs.priority == 'true' }}"
        )
        resolver_generation_lock = (
            "concurrency:\n      group: pr-governance-dispatcher-${{ github.repository_id }}\n"
            "      cancel-in-progress: ${{ needs.establish-resolver-failure-barrier.outputs.priority == 'true' }}"
        )
        # A priority event cancels both an older resolver and reconciler. A
        # validated CI/release event has priority=false and remains serialized,
        # so ordinary workflow traffic cannot cancel a paced all-open scan.
        self.assertIn(preflight_generation_lock, job)
        self.assertIn(resolver_generation_lock, resolver_job)
        self.assertIn("group: pr-governance-dispatcher-${{ github.repository_id }}", reconciler_job)
        self.assertIn("cancel-in-progress: ${{ needs.resolve_event.outputs.priority_targets != '[]' }}", reconciler_job)
        self.assertNotIn("actions/checkout", job)
        self.assertNotIn("github.event.pull_request", job)
        self.assertIn("repos/{repository}/git/ref/heads/{branch}", job)
        self.assertIn("contents/.github/workflows/pr-governance.yml?ref={workflow_sha}", job)
        for step in (
            "Create resolver-failure barrier marker write token",
            "Create resolver-failure barrier marker read token",
            "Publish resolver-failure barrier App marker",
            "Create resolver-failure barrier branch-protection token",
            "Activate resolver-failure merge barrier",
        ):
            self.assertIn(step, job)
        # This predecessor deliberately does not defer token, marker, or
        # branch-protection failures: a failed prerequisite skips resolver,
        # dispatcher, writer, and release while an already-active barrier is
        # retained for the next schedule recovery.
        self.assertNotIn("continue-on-error: true", job)

        def response(value: object, code: int = 0) -> subprocess.CompletedProcess[str]:
            return subprocess.CompletedProcess([], code, json.dumps(value), "")

        barrier = "KRR / PR governance affected-head barrier"
        status_checks_url = "https://api.github.com/repos/owner/repository/branches/master/protection/required_status_checks"
        state: dict[str, object] = {
            "protection": {
                "required_status_checks": {
                    "url": status_checks_url,
                    "contexts_url": status_checks_url + "/contexts",
                    "strict": True,
                    "contexts": ["CI / test"],
                    "checks": [{"context": "CI / test", "app_id": None}],
                },
            },
            "mutations": [],
        }

        def route(arguments: list[str]) -> str:
            endpoints = [value for value in arguments if isinstance(value, str) and value.startswith("repos/")]
            self.assertEqual(len(endpoints), 1, arguments)
            return endpoints[0]

        def fake_run(arguments: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
            self.assertEqual(arguments[:4], ["gh", "api", "--hostname", "github.com"])
            environment = kwargs.get("env"); self.assertIsInstance(environment, dict)
            self.assertEqual(environment.get("GH_TOKEN"), "admin")  # type: ignore[union-attr]
            endpoint = route(arguments)
            if endpoint == "repos/owner/repository/branches/master/protection":
                return response(state["protection"])
            if endpoint == "repos/owner/repository/branches/master/protection/required_status_checks":
                method = arguments[arguments.index("--method") + 1]
                self.assertIn(method, {"PUT", "PATCH"})
                required = state["protection"]["required_status_checks"]  # type: ignore[index]
                self.assertIsInstance(required, dict)
                expected = json.loads(str(kwargs["input"]))
                self.assertEqual(expected, {
                    "strict": required["strict"],
                    "checks": [*required["checks"], {"context": barrier, "app_id": 4_766_933}],
                })
                contexts = required["contexts"]; checks = required["checks"]
                self.assertIsInstance(contexts, list); self.assertIsInstance(checks, list)
                contexts.append(barrier); checks.append({"context": barrier, "app_id": 4_766_933})
                state["mutations"].append("ACTIVATE")  # type: ignore[index]
                return response({"url": required["url"], "contexts_url": required["contexts_url"], "strict": required["strict"], "checks": checks, "contexts": contexts})
            raise AssertionError(arguments)

        def run(code: str, environment: dict[str, str]) -> int:
            with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run):
                try:
                    exec(code, {"__name__": "__main__"})
                except SystemExit:
                    return 1
                return 0

        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            source_output = directory / "source-output"
            source_state = {"head": "a" * 40}

            def fake_source_run(arguments: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                self.assertEqual(arguments[:4], ["gh", "api", "--hostname", "github.com"])
                environment = kwargs.get("env"); self.assertIsInstance(environment, dict)
                self.assertEqual(environment.get("GH_TOKEN"), "source")  # type: ignore[union-attr]
                endpoint = route(arguments)
                if endpoint == "repos/owner/repository":
                    return response({"default_branch": "master"})
                if endpoint == "repos/owner/repository/git/ref/heads/master":
                    return response({"object": {"sha": source_state["head"]}})
                if endpoint == "repos/owner/repository/contents/.github/workflows/pr-governance.yml?ref=" + "a" * 40:
                    return response({"sha": "b" * 40})
                raise AssertionError(arguments)

            source_environment = {
                "GITHUB_REPOSITORY": "owner/repository", "GH_TOKEN": "source", "PATH": os.environ["PATH"],
                "GITHUB_OUTPUT": str(source_output),
                "WORKFLOW_REF": "owner/repository/.github/workflows/pr-governance.yml@refs/heads/master",
                "WORKFLOW_SHA": "a" * 40,
            }
            with patch.dict(os.environ, source_environment, clear=True), patch("subprocess.run", side_effect=fake_source_run):
                exec(self._workflow_program(source), {"__name__": "__main__"})
            source_values = dict(line.split("=", 1) for line in source_output.read_text(encoding="utf-8").splitlines())
            self.assertEqual(source_values, {"default_branch": "master", "default_head": "a" * 40})
            source_state["head"] = "c" * 40
            changed_source_output = directory / "changed-source-output"
            with patch.dict(os.environ, source_environment | {"GITHUB_OUTPUT": str(changed_source_output)}, clear=True), patch("subprocess.run", side_effect=fake_source_run):
                with self.assertRaises(SystemExit):
                    exec(self._workflow_program(source), {"__name__": "__main__"})
            self.assertFalse(changed_source_output.exists())

            activation_environment = {
                "GITHUB_REPOSITORY": "owner/repository", "PATH": os.environ["PATH"],
                "ADMIN_TOKEN": "admin", "DEFAULT_BRANCH": "master", "CHECK_APP_ID": "4766933",
            }
            self.assertEqual(run(self._workflow_program(activate), activation_environment), 0)
            protection = state["protection"]["required_status_checks"]  # type: ignore[index]
            self.assertIsInstance(protection, dict)
            records = protection["checks"]; self.assertIsInstance(records, list)
            self.assertIn({"context": barrier, "app_id": 4_766_933}, records)
            self.assertNotEqual({entry["context"] for entry in records}, {"CI / test"})

            # Failure injection happens only after the barrier mutation. The
            # resolver has no output and the downstream job's `needs`/`if`
            # contract means it cannot dispatch a writer or release the
            # barrier from this failed generation.
            failed_gh = directory / "gh"
            failed_gh.write_text("#!/bin/sh\nexit 1\n", encoding="utf-8")
            failed_gh.chmod(failed_gh.stat().st_mode | stat.S_IXUSR)
            failed = subprocess.run(
                [sys.executable, "-c", self._workflow_program(resolver)],
                env={
                    "GITHUB_REPOSITORY": "owner/repository", "EVENT_NAME": "schedule",
                    "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(directory / "resolver-output"),
                    "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}", "GH_TOKEN": "read",
                }, capture_output=True, text=True, check=False,
            )
            self.assertNotEqual(failed.returncode, 0)
            self.assertFalse((directory / "resolver-output").exists())
            self.assertEqual(state["mutations"], ["ACTIVATE"])

    def test_resolver_failure_barrier_rejects_reruns_before_any_mutation(self) -> None:
        """A rerun must not arm a barrier that its rerun-skipped reconciler cannot release."""
        job = self.workflow[
            self.workflow.index("  establish-resolver-failure-barrier:"):
            self.workflow.index("  resolve_event:")
        ]
        condition = re.search(r"^    if: (?P<value>.+)$", job, re.MULTILINE)
        self.assertIsNotNone(condition); assert condition is not None
        self.assertEqual(
            condition.group("value"),
            "${{ needs.preflight-workflow-run-source.outputs.reconcile == 'true' && github.run_attempt == 1 && (github.event_name != 'workflow_run' || ("
            "(github.event.workflow_run.name == 'PR governance review sensor' && "
            "(github.event.workflow_run.event == 'pull_request' || "
            "github.event.workflow_run.event == 'pull_request_review' || "
            "github.event.workflow_run.event == 'pull_request_review_comment')) || "
            "((github.event.workflow_run.name == 'CI' || "
            "github.event.workflow_run.name == 'release-preflight') && "
            "github.event.workflow_run.event == 'pull_request'))) }}",
        )

        def permitted(attempt: int, event_name: str, workflow_name: str = "", workflow_event: str = "") -> bool:
            return attempt == 1 and (
                event_name != "workflow_run"
                or (
                    workflow_name == "PR governance review sensor"
                    and workflow_event in {"pull_request", "pull_request_review", "pull_request_review_comment"}
                )
                or (
                    workflow_name in {"CI", "release-preflight"}
                    and workflow_event == "pull_request"
                )
            )

        fixtures = (
            (1, "schedule", "", "", True),
            (1, "workflow_run", "PR governance review sensor", "pull_request", True),
            (1, "workflow_run", "PR governance review sensor", "pull_request_review", True),
            (1, "workflow_run", "PR governance review sensor", "pull_request_review_comment", True),
            (1, "workflow_run", "CI", "pull_request", True),
            (1, "workflow_run", "release-preflight", "pull_request", True),
            (1, "workflow_run", "CI", "push", False),
            (2, "schedule", "", "", False),
            (2, "workflow_run", "PR governance review sensor", "pull_request", False),
            (2, "workflow_run", "CI", "pull_request", False),
            (2, "workflow_run", "release-preflight", "pull_request", False),
        )
        for attempt, event_name, workflow_name, workflow_event, expected in fixtures:
            with self.subTest(
                attempt=attempt,
                event_name=event_name,
                workflow_name=workflow_name,
                workflow_event=workflow_event,
            ):
                self.assertEqual(permitted(attempt, event_name, workflow_name, workflow_event), expected)

    def test_steady_priority_preinvalidates_before_fallible_marker_failure(self) -> None:
        """Setup failures defer the abort until both priority chunks are pending."""
        def step(name: str) -> str:
            match = re.search(
                rf"^      - name: {re.escape(name)}\n(?P<body>.*?)(?=^      - name: |\Z)",
                self.workflow, re.MULTILINE | re.DOTALL,
            )
            self.assertIsNotNone(match, name); assert match is not None
            return match.group("body")

        def program(name: str) -> str:
            match = re.search(rf"- name: {re.escape(name)}.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
            self.assertIsNotNone(match, name); assert match is not None
            return self._workflow_program(match)

        marker_name = "Publish periodic static affected-head barrier App marker"
        activate_name = "Activate complete affected-head merge barrier"
        pre_names = (
            "Pre-invalidate priority event heads (first TTL-safe chunk)",
            "Pre-invalidate priority event heads (second TTL-safe chunk)",
        )
        marker_position = self.workflow.index(f"- name: {marker_name}")
        activate_position = self.workflow.index(f"- name: {activate_name}")
        pre_positions = [self.workflow.index(f"- name: {name}") for name in pre_names]
        drain_position = self.workflow.index("- name: Drain authoritative writer before the next governance hand-off")
        self.assertLess(marker_position, activate_position)
        self.assertLess(activate_position, pre_positions[0])
        self.assertLess(pre_positions[0], pre_positions[1])
        self.assertLess(pre_positions[1], drain_position)

        # Setup failures must be observed but not stop either pending fence.
        for name in (
            "Create periodic affected-head barrier marker write token",
            "Create periodic affected-head barrier marker read token",
            marker_name,
            "Create affected-head barrier branch-protection token",
            activate_name,
        ):
            self.assertIn("continue-on-error: true", step(name), name)
        fence_matches = list(re.finditer(
            r"^      - name: (?P<name>[^\n]*(?:fail|abort|fence)[^\n]*)\n(?P<body>.*?)(?=^      - name: |\Z)",
            self.workflow, re.MULTILINE | re.DOTALL | re.IGNORECASE,
        ))
        fences = [match for match in fence_matches if match.start() > pre_positions[1] and match.start() < drain_position]
        self.assertEqual(len(fences), 1, "one explicit setup fence must abort before drain")
        fence = fences[0].group("body")
        self.assertRegex(fence, r"exit 1|SystemExit")
        self.assertRegex(fence, r"barrier-(?:marker|protection)|affected-barrier")
        self.assertIn("has_preinvalidate_targets", step(marker_name))

        marker = program(marker_name)
        preinvalidate = [program(name) for name in pre_names]
        heads = {72: "a" * 40, 73: "b" * 40}
        events: list[str] = []
        failure_mode = "post"
        state: dict[str, object] = {"trusted": {}, "ids": {}, "next_id": 901}

        def response(value: object, code: int = 0) -> subprocess.CompletedProcess[str]:
            return subprocess.CompletedProcess([], code, json.dumps(value), "")

        def fake_run(arguments: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
            environment = kwargs.get("env")
            self.assertIsInstance(environment, dict)
            token = environment.get("GH_TOKEN")  # type: ignore[union-attr]
            endpoints = [value for value in arguments if isinstance(value, str) and value.startswith("repos/")]
            self.assertEqual(len(endpoints), 1, arguments)
            endpoint = endpoints[0]
            if endpoint == "repos/owner/repository/check-runs":
                method = arguments[arguments.index("--method") + 1]
                self.assertEqual(method, "POST")
                fields = {field.split("=", 1)[0]: field.split("=", 1)[1] for field in arguments if isinstance(field, str) and "=" in field}
                if token == "marker-write":
                    events.append("marker-post")
                    if failure_mode == "post":
                        return response({}, 1)
                    return response({"id": 501, "name": "KRR / PR governance affected-head barrier", "head_sha": "c" * 40, "external_id": f"krr-governance-affected-head-barrier/v1/{'c' * 40}/scheduler-99", "status": "completed", "conclusion": "success", "details_url": "https://github.com/owner/repository/actions/runs/99?barrier_marker=periodic", "app": {"id": 4766933}})
                self.assertIn(token, {"write-1", "write-2"})
                self.assertEqual(fields.get("status"), "in_progress")
                head = fields["head_sha"]
                number = 72 if head == heads[72] else 73
                events.append(f"pending-post-{number}")
                identifier = state["next_id"]  # type: ignore[assignment]
                state["next_id"] = identifier + 1  # type: ignore[operator]
                state["trusted"][head] = {"id": identifier, "status": "in_progress", "conclusion": None}  # type: ignore[index]
                state["ids"][identifier] = head  # type: ignore[index]
                return response({"id": identifier, "name": "KRR / PR governance (trusted check)", "head_sha": head, "external_id": fields["external_id"], "status": "in_progress", "conclusion": None, "details_url": fields["details_url"], "app": {"id": 4766933}})
            if endpoint == "repos/owner/repository/check-runs/501":
                self.assertEqual(token, "marker-read")
                events.append("marker-read")
                return response({}, 1) if failure_mode == "readback" else response({"id": 501, "name": "KRR / PR governance affected-head barrier", "head_sha": "c" * 40, "external_id": f"krr-governance-affected-head-barrier/v1/{'c' * 40}/scheduler-99", "status": "completed", "conclusion": "success", "details_url": "https://github.com/owner/repository/actions/runs/99?barrier_marker=periodic", "app": {"id": 4766933}})
            if endpoint.startswith("repos/owner/repository/check-runs/"):
                self.assertIn(token, {"read-1", "read-2"})
                identifier = int(endpoint.rsplit("/", 1)[1]); head = state["ids"][identifier]  # type: ignore[index]
                events.append("pending-read")
                current = state["trusted"][head]  # type: ignore[index]
                return response({"id": identifier, "name": "KRR / PR governance (trusted check)", "head_sha": head, "external_id": f"krr-governance/v1/{head}/dispatcher-99", "status": current["status"], "conclusion": current["conclusion"], "details_url": "https://github.com/owner/repository/actions/runs/99?dispatcher_run_id=99&carry_pending=0", "app": {"id": 4766933}})
            if endpoint.startswith("repos/owner/repository/pulls/"):
                self.assertIn(token, {"read-1", "read-2"})
                number = int(endpoint.rsplit("/", 1)[1]); head = heads[number]
                return response({"number": number, "state": "open", "draft": False, "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": head, "repo": {"full_name": "owner/repository"}}})
            raise AssertionError(arguments)

        def execute(source: str, environment: dict[str, str]) -> bool:
            with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run):
                try:
                    exec(source, {"__name__": "__main__"})
                except SystemExit:
                    return False
                return True

        with tempfile.TemporaryDirectory() as temporary:
            common = {"GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "PATH": os.environ["PATH"], "CHECK_APP_ID": "4766933", "DEFAULT_BRANCH": "master", "DISPATCHER_RUN_ID": "99", "GITHUB_OUTPUT": str(Path(temporary) / "output")}
            for failure_mode in ("token", "post", "readback"):
                with self.subTest(failure_mode=failure_mode):
                    events.clear(); state["trusted"] = {}; state["ids"] = {}; state["next_id"] = 901
                    marker_environment = common | {"CHECK_WRITE_TOKEN": "" if failure_mode == "token" else "marker-write", "CHECK_READ_TOKEN": "marker-read", "DEFAULT_HEAD": "c" * 40}
                    marker_failed = not execute(marker, marker_environment)
                    self.assertTrue(marker_failed)
                    pre_environments = (
                        common | {"GH_TOKEN": "read-1", "CHECK_WRITE_TOKEN": "write-1", "TARGETS": "[72]", "TARGET_SNAPSHOTS": json.dumps([[72, heads[72], False, "master", "owner/repository", "owner/repository"]], separators=(",", ":"))},
                        common | {"GH_TOKEN": "read-2", "CHECK_WRITE_TOKEN": "write-2", "TARGETS": "[73]", "TARGET_SNAPSHOTS": json.dumps([[73, heads[73], False, "master", "owner/repository", "owner/repository"]], separators=(",", ":"))},
                    )
                    self.assertTrue(execute(preinvalidate[0], pre_environments[0]))
                    self.assertTrue(execute(preinvalidate[1], pre_environments[1]))
                    self.assertTrue(marker_failed)
                    self.assertEqual({state["trusted"][head]["status"] for head in heads.values()}, {"in_progress"})  # type: ignore[index]
                    self.assertEqual({state["trusted"][head]["conclusion"] for head in heads.values()}, {None})  # type: ignore[index]
                    self.assertEqual([event for event in events if event.startswith("pending-post-")], ["pending-post-72", "pending-post-73"])
                    if failure_mode == "readback":
                        self.assertIn("marker-read", events)

    def test_first_priority_chunk_failure_still_runs_second_chunk_and_fails_fence(self) -> None:
        """The second invalidation chunk is an always-run fail-closed continuation."""
        def step(name: str) -> str:
            match = re.search(
                rf"^      - name: {re.escape(name)}\n(?P<body>.*?)(?=^      - name: |\Z)",
                self.workflow, re.MULTILINE | re.DOTALL,
            )
            self.assertIsNotNone(match, name); assert match is not None
            return match.group("body")

        def program(name: str) -> str:
            match = re.search(rf"- name: {re.escape(name)}.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
            self.assertIsNotNone(match, name); assert match is not None
            return self._workflow_program(match)

        first_name = "Pre-invalidate priority event heads (first TTL-safe chunk)"
        second_name = "Pre-invalidate priority event heads (second TTL-safe chunk)"
        fence = step("Fail closed after deferred priority barrier setup")
        for name in (
            "Create first priority invalidator write token",
            "Create first priority invalidator read token",
            first_name,
            "Create second priority invalidator write token",
            "Create second priority invalidator read token",
            second_name,
        ):
            self.assertIn("if: always()", step(name), name)
        self.assertIn("if: always()", fence)
        self.assertIn("PREINVALIDATE_CHUNK_1_OUTCOME", fence)
        self.assertIn("PREINVALIDATE_CHUNK_2_OUTCOME", fence)
        self.assertLess(self.workflow.index(f"- name: {first_name}"), self.workflow.index(f"- name: {second_name}"))
        self.assertLess(self.workflow.index(f"- name: {second_name}"), self.workflow.index("- name: Fail closed after deferred priority barrier setup"))

        first_program = program(first_name)
        second_program = program(second_name)
        heads = {72: "a" * 40, 73: "b" * 40}
        events: list[str] = []
        state: dict[str, object] = {"next_id": 901, "ids": {}, "pending": {}}

        def response(value: object, code: int = 0) -> subprocess.CompletedProcess[str]:
            return subprocess.CompletedProcess([], code, json.dumps(value), "")

        def fake_run(arguments: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
            environment = kwargs.get("env")
            self.assertIsInstance(environment, dict)
            token = environment.get("GH_TOKEN")  # type: ignore[union-attr]
            endpoints = [value for value in arguments if isinstance(value, str) and value.startswith("repos/")]
            self.assertEqual(len(endpoints), 1, arguments)
            endpoint = endpoints[0]
            if endpoint == "repos/owner/repository/check-runs":
                self.assertEqual(arguments[arguments.index("--method") + 1], "POST")
                fields = {field.split("=", 1)[0]: field.split("=", 1)[1] for field in arguments if isinstance(field, str) and "=" in field}
                self.assertIn(token, {"write-1", "write-2"})
                head = fields["head_sha"]
                number = 72 if head == heads[72] else 73
                events.append(f"post-{number}")
                if token == "write-1":
                    return response({}, 1)
                identifier = state["next_id"]  # type: ignore[assignment]
                state["next_id"] = identifier + 1  # type: ignore[operator]
                state["ids"][identifier] = head  # type: ignore[index]
                state["pending"][head] = True  # type: ignore[index]
                return response({"id": identifier, "name": "KRR / PR governance (trusted check)", "head_sha": head, "external_id": fields["external_id"], "status": "in_progress", "conclusion": None, "details_url": fields["details_url"], "app": {"id": 4766933}})
            if endpoint.startswith("repos/owner/repository/check-runs/"):
                self.assertEqual(token, "read-2")
                identifier = int(endpoint.rsplit("/", 1)[1]); head = state["ids"][identifier]  # type: ignore[index]
                return response({"id": identifier, "name": "KRR / PR governance (trusted check)", "head_sha": head, "external_id": f"krr-governance/v1/{head}/dispatcher-99", "status": "in_progress", "conclusion": None, "details_url": "https://github.com/owner/repository/actions/runs/99?dispatcher_run_id=99&carry_pending=0", "app": {"id": 4766933}})
            if endpoint == "repos/owner/repository/pulls/73":
                self.assertEqual(token, "read-2")
                return response({"number": 73, "state": "open", "draft": False, "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": heads[73], "repo": {"full_name": "owner/repository"}}})
            if endpoint == "repos/owner/repository/pulls/72":
                self.assertEqual(token, "read-1")
                return response({"number": 72, "state": "open", "draft": False, "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": heads[72], "repo": {"full_name": "owner/repository"}}})
            raise AssertionError(arguments)

        def execute(source: str, environment: dict[str, str]) -> bool:
            with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run):
                try:
                    exec(source, {"__name__": "__main__"})
                except SystemExit:
                    return False
                return True

        with tempfile.TemporaryDirectory() as temporary:
            common = {"GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "PATH": os.environ["PATH"], "CHECK_APP_ID": "4766933", "DEFAULT_BRANCH": "master", "DISPATCHER_RUN_ID": "99", "GITHUB_OUTPUT": str(Path(temporary) / "output")}
            first_environment = common | {"GH_TOKEN": "read-1", "CHECK_WRITE_TOKEN": "write-1", "TARGETS": "[72]", "TARGET_SNAPSHOTS": json.dumps([[72, heads[72], False, "master", "owner/repository", "owner/repository"]], separators=(",", ":"))}
            second_environment = common | {"GH_TOKEN": "read-2", "CHECK_WRITE_TOKEN": "write-2", "TARGETS": "[73]", "TARGET_SNAPSHOTS": json.dumps([[73, heads[73], False, "master", "owner/repository", "owner/repository"]], separators=(",", ":"))}
            self.assertFalse(execute(first_program, first_environment))
            self.assertTrue(execute(second_program, second_environment))
            self.assertEqual(events, ["post-72", "post-73"])
            self.assertEqual(set(state["pending"]), {heads[73]})  # type: ignore[arg-type]

            fence_environment = common | {
                "MARKER_WRITE_OUTCOME": "success", "MARKER_READ_OUTCOME": "success", "MARKER_PUBLISH_OUTCOME": "success",
                "BARRIER_TOKEN_OUTCOME": "success", "BARRIER_ACTIVATE_OUTCOME": "success", "BARRIER_ACTIVE": "true",
                "PREINVALIDATE_CHUNK_1_REQUIRED": "true", "PREINVALIDATE_CHUNK_1_OUTCOME": "failure",
                "PREINVALIDATE_CHUNK_2_REQUIRED": "true", "PREINVALIDATE_CHUNK_2_OUTCOME": "success",
            }
            fence_program = program("Fail closed after deferred priority barrier setup")
            success_environment = fence_environment | {"PREINVALIDATE_CHUNK_1_OUTCOME": "success"}
            self.assertTrue(execute(fence_program, success_environment))
            for failed_outcome in (
                "MARKER_WRITE_OUTCOME", "MARKER_READ_OUTCOME", "MARKER_PUBLISH_OUTCOME", "BARRIER_TOKEN_OUTCOME",
                "BARRIER_ACTIVATE_OUTCOME", "PREINVALIDATE_CHUNK_1_OUTCOME", "PREINVALIDATE_CHUNK_2_OUTCOME",
            ):
                with self.subTest(failed_outcome=failed_outcome):
                    self.assertFalse(execute(fence_program, success_environment | {failed_outcome: "failure"}))
            self.assertFalse(execute(fence_program, success_environment | {"BARRIER_ACTIVE": "false"}))

    def test_old_writer_generation_cannot_terminalize_current_manifest_check(self) -> None:
        """旧dispatcherのfingerprintはcurrent manifest IDのPATCH前に停止する。"""
        head = "a" * 40
        module_name = "krr_status_writer_barrier_old_generation"
        spec = importlib.util.spec_from_file_location(module_name, ROOT / "scripts/review/pr_governance_status_writer.py")
        self.assertIsNotNone(spec); assert spec is not None and spec.loader is not None
        writer = importlib.util.module_from_spec(spec); sys.modules[module_name] = writer
        try:
            spec.loader.exec_module(writer)
            old_external = f"krr-governance/v1/{head}/dispatcher-98"
            current_external = f"krr-governance/v1/{head}/dispatcher-99"
            old = {"id": 700, "name": writer.CHECK_NAME, "head_sha": head, "external_id": old_external, "updated_at": "2026-08-30T00:00:00Z", "status": "in_progress", "conclusion": None, "details_url": "https://github.com/owner/repository/actions/runs/98?dispatcher_run_id=98&carry_pending=0", "app": {"id": 4_766_933}}
            current = {"id": 801, "name": writer.CHECK_NAME, "head_sha": head, "external_id": current_external, "updated_at": "2026-08-30T00:00:01Z", "status": "in_progress", "conclusion": None, "details_url": "https://github.com/owner/repository/actions/runs/99?dispatcher_run_id=99&carry_pending=0", "app": {"id": 4_766_933}}
            with patch.dict(os.environ, {"KRR_GOVERNANCE_CHECK_APP_ID": "4766933", "GOVERNANCE_SCOPE": "all", "GOVERNANCE_DISPATCHER_RUN_ID": "98"}, clear=False):
                old_fingerprint = writer.check_fingerprint(old)
            with patch.dict(os.environ, {"KRR_GOVERNANCE_CHECK_APP_ID": "4766933", "GOVERNANCE_SCOPE": "all", "GOVERNANCE_DISPATCHER_RUN_ID": "99"}, clear=False), \
                 patch.object(writer, "check_run", return_value=current), patch.object(writer, "command") as command:
                with self.assertRaises(writer.NoPostGovernanceError):
                    writer.write_check(head, state="failure", description="old writer", details_url=current["details_url"], existing=current, expected_fingerprint=old_fingerprint)
            self.assertNotEqual(old_fingerprint[-1], current_external)
            command.assert_not_called()
        finally:
            sys.modules.pop(module_name, None)

    def test_invalidator_preempts_priority_dispatchers_and_paces_every_check_write(self) -> None:
        dispatcher_group = "group: pr-governance-dispatcher-${{ github.repository_id }}"
        self.assertEqual(self.workflow.count(dispatcher_group), 3)
        establish = self.workflow[
            self.workflow.index("  establish-resolver-failure-barrier:"):
            self.workflow.index("  resolve_event:")
        ]
        self.assertIn(
            "concurrency:\n      group: pr-governance-dispatcher-${{ github.repository_id }}\n      cancel-in-progress: ${{ needs.preflight-workflow-run-source.outputs.priority == 'true' }}",
            establish,
        )
        resolver = self.workflow[
            self.workflow.index("  resolve_event:"):
            self.workflow.index("  reconcile-all-open:")
        ]
        self.assertIn(
            "concurrency:\n      group: pr-governance-dispatcher-${{ github.repository_id }}\n      cancel-in-progress: ${{ needs.establish-resolver-failure-barrier.outputs.priority == 'true' }}",
            resolver,
        )
        self.assertIn(
            "concurrency:\n      group: pr-governance-dispatcher-${{ github.repository_id }}\n      cancel-in-progress: ${{ needs.resolve_event.outputs.priority_targets != '[]' }}",
            self.workflow,
        )
        match = re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        source_program = textwrap.dedent(match.group(1))
        program = self._workflow_program(match)
        # The extracted fixture rewrites wall-clock access to a fake clock so
        # polling tests cannot sleep in real time.  Assert production pacing
        # against the original snippet and the deterministic adaptation
        # against the transformed program.
        for value in (
            "write_clock=[time.monotonic()+8.1]",
            "time.sleep(delay)",
            "delay=write_clock[0]-time.monotonic()",
            "write_clock[0]=time.monotonic()+8.1",
        ):
            self.assertIn(value, source_program)
        self.assertIn("time = _krr_clock", program)
        self.assertIn("_krr_sleep(delay)", program)
        writer = (ROOT / ".github/workflows/pr-governance-status-writer.yml").read_text(encoding="utf-8")
        self.assertIn("cancel-in-progress: ${{ inputs.scope == 'early' }}", writer)

    def test_invalidator_reopens_terminal_trusted_checks_but_marks_carry_only_for_pending_dispatcher_state(self) -> None:
        match = re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = self._workflow_program(match)
        self.assertIn('external_id=f"krr-governance/v1/{head.lower()}/dispatcher-{dispatcher}"', program)
        self.assertIn('command=["gh","api","--method","POST"', program)
        self.assertNotIn('"--method","PATCH"', program)
        self.assertIn('type(draft) is not bool', program)
        self.assertIn('"carry_pending":str(carry_pending)', program)

    def test_invalidator_pendingizes_a_draft_before_failing_closed_on_terminal_carry(self) -> None:
        match = re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = self._workflow_program(match).replace("time.sleep(delay)", "None").replace("write_clock=[time.monotonic()+8.1]", "write_clock=[time.monotonic()]")
        head = "a" * 40
        prior = {
            "id": 101, "created_at": "2026-08-30T00:00:00Z", "app": {"id": 42}, "name": "KRR / PR governance (trusted check)",
            "head_sha": head, "external_id": "krr-governance/v1/" + head + "/dispatcher-9",
            "status": "in_progress", "conclusion": None,
            "details_url": "https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=1",
        }
        current = {**prior, "details_url": "https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=0"}
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); fake = directory / "gh"; log = directory / "patch.log"
            fake.write_text(
                "#!/bin/sh\ncase \"$*\" in\n"
                f"  *'--method POST'*) echo \"$*\" >> '{log}'; printf '%s' '{json.dumps(current)}' ;;\n"
                f"  *'check-runs/101'*) printf '%s' '{json.dumps(current)}' ;;\n"
                f"  *'check-runs?'*) printf '%s' '{json.dumps([{'check_runs': [prior]}])}' ;;\n"
                f"  *'/pulls/72'*) printf '%s' '{{\"draft\":true,\"head\":{{\"sha\":\"{head}\"}}}}' ;;\n"
                "  *) exit 91 ;;\nesac\n", encoding="utf-8",
            ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            environment = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "9",
                "GH_TOKEN": "read", "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "DUPLICATE_GOVERNED_HEADS": "[]", "AFFECTED": "[72]",
                "KNOWN_TARGET_SNAPSHOTS": json.dumps([[72, head, True]]),
                "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            result = subprocess.run([sys.executable, "-c", program], env=environment, capture_output=True, text=True, check=False)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(len(log.read_text(encoding="utf-8").splitlines()), 1)

    def test_invalidator_selects_latest_generation_beyond_first_page_with_dynamic_history_scan(self) -> None:
        """A newer generation beyond page one must supersede an old carry."""
        match = re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = self._workflow_program(match).replace("time.sleep(delay)", "None").replace("write_clock=[time.monotonic()+8.1]", "write_clock=[time.monotonic()]")
        self.assertIn('"filter":"all"', program)
        self.assertNotIn('"--paginate"', program)
        self.assertIn("prior_scan_max_pages=181", program)
        self.assertIn("prior_scan_page_timeout_seconds=15", program)
        self.assertIn("prior_scan_deadline=prior_scan_started+prior_scan_page_timeout_seconds*(page_count+1)", program)
        second_match = re.search(
            r"- name: Invalidate current pull requests for the all-open writer \(second TTL-safe chunk\).*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(second_match); assert second_match is not None
        second_program = self._workflow_program(second_match)
        self.assertIn('"filter":"all"', second_program)
        self.assertNotIn('"--paginate"', second_program)
        head = "a" * 40
        prior = {
            "id": 101, "created_at": "2026-08-30T00:00:00Z", "app": {"id": 42}, "name": "KRR / PR governance (trusted check)",
            "head_sha": head, "external_id": f"krr-governance/v1/{head}/dispatcher-8",
            "status": "in_progress", "conclusion": None,
            "details_url": "https://github.com/owner/repository/actions/runs/8?dispatcher_run_id=8&carry_pending=1",
        }
        newer = {**prior, "id": 102, "created_at": "2026-08-30T00:00:01Z", "external_id": f"krr-governance/v1/{head}/dispatcher-7", "details_url": "https://github.com/owner/repository/actions/runs/7?dispatcher_run_id=7&carry_pending=0"}
        current = {**prior, "id": 103, "external_id": f"krr-governance/v1/{head}/dispatcher-9", "details_url": "https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=0"}
        page_one = {"total_count": 601, "check_runs": [prior] + [{"id": value, "name": "unrelated"} for value in range(200, 299)]}
        pages = {1: page_one, 2: {"total_count": 601, "check_runs": [newer] + [{"id": value, "name": "unrelated"} for value in range(300, 399)]}}
        pages.update({page: {"total_count": 601, "check_runs": [{"id": value, "name": "unrelated"} for value in range(400 + (page - 3) * 100, 500 + (page - 3) * 100)]} for page in range(3, 7)})
        pages[7] = {"total_count": 601, "check_runs": [{"id": 999, "name": "unrelated"}]}
        calls: list[str] = []
        post_arguments: list[str] = []

        def response(value: object) -> subprocess.CompletedProcess[str]:
            return subprocess.CompletedProcess([], 0, json.dumps(value), "")

        def fake_run(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
            nonlocal post_arguments
            endpoint = next((value for value in arguments if isinstance(value, str) and value.startswith("repos/")), "")
            calls.append(endpoint)
            if "/check-runs?" in endpoint:
                page = int(parse_qs(urlparse(endpoint).query)["page"][0])
                return response(pages[page])
            if arguments[:3] == ["gh", "api", "--method"]:
                post_arguments = arguments
                return response(current)
            if endpoint.endswith("/check-runs/103"):
                return response(current)
            if endpoint.endswith("/pulls/72"):
                return response({"draft": False, "head": {"sha": head}})
            raise AssertionError(arguments)

        with tempfile.TemporaryDirectory() as temporary:
            environment = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "9",
                "GH_TOKEN": "read", "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "DUPLICATE_GOVERNED_HEADS": "[]",
                "AFFECTED": "[72]", "KNOWN_TARGET_SNAPSHOTS": json.dumps([[72, head, False]]), "GITHUB_OUTPUT": str(Path(temporary) / "output"),
            }
            with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run):
                namespace: dict[str, object] = {"__name__": "__main__"}
                exec(program, namespace)
        self.assertEqual(sum("/check-runs?" in call for call in calls), 8)
        self.assertEqual(
            [int(parse_qs(urlparse(call).query)["page"][0]) for call in calls if "/check-runs?" in call],
            [1, 2, 3, 4, 5, 6, 7, 1],
        )
        self.assertIn("details_url=https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=0", post_arguments)

    def test_invalidator_carry_scan_converges_for_the_entire_advertised_page_window(self) -> None:
        """The 181-page contract has a proportional deadline, not an impossible 20s cap."""

        match = re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = self._workflow_program(match).replace("time.sleep(delay)", "None").replace("write_clock=[time.monotonic()+8.1]", "write_clock=[time.monotonic()]")
        prefix = program[:program.index("# This is intentionally before every")]
        head = "a" * 40
        total, page_count = 18_100, 181
        pages: list[int] = []
        timeouts: list[float] = []
        latest = {
            "id": 900_001, "created_at": "2026-08-30T00:00:01Z", "app": {"id": 42},
            "name": "KRR / PR governance (trusted check)", "head_sha": head,
            "external_id": f"krr-governance/v1/{head}/dispatcher-8",
            "status": "in_progress", "conclusion": None,
            "details_url": "https://github.com/owner/repository/actions/runs/8?dispatcher_run_id=8&carry_pending=1",
        }

        def fake_run(arguments: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
            endpoint = next((item for item in arguments if isinstance(item, str) and "/check-runs?" in item), "")
            self.assertTrue(endpoint, arguments)
            page = int(parse_qs(urlparse(endpoint).query)["page"][0])
            self.assertGreaterEqual(page, 1)
            self.assertLessEqual(page, page_count)
            pages.append(page)
            timeout = kwargs.get("timeout")
            self.assertIsInstance(timeout, (int, float))
            assert isinstance(timeout, (int, float))
            timeouts.append(float(timeout))
            first_identifier = (page - 1) * 100 + 1
            entries: list[dict[str, object]] = [
                {"id": identifier, "name": "unrelated"}
                for identifier in range(first_identifier, first_identifier + 100)
            ]
            if page == page_count:
                entries[0] = latest
            payload = json.dumps({"total_count": total, "check_runs": entries})
            if "--include" in arguments:
                self.assertEqual(page, page_count)
                payload = "HTTP/2 200 OK\n\n" + payload
            return subprocess.CompletedProcess(arguments, 0, payload, "")

        environment = os.environ | {
            "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "9",
            "GH_TOKEN": "read", "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "DUPLICATE_GOVERNED_HEADS": "[]",
            "AFFECTED": "[72]", "KNOWN_TARGET_SNAPSHOTS": json.dumps([[72, head, False]]), "PATH": os.environ["PATH"],
        }
        with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run):
            namespace: dict[str, object] = {"__name__": "__main__"}
            exec(prefix, namespace)
            self.assertEqual(namespace["prior_carry_pending"](head), 1)
        self.assertEqual(pages, [*range(1, page_count + 1), 1])
        self.assertTrue(all(timeout == 15.0 for timeout in timeouts))

    def test_invalidator_fences_a_single_page_link_next_as_truncated(self) -> None:
        """Even a one-page result must reject a contradictory pagination fence."""
        match = re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = self._workflow_program(match).replace("time.sleep(delay)", "None").replace("write_clock=[time.monotonic()+8.1]", "write_clock=[time.monotonic()]")
        calls: list[list[str]] = []
        head = "a" * 40
        current = {
            "id": 103, "app": {"id": 42}, "name": "KRR / PR governance (trusted check)", "head_sha": head,
            "external_id": f"krr-governance/v1/{head}/dispatcher-9", "status": "in_progress", "conclusion": None,
            "details_url": "https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=0",
        }

        def fake_run(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
            calls.append(arguments)
            endpoint = next((value for value in arguments if isinstance(value, str) and value.startswith("repos/")), "")
            if "/check-runs?" in endpoint:
                payload = json.dumps({"total_count": 0, "check_runs": []})
                if "--include" in arguments:
                    payload = 'HTTP/2 200 OK\nLink: <https://api.github.com/next>; rel="next"\n\n' + payload
                return subprocess.CompletedProcess(arguments, 0, payload, "")
            if arguments[:3] == ["gh", "api", "--method"]:
                return subprocess.CompletedProcess(arguments, 0, json.dumps(current), "")
            if endpoint.endswith("/check-runs/103"):
                return subprocess.CompletedProcess(arguments, 0, json.dumps(current), "")
            if endpoint.endswith("/pulls/72"):
                return subprocess.CompletedProcess(arguments, 0, json.dumps({"draft": False, "head": {"sha": head}}), "")
            raise AssertionError(arguments)

        with tempfile.TemporaryDirectory() as temporary:
            environment = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "9",
                "GH_TOKEN": "read", "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "DUPLICATE_GOVERNED_HEADS": "[]",
                "AFFECTED": "[72]", "KNOWN_TARGET_SNAPSHOTS": json.dumps([[72, head, False]]), "GITHUB_OUTPUT": str(Path(temporary) / "output"),
            }
            with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run):
                with self.assertRaises(SystemExit):
                    exec(program, {"__name__": "__main__"})
        self.assertEqual(sum(any("/check-runs?" in item for item in call) for call in calls), 2)
        self.assertTrue(any(any("/check-runs?" in item for item in call) and "--include" in call for call in calls))

    def test_invalidator_rejects_a_read_that_reaches_the_deadline(self) -> None:
        """A near-deadline API response cannot authorize a carry decision."""
        match = re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = self._workflow_program(match).replace("time.sleep(delay)", "None").replace("write_clock=[time.monotonic()+8.1]", "write_clock=[0]")
        prefix = program[:program.index("# This is intentionally before every")]
        timeouts: list[float] = []
        clock_reads: list[int] = []
        head = "a" * 40

        def clock() -> int:
            values = [0, 29, 31]
            value = values[min(len(clock_reads), len(values) - 1)]
            clock_reads.append(value)
            return value

        def fake_run(arguments: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
            if "/check-runs?" in " ".join(str(item) for item in arguments):
                timeout = kwargs.get("timeout")
                self.assertIsInstance(timeout, (int, float))
                timeouts.append(timeout)
                return subprocess.CompletedProcess(arguments, 0, json.dumps({"total_count": 0, "check_runs": []}), "")
            raise AssertionError(arguments)

        environment = os.environ | {
            "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "9",
            "GH_TOKEN": "read", "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "DUPLICATE_GOVERNED_HEADS": "[]",
            "AFFECTED": "[72]", "KNOWN_TARGET_SNAPSHOTS": json.dumps([[72, head, False]]),
        }
        with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run):
            namespace: dict[str, object] = {"__name__": "__main__"}
            exec(prefix, namespace)
            namespace["_krr_clock"].monotonic = clock  # type: ignore[attr-defined]
            self.assertIsNone(namespace["prior_carry_pending"](head), (timeouts, clock_reads))
        self.assertEqual(timeouts, [1.0])

    def test_invalidator_carries_pending_tail_across_104_to_600_open_prs(self) -> None:
        """Every later all-open generation inherits a valid pending tail, not just its first page."""
        match = re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = self._workflow_program(match).replace("time.sleep(delay)", "None").replace("write_clock=[time.monotonic()+8.1]", "write_clock=[time.monotonic()]")
        for total in (104, 300, 451, 600):
            with self.subTest(total=total), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); posted: dict[int, dict[str, object]] = {}; writes: list[list[str]] = []
                def response(value: object) -> subprocess.CompletedProcess[str]:
                    return subprocess.CompletedProcess([], 0, json.dumps(value), "")
                def fake_run(arguments: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                    endpoint = arguments[-1]
                    if isinstance(endpoint, str) and "/pulls/" in endpoint:
                        number = int(endpoint.rsplit("/", 1)[1]); return response({"draft": False, "head": {"sha": f"{number:040x}"}})
                    if isinstance(endpoint, str) and "check-runs?" in endpoint:
                        head = endpoint.split("/commits/", 1)[1].split("/", 1)[0]
                        return response([{"check_runs": [{"id": 500_000 + int(head, 16), "created_at": "2026-08-30T00:00:00Z", "app": {"id": 42}, "name": "KRR / PR governance (trusted check)", "head_sha": head, "external_id": f"krr-governance/v1/{head}/dispatcher-8", "status": "in_progress", "conclusion": None, "details_url": "https://github.com/owner/repository/actions/runs/8?dispatcher_run_id=8&carry_pending=1"}]}])
                    if "--method" in arguments and "POST" in arguments:
                        fields = {item.split("=", 1)[0]: item.split("=", 1)[1] for item in arguments if "=" in item}
                        identifier = 1_000_000 + int(fields["head_sha"], 16)
                        value = {"id": identifier, "app": {"id": 42}, "name": "KRR / PR governance (trusted check)", "head_sha": fields["head_sha"], "external_id": fields["external_id"], "status": "in_progress", "conclusion": None, "details_url": fields["details_url"]}
                        posted[identifier] = value; writes.append(arguments)
                        self.assertEqual(kwargs["env"], {"GH_TOKEN": "write", "PATH": os.environ["PATH"]})
                        return response(value)
                    if isinstance(endpoint, str) and "/check-runs/" in endpoint:
                        return response(posted[int(endpoint.rsplit("/", 1)[1])])
                    raise AssertionError(arguments)
                numbers = list(range(1, total + 1))
                for chunk_index in range(0, len(numbers), 300):
                    chunk = numbers[chunk_index:chunk_index + 300]
                    output = directory / f"output-{chunk_index}"
                    environment = os.environ | {
                        "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "9",
                        "GH_TOKEN": "read", "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "DUPLICATE_GOVERNED_HEADS": "[]", "AFFECTED": json.dumps(chunk),
                        "KNOWN_TARGET_SNAPSHOTS": json.dumps([[number, f"{number:040x}", False] for number in chunk]),
                        "GITHUB_OUTPUT": str(output), "PATH": os.environ["PATH"],
                    }
                    with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run):
                        namespace: dict[str, object] = {"__name__": "__main__"}
                        exec(program, namespace)
                    manifest = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())["check_manifest"]
                    self.assertEqual(len(json.loads(manifest)), len(chunk))
                self.assertEqual(len(writes), total)
                self.assertTrue(all("details_url=https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=1" in write for write in writes))

    def test_compact_check_manifest_stays_within_the_workflow_dispatch_payload_limit(self) -> None:
        # `[pr,check_run_id]` keeps a 600-PR all-open dispatch well below the
        # 65,535-byte GitHub workflow_dispatch input ceiling.
        manifest = json.dumps([[number, 9_000_000_000_000_000_000 + number] for number in range(1, 601)], separators=(",", ":"))
        self.assertLess(len(manifest.encode("utf-8")), 65_535)
        self.assertIn('inputs[check_manifest]={raw_manifest}', self.workflow)

    def test_invalidator_replaces_duplicate_heads_and_pendingizes_unique_known_heads(self) -> None:
        match = re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = self._workflow_program(match).replace("time.sleep(delay)", "None").replace("write_clock=[time.monotonic()+8.1]", "write_clock=[time.monotonic()]")
        head, unique_head = "a" * 40, "b" * 40
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); fake = directory / "gh"; post = directory / "post"
            response = {
                "id": 101, "app": {"id": 42}, "name": "KRR / PR governance (trusted check)",
                "head_sha": head, "external_id": f"krr-governance/v1/{head}/dispatcher-9",
                "status": "in_progress", "conclusion": None,
                "details_url": "https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=0",
            }
            fake.write_text(
                "#!/bin/sh\ncase \"$*\" in\n"
                f"  *'/pulls/72'|*'/pulls/73'*) printf '%s' '{{\"draft\":false,\"head\":{{\"sha\":\"{head}\"}}}}' ;;\n"
                f"  *'/pulls/74'*) printf '%s' '{{\"draft\":false,\"head\":{{\"sha\":\"{unique_head}\"}}}}' ;;\n"
                f"  *'check-runs/101'*) printf '%s' '{json.dumps(response)}' ;;\n"
                f"  *'--method POST'*) echo \"$*\" >> '{post}'; printf '%s' '{json.dumps(response)}' ;;\n"
                "  *) exit 91 ;;\nesac\n",
                encoding="utf-8",
            ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            environment = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "9",
                "GH_TOKEN": "read", "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "DUPLICATE_GOVERNED_HEADS": json.dumps([head]), "AFFECTED": "[72,73,74]",
                "KNOWN_TARGET_SNAPSHOTS": json.dumps([[72, head, False], [73, head, False], [74, unique_head, False]]),
                "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            result = subprocess.run([sys.executable, "-c", program], env=environment, capture_output=True, text=True, check=False)
            self.assertNotEqual(result.returncode, 0)
            writes = post.read_text(encoding="utf-8").splitlines()
            self.assertEqual(len(writes), 2)
            self.assertIn(f"head_sha={head}", writes[0])
            self.assertIn(f"external_id=krr-governance/v1/{head}/dispatcher-9", writes[0])
            self.assertIn("status=in_progress", writes[0])
            self.assertTrue(any(f"head_sha={unique_head}" in write for write in writes))

    def test_event_source_and_duplicate_heads_are_invalidated_after_an_earlier_failure(self) -> None:
        match = re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = self._workflow_program(match).replace("time.sleep(delay)", "None").replace("write_clock=[time.monotonic()+8.1]", "write_clock=[time.monotonic()]")
        source_head, duplicate_head, unrelated_head = "c" * 40, "a" * 40, "b" * 40
        response = {
            "id": 102, "app": {"id": 42}, "name": "KRR / PR governance (trusted check)",
            "head_sha": duplicate_head, "external_id": f"krr-governance/v1/{duplicate_head}/dispatcher-9",
            "status": "in_progress", "conclusion": None,
            "details_url": "https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=0",
        }
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); fake = directory / "gh"; log = directory / "posts"
            fake.write_text(
                "#!/bin/sh\ncase \"$*\" in\n"
                f"  *'/pulls/72'*) printf '%s' '{{\"draft\":false,\"head\":{{\"sha\":\"{source_head}\"}}}}' ;;\n"
                f"  *'/pulls/73'|*'/pulls/74'*) printf '%s' '{{\"draft\":false,\"head\":{{\"sha\":\"{duplicate_head}\"}}}}' ;;\n"
                f"  *'/pulls/75'*) printf '%s' '{{\"draft\":false,\"head\":{{\"sha\":\"{unrelated_head}\"}}}}' ;;\n"
                f"  *'check-runs/102'*) printf '%s' '{json.dumps(response)}' ;;\n"
                f"  *'--method POST'*) echo \"$*\" >> '{log}'; case \"$*\" in *'head_sha={source_head}'*) exit 7 ;; *) printf '%s' '{json.dumps(response)}' ;; esac ;;\n"
                "  *) exit 91 ;;\nesac\n",
                encoding="utf-8",
            ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            environment = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "9",
                "GH_TOKEN": "read", "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "DUPLICATE_GOVERNED_HEADS": json.dumps([duplicate_head]), "AFFECTED": "[72,73,74,75]", "EVENT_TARGETS": "[72]",
                "KNOWN_TARGET_SNAPSHOTS": json.dumps([[72, source_head, False], [73, duplicate_head, False], [74, duplicate_head, False], [75, unrelated_head, False]]),
                "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            result = subprocess.run([sys.executable, "-c", program], env=environment, capture_output=True, text=True, check=False)
            self.assertNotEqual(result.returncode, 0)
            writes = log.read_text(encoding="utf-8").splitlines()
            self.assertEqual(len(writes), 3)
            self.assertEqual(sum(f"head_sha={source_head}" in write for write in writes), 1)
            self.assertEqual(sum(f"head_sha={duplicate_head}" in write for write in writes), 1)
            self.assertEqual(sum(f"head_sha={unrelated_head}" in write for write in writes), 1)

    def test_invalidator_pendingizes_all_known_heads_before_a_refresh_failure(self) -> None:
        match = re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = self._workflow_program(match).replace("time.sleep(delay)", "None").replace("write_clock=[time.monotonic()+8.1]", "write_clock=[time.monotonic()]")
        first_head, second_head = "a" * 40, "b" * 40
        calls: list[list[str]] = []; posted: dict[int, dict[str, object]] = {}

        def response(value: object, code: int = 0) -> subprocess.CompletedProcess[str]:
            return subprocess.CompletedProcess([], code, json.dumps(value), "")

        def fake_run(arguments: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
            calls.append(arguments)
            endpoint = arguments[-1]
            if isinstance(endpoint, str) and "check-runs?" in endpoint:
                return response([])
            if "--method" in arguments and "POST" in arguments:
                fields = {item.split("=", 1)[0]: item.split("=", 1)[1] for item in arguments if "=" in item}
                identifier = 100 + len(posted)
                check = {
                    "id": identifier, "app": {"id": 42}, "name": "KRR / PR governance (trusted check)",
                    "head_sha": fields["head_sha"], "external_id": fields["external_id"],
                    "status": "in_progress", "conclusion": None, "details_url": fields["details_url"],
                }
                posted[identifier] = check
                return response(check)
            if isinstance(endpoint, str) and "/check-runs/" in endpoint:
                return response(posted[int(endpoint.rsplit("/", 1)[1])])
            if isinstance(endpoint, str) and endpoint.endswith("/pulls/72"):
                return response({}, 7)
            if isinstance(endpoint, str) and endpoint.endswith("/pulls/73"):
                return response({"draft": False, "head": {"sha": second_head}})
            raise AssertionError(arguments)

        environment = os.environ | {
            "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "9",
            "GH_TOKEN": "read", "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "DUPLICATE_GOVERNED_HEADS": "[]", "AFFECTED": "[72,73]",
            "KNOWN_TARGET_SNAPSHOTS": json.dumps([[72, first_head, False], [73, second_head, False]]),
            "GITHUB_OUTPUT": str(Path(tempfile.gettempdir()) / "krr-invalidator-output"), "PATH": os.environ["PATH"],
        }
        with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run):
            with self.assertRaises(SystemExit):
                exec(program, {"__name__": "__main__"})
        post_indexes = [index for index, arguments in enumerate(calls) if "--method" in arguments and "POST" in arguments]
        refresh_indexes = [index for index, arguments in enumerate(calls) if arguments[-1] in {"repos/owner/repository/pulls/72", "repos/owner/repository/pulls/73"}]
        self.assertEqual(len(post_indexes), 2)
        self.assertEqual(len(refresh_indexes), 2)
        self.assertLess(max(post_indexes), min(refresh_indexes))
        self.assertEqual({check["head_sha"] for check in posted.values()}, {first_head, second_head})

    def test_writer_drain_handles_historical_and_completed_races_but_fails_closed_otherwise(self) -> None:
        match = re.search(
            r"- name: Drain authoritative writer before the next governance hand-off.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = self._workflow_program(match).replace("time.sleep(2)", "None")
        head = "a" * 40
        valid = {
            "id": 7, "name": "PR governance status writer",
            "path": ".github/workflows/pr-governance-status-writer.yml@master",
            "event": "workflow_dispatch", "head_sha": head,
            "workflow_id": 44, "repository": {"full_name": "owner/repository"},
            "run_number": 1, "run_attempt": 1, "status": "in_progress",
        }
        for mode, expected in (("cancel-failure", 1), ("timeout", 1), ("bad-identity", 1), ("old-head", 0), ("already-completed", 0)):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); fake = directory / "gh"; marker = directory / "cancelled"
                bad = {**valid, "name": "unexpected writer"}
                fake.write_text(
                    "#!/bin/sh\ncase \"$*\" in\n"
                    "  *'/actions/runs/7/cancel'*) case \"${MODE}\" in cancel-failure|already-completed) exit 7 ;; esac; touch \"${MARKER}\" ;;\n"
                    "  *'/actions/runs/7'*) printf '%s' \"${POLL}\" ;;\n"
                    "  *'actions/workflows/44/runs?'*) case \"$*\" in\n"
                    "    *'event=workflow_dispatch&branch=master&status=in_progress&per_page=100'*) printf '%s' \"${RUNS}\" ;;\n"
                    "    *'event=workflow_dispatch&branch=master&status=requested&per_page=100'*) printf '%s' '{\"total_count\":0,\"workflow_runs\":[]}' ;;\n"
                    "    *'event=workflow_dispatch&branch=master&status=queued&per_page=100'*) printf '%s' '{\"total_count\":0,\"workflow_runs\":[]}' ;;\n"
                    "    *'event=workflow_dispatch&branch=master&status=pending&per_page=100'*) printf '%s' '{\"total_count\":0,\"workflow_runs\":[]}' ;;\n"
                    "    *'event=workflow_dispatch&branch=master&status=waiting&per_page=100'*) printf '%s' '{\"total_count\":0,\"workflow_runs\":[]}' ;;\n"
                    "    *) exit 93 ;; esac ;;\n"
                    "  *'actions/workflows/pr-governance-status-writer.yml'*) printf '%s' '{\"id\":44}' ;;\n"
                    f"  *'git/ref/heads/master'*) printf '%s' '{{\"object\":{{\"sha\":\"{head}\"}}}}' ;;\n"
                    "  *'repos/owner/repository'*) printf '%s' '{\"default_branch\":\"master\"}' ;;\n"
                    "  *) exit 91 ;;\nesac\n", encoding="utf-8",
                )
                fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                listed_run = {**valid, "head_sha": "b" * 40} if mode == "old-head" else valid
                poll = valid if mode in {"timeout", "cancel-failure"} else bad if mode == "bad-identity" else {**listed_run, "status": "completed"}
                environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "GH_TOKEN": "actions-write", "MODE": mode,
                    "MARKER": str(marker), "RUNS": json.dumps({"total_count": 1, "workflow_runs": [listed_run]}),
                    "POLL": json.dumps(poll), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", program], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, expected, result.stderr)

    def test_writer_drain_filters_more_than_six_hundred_completed_runs_before_cancelling_active_writer(self) -> None:
        """Completed history must not consume the active-writer drain budget."""
        match = re.search(
            r"- name: Drain authoritative writer before the next governance hand-off.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = self._workflow_program(match)
        head = "a" * 40
        template: dict[str, object] = {
            "name": "PR governance status writer",
            "path": ".github/workflows/pr-governance-status-writer.yml@master",
            "event": "workflow_dispatch", "workflow_id": 44,
            "repository": {"full_name": "owner/repository"},
            "run_attempt": 1,
        }
        historical = [
            {
                **template, "id": identifier, "head_sha": "b" * 40,
                "run_number": identifier, "status": "completed",
            }
            for identifier in range(1, 602)
        ]
        active = {
            **template, "id": 1002, "head_sha": head,
            "run_number": 1002, "status": "in_progress",
        }
        queried_statuses: list[str] = []
        cancelled: list[int] = []

        def response(value: object, code: int = 0) -> subprocess.CompletedProcess[str]:
            return subprocess.CompletedProcess([], code, json.dumps(value), "")

        def fake_run(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
            endpoint = next((item for item in arguments if isinstance(item, str) and item.startswith("repos/")), "")
            if endpoint == "repos/owner/repository":
                return response({"default_branch": "master"})
            if endpoint == "repos/owner/repository/git/ref/heads/master":
                return response({"object": {"sha": head}})
            if endpoint == "repos/owner/repository/actions/workflows/pr-governance-status-writer.yml":
                return response({"id": 44})
            if endpoint.startswith("repos/owner/repository/actions/workflows/44/runs?"):
                query = parse_qs(urlparse(endpoint).query)
                self.assertEqual(set(query), {"event", "branch", "status", "per_page"})
                self.assertEqual(query["event"], ["workflow_dispatch"])
                self.assertEqual(query["branch"], ["master"])
                self.assertEqual(query["per_page"], ["100"])
                status = query["status"][0]
                self.assertIn(status, {"requested", "queued", "pending", "waiting", "in_progress"})
                queried_statuses.append(status)
                matching = [run for run in [*historical, active] if run["status"] == status]
                return response({"total_count": len(matching), "workflow_runs": matching})
            if endpoint == "repos/owner/repository/actions/runs/1002/cancel":
                cancelled.append(1002)
                active["status"] = "completed"
                return response({})
            if endpoint == "repos/owner/repository/actions/runs/1002":
                return response(active)
            raise AssertionError(arguments)

        environment = os.environ | {
            "GITHUB_REPOSITORY": "owner/repository", "GH_TOKEN": "actions-write",
            "PATH": os.environ["PATH"],
        }
        with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run):
            with self.assertRaises(SystemExit) as exited:
                exec(program, {"__name__": "__main__"})
        self.assertEqual(exited.exception.code, 0)
        self.assertEqual(len(historical), 601)
        self.assertEqual(cancelled, [1002])
        self.assertEqual(
            queried_statuses,
            ["requested", "queued", "pending", "waiting", "in_progress"] * 2,
        )

    def test_writer_drain_reconciles_a_normal_active_status_partition_transition(self) -> None:
        """A requested→queued transition restarts enumeration before one safe cancel."""

        match = re.search(
            r"- name: Drain authoritative writer before the next governance hand-off.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = self._workflow_program(match)
        head = "a" * 40
        template: dict[str, object] = {
            "id": 7, "name": "PR governance status writer",
            "path": ".github/workflows/pr-governance-status-writer.yml@master",
            "event": "workflow_dispatch", "head_sha": head, "workflow_id": 44,
            "repository": {"full_name": "owner/repository"},
            "run_number": 1, "run_attempt": 1,
        }
        listing_calls = 0
        queried_statuses: list[str] = []
        cancelled: list[int] = []
        current = {**template, "status": "queued"}

        def response(value: object) -> subprocess.CompletedProcess[str]:
            return subprocess.CompletedProcess([], 0, json.dumps(value), "")

        def fake_run(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
            nonlocal listing_calls
            endpoint = next((item for item in arguments if isinstance(item, str) and item.startswith("repos/")), "")
            if endpoint == "repos/owner/repository":
                return response({"default_branch": "master"})
            if endpoint == "repos/owner/repository/git/ref/heads/master":
                return response({"object": {"sha": head}})
            if endpoint == "repos/owner/repository/actions/workflows/pr-governance-status-writer.yml":
                return response({"id": 44})
            if endpoint.startswith("repos/owner/repository/actions/workflows/44/runs?"):
                status = parse_qs(urlparse(endpoint).query)["status"][0]
                queried_statuses.append(status)
                snapshot = listing_calls // 5
                listing_calls += 1
                # The first read sees the same run in two adjacent partitions;
                # this is the ordinary status transition that must restart.
                entries: list[dict[str, object]] = []
                if snapshot == 0 and status in {"requested", "queued"}:
                    entries = [{**template, "status": status}]
                elif snapshot > 0 and status == "queued":
                    entries = [{**template, "status": "queued"}]
                return response({"total_count": len(entries), "workflow_runs": entries})
            if endpoint == "repos/owner/repository/actions/runs/7/cancel":
                cancelled.append(7)
                current["status"] = "completed"
                return response({})
            if endpoint == "repos/owner/repository/actions/runs/7":
                return response(current)
            raise AssertionError(arguments)

        environment = os.environ | {
            "GITHUB_REPOSITORY": "owner/repository", "GH_TOKEN": "actions-write",
            "PATH": os.environ["PATH"],
        }
        with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run):
            with self.assertRaises(SystemExit) as exited:
                exec(program, {"__name__": "__main__"})
        self.assertEqual(exited.exception.code, 0)
        self.assertEqual(cancelled, [7])
        self.assertEqual(
            queried_statuses,
            ["requested", "queued", "pending", "waiting", "in_progress"] * 3,
        )

    def test_dispatch_waits_for_exact_new_writer_registration_and_rejects_gap_or_bad_identity(self) -> None:
        writer = (ROOT / ".github/workflows/pr-governance-status-writer.yml").read_text(encoding="utf-8")
        self.assertIn("run-name: source=${{ inputs.dispatcher_run_id }} scope=${{ inputs.scope }}", writer)
        match = re.search(
            r"- name: Dispatch one repository-wide governance arbiter.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        base_program = self._workflow_program(match).replace('subprocess.run(["sleep", "2"], check=False)', "None")
        valid = {
            "id": 71, "name": "PR governance status writer", "display_title": "source=99 scope=all segment=1",
            "path": ".github/workflows/pr-governance-status-writer.yml@master", "event": "workflow_dispatch",
            "repository": {"full_name": "owner/repository"}, "head_branch": "master", "head_sha": "a" * 40,
            "status": "queued", "run_number": 1, "run_attempt": 1,
        }
        for mode, expected in (("gap", 0), ("bad", 1), ("bad-attempt", 1), ("overbound", 1), ("incomplete-page", 1), ("timeout", 1)):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); fake = directory / "gh"; state = directory / "state"
                fake.write_text(
                    "#!/usr/bin/env python3\n"
                    "import json, os, sys\n"
                    "arguments = ' '.join(sys.argv[1:])\n"
                    "state = os.environ['STATE']\n"
                    "count = int(open(state).read()) if os.path.exists(state) else 0\n"
                    "if '/git/ref/heads/master' in arguments: print(json.dumps({'object': {'sha': 'a' * 40}}))\n"
                    "elif '/actions/runs/99' in arguments: print(os.environ['SOURCE'])\n"
                    "elif 'pulls?state=open' in arguments:\n"
                    "    pages = json.loads(os.environ['PULLS']); page = 1\n"
                    "    print(json.dumps(pages[page - 1] if page <= len(pages) else []))\n"
                    "elif arguments.endswith('repos/owner/repository'): print(json.dumps({'default_branch': 'master'}))\n"
                    "elif '/actions/workflows/pr-governance-status-writer.yml/runs?' in arguments:\n"
                    "    if 'event=workflow_dispatch' not in arguments or 'branch=master' not in arguments or 'head_sha=' + ('a' * 40) not in arguments or 'created=%3E%3D2026-09-01T00%3A00%3A00Z' not in arguments or 'per_page=100' not in arguments: raise SystemExit(93)\n"
                    "    open(state, 'w').write(str(count + 1))\n"
                    "    if os.environ['MODE'] == 'overbound':\n"
                    "        print(json.dumps({'total_count': 101, 'workflow_runs': []}))\n"
                    "    elif os.environ['MODE'] == 'incomplete-page':\n"
                    "        print(json.dumps({'total_count': 1, 'workflow_runs': []}))\n"
                    "    elif count < 2 or os.environ['MODE'] == 'timeout':\n"
                    "        print(json.dumps({'total_count': 0, 'workflow_runs': []}))\n"
                    "    else:\n"
                    "        run = json.loads(os.environ['RUN'])\n"
                    "        if os.environ['MODE'] == 'bad': run['head_sha'] = 'b' * 40\n"
                    "        if os.environ['MODE'] == 'bad-attempt': run['run_attempt'] = True\n"
                    "        unrelated = dict(run, id=70, display_title='source=other scope=all')\n"
                    "        print(json.dumps({'total_count': 2, 'workflow_runs': [unrelated, run]}))\n"
                    "elif '/dispatches' in arguments:\n"
                    "    if 'inputs[target_numbers]=[72,73]' not in arguments or 'inputs[preserved_target_numbers]=[]' not in arguments or 'inputs[preserved_writer_run_id]=0' not in arguments: raise SystemExit(92)\n"
                    "else:\n"
                    "    raise SystemExit(91)\n",
                    encoding="utf-8",
                ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                sleeper = directory / "sleep"
                sleeper.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
                sleeper.chmod(sleeper.stat().st_mode | stat.S_IXUSR)
                program = base_program.replace('subprocess.run(["sleep", str(min(5, remaining))], check=False)', "None")
                if mode == "timeout":
                    program = program.replace("range(60)", "range(2)")
                environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "WRITER_HEAD": "a" * 40,
                    "DISPATCHER_RUN_ID": "99", "WRITER_SCOPE": "all", "WRITER_TARGETS": "[72,73]",
                    "WRITER_PRESERVED_TARGETS": "[]", "PRESERVED_WRITER_RUN_ID": "0", "MODE": mode, "STATE": str(state), "RUN": json.dumps(valid),
                    "SOURCE": json.dumps({"id": 99, "name": "PR governance dispatcher", "event": "issues", "status": "in_progress", "run_number": 1, "run_attempt": 1, "head_branch": "master", "head_sha": "a" * 40, "repository": {"full_name": "owner/repository"}, "created_at": "2026-09-01T00:00:00Z"}),
                    "PULLS": json.dumps([[{"number": 72, "state": "open", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": "a" * 40, "repo": {"full_name": "owner/repository"}}, "draft": False}, {"number": 73, "state": "open", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": "b" * 40, "repo": {"full_name": "owner/repository"}}, "draft": False}]]),
                    "WRITER_ALL_OPEN_TARGETS": "[72,73]", "WRITER_ALL_OPEN_SNAPSHOTS": "[[72,\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",false],[73,\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\",false]]",
                    "WRITER_PREINVALIDATE_TARGETS": "[]", "WRITER_PRE_CHECK_MANIFEST_1": "[]", "WRITER_PRE_CHECK_MANIFEST_2": "[]",
                    "WRITER_TAIL_CHECK_MANIFEST_1": "[[72,701],[73,702]]", "WRITER_TAIL_CHECK_MANIFEST_2": "[]", "WRITER_PRESERVED_CHECK_MANIFEST": "[]",
                    "WRITER_CARRY_TARGET_NUMBERS_1": "[]", "WRITER_CARRY_TARGET_NUMBERS_2": "[]", "GITHUB_OUTPUT": str(directory / "output"),
                    "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", program], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, expected, result.stderr)

    def test_writer_registration_queries_are_filtered_bounded_and_not_paginated(self) -> None:
        """Every segment polls only the current dispatch generation's bounded window."""
        for name in (
            "Dispatch one repository-wide governance arbiter segment",
            "Dispatch second repository-wide governance arbiter segment",
            "Dispatch third repository-wide governance arbiter segment",
            "Dispatch fourth repository-wide governance arbiter segment",
        ):
            match = re.search(rf"^      - name: {re.escape(name)}\n(?P<body>.*?)(?=^      - name: |\Z)", self.workflow, re.MULTILINE | re.DOTALL)
            self.assertIsNotNone(match, name); assert match is not None
            body = match.group("body")
            registration = body[body.index("writer_runs_query"):]
            compact = re.sub(r"\s+", "", registration)
            self.assertIn('"event":"workflow_dispatch"', compact)
            self.assertIn('"head_sha"', compact)
            self.assertIn('"created"', compact)
            self.assertIn('"per_page":"100"', compact)
            self.assertNotIn('"--paginate"', registration)
            self.assertIn("total>100", compact)
            self.assertTrue("len(runs)!=total" in compact or "len(flattened)!=total" in compact)

    def test_invalidator_rejects_wrong_or_malformed_check_app_before_dispatch(self) -> None:
        match = re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match)
        assert match is not None
        program = self._workflow_program(match).replace("time.sleep(delay)", "None")
        base = {
            "id": 101, "app": {"id": 42}, "name": "KRR / PR governance (trusted check)",
            "head_sha": "a" * 40, "external_id": "krr-governance/v1/" + "a" * 40,
            "status": "in_progress", "conclusion": None,
            "details_url": "https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=0",
        }
        for response in ({**base, "app": {"id": 7}}, {**base, "id": "101"}, {**base, "app": {"id": True}}):
            with self.subTest(response=response), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary)
                fake = directory / "gh"
                fake.write_text(
                    "#!/bin/sh\ncase \"$*\" in\n"
                    "  *'check-runs?'*) printf '%s' '[{\"check_runs\":[]}]' ;;\n"
                    "  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n"
                    "  *'/pulls/72'*) printf '%s' \"${PULL}\" ;;\n"
                    "  *'--method POST'*) printf '%s' \"${POST}\" ;;\n"
                    "  *) exit 91 ;;\nesac\n", encoding="utf-8",
                )
                fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "9",
                    "GITHUB_OUTPUT": str(directory / "output"),
                    "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "PULLS": json.dumps([[{"number": 72, "state": "open"}]]),
                    "AFFECTED": "[72]",
                    "KNOWN_TARGET_SNAPSHOTS": json.dumps([[72, "a" * 40, False]]),
                    "PULL": json.dumps({"draft": False, "head": {"sha": "a" * 40}}), "POST": json.dumps(response),
                    "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", program], env=environment, capture_output=True, text=True, check=False)
                self.assertNotEqual(result.returncode, 0)

    def test_invalidator_has_no_all_open_cap_and_continues_after_a_post_failure(self) -> None:
        match = re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = self._workflow_program(match).replace("time.sleep(delay)", "None").replace("write_clock=[time.monotonic()+8.1]", "write_clock=[time.monotonic()]")
        self.assertNotIn("numbers[:", program)
        self.assertNotIn("len(numbers) >", program)
        # The large production-path regression lives above; this fixture
        # retains the single-write failure boundary without inventing a
        # duplicate head SHA that production now rejects before mutation.
        for total, failed, expected in ((1, "", 0), (1, "1", 1)):
            with self.subTest(total=total, failed=failed), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); log = directory / "post.log"; fake = directory / "gh"
                fake.write_text(
                    f"#!/bin/sh\necho \"${{GH_TOKEN}}:$*\" >> '{directory / 'calls.log'}'\ncase \"$*\" in\n"
                    "  *'check-runs/101'*) printf '%s' '{\"id\":101,\"app\":{\"id\":42},\"name\":\"KRR / PR governance (trusted check)\",\"head_sha\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"external_id\":\"krr-governance/v1/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/dispatcher-9\",\"status\":\"in_progress\",\"conclusion\":null,\"details_url\":\"https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=0\"}' ;;\n"
                    "  *'check-runs?'*) printf '%s' '[{\"check_runs\":[]}]' ;;\n"
                    "  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n"
                    f"  *'/pulls/'*) printf '%s' '{json.dumps({'number': 1, 'state': 'open', 'draft': False, 'base': {'ref': 'master', 'repo': {'full_name': 'owner/repository'}}, 'head': {'sha': 'a' * 40, 'repo': {'full_name': 'owner/repository'}}}, separators=(',', ':'))}' ;;\n"
                    "  *'--method POST'*)\n"
                    f"    echo \"$*\" >> '{log}'\n"
                    f"    count=$(awk 'END {{ print NR }}' '{log}')\n"
                    f"    if [ '{failed}' = \"$count\" ]; then exit 7; fi\n"
                    "    printf '%s' '{\"id\":101,\"app\":{\"id\":42},\"name\":\"KRR / PR governance (trusted check)\",\"head_sha\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"external_id\":\"krr-governance/v1/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/dispatcher-9\",\"status\":\"in_progress\",\"conclusion\":null,\"details_url\":\"https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=0\"}' ;;\n"
                    "  *) exit 91 ;;\nesac\n", encoding="utf-8",
                )
                fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "9",
                    "GITHUB_OUTPUT": str(directory / "output"),
                    # The invalidator now binds every list/reread to this
                    # explicit read token; the fixture must not accidentally
                    # rely on the ambient process credential.
                    "GH_TOKEN": "read", "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "DUPLICATE_GOVERNED_HEADS": "[]", "PULLS": "[]",
                    "AFFECTED": json.dumps(list(range(1, total + 1))),
                    "KNOWN_TARGET_SNAPSHOTS": json.dumps([[number, "a" * 40, False] for number in range(1, total + 1)]),
                    "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", program], env=environment, capture_output=True, text=True, check=False)
                if result.returncode != expected:
                    calls = directory / "calls.log"
                    self.fail(result.stdout + result.stderr + (calls.read_text(encoding="utf-8") if calls.exists() else ""))
                self.assertEqual(result.returncode, expected, result.stderr)
                calls = (directory / "calls.log").read_text(encoding="utf-8").splitlines()
                first_write = next(index for index, line in enumerate(calls) if line.startswith("write:"))
                self.assertGreater(first_write, 0)
                self.assertTrue(all(line.startswith("read:") for line in calls[:first_write]))
                if expected == 0:
                    self.assertTrue(all(line.startswith("read:") for line in calls[first_write + 1:]))
                if log.exists():
                    self.assertEqual(len(log.read_text(encoding="utf-8").splitlines()), total)

    def test_global_serialized_rate_model_bounds_barrier_and_check_writes_at_445_per_hour(self) -> None:
        pace_seconds = 8.1
        writer_first_write_delay = 8.1
        establish = self.workflow[
            self.workflow.index("  establish-resolver-failure-barrier:"):
            self.workflow.index("  resolve_event:")
        ]
        self.assertIn("check-runs/{identifier}", establish)
        self.assertIn("branches/{branch}/protection", establish)
        marker_names = (
            "Publish resolver-failure barrier App marker",
            "Publish periodic static affected-head barrier App marker",
        )
        for marker_name in marker_names:
            match = re.search(
                rf"- name: {re.escape(marker_name)}.*?python3 - <<'PY'\n(.*?)\n          PY",
                self.workflow, re.DOTALL,
            )
            self.assertIsNotNone(match, marker_name); assert match is not None
            program = textwrap.dedent(match.group(1))
            self.assertIn("import json, os, re, subprocess, time", program)
            self.assertIn("time.sleep(8.1)", program)

        # The shared generation lock puts the resolver-failure marker, its
        # normal reconciliation marker, invalidations, and the first writer
        # mutation on one 8.1s timeline. The marker re-read and protection
        # read/POST/read are explicit control-plane calls above; only Check
        # Run writes consume this 445/hour budget.
        barrier_writes = 2
        for all_open, expected_maximum in ((300, 303), (451, 445), (600, 445)):
            with self.subTest(all_open=all_open):
                marker_writes = [(index + 1) * pace_seconds for index in range(barrier_writes)]
                invalidator_writes = [(barrier_writes + index + 1) * pace_seconds for index in range(all_open)]
                writer_first_write = invalidator_writes[-1] + writer_first_write_delay if invalidator_writes else None
                writes = [*marker_writes, *invalidator_writes, *([writer_first_write] if writer_first_write is not None else [])]
                maximum = max(
                    sum(window_end - 3600 < write <= window_end for write in writes)
                    for window_end in writes
                )
                self.assertEqual(len(marker_writes), barrier_writes)
                self.assertEqual(len(invalidator_writes), all_open)
                self.assertTrue(all(
                    later - earlier >= pace_seconds - 1e-9
                    for earlier, later in zip(writes, writes[1:])
                ))
                if writer_first_write is not None:
                    self.assertGreaterEqual(writer_first_write - invalidator_writes[-1], writer_first_write_delay - 1e-9)
                self.assertEqual(maximum, expected_maximum)
                self.assertLessEqual(maximum, 445)

    def test_embedded_python_imports_modules_used_by_qualified_names(self) -> None:
        """Embedded workflow programs must import every referenced top-level module."""
        programs = re.findall(
            r"^          python3 - <<'PY'\n(.*?)^          PY$",
            self.workflow,
            re.MULTILINE | re.DOTALL,
        )
        self.assertGreater(len(programs), 0)
        for index, source in enumerate(programs, start=1):
            tree = ast.parse(textwrap.dedent(source), filename=f"workflow heredoc #{index}")
            imported = {
                alias.asname or alias.name.split(".", 1)[0]
                for node in ast.walk(tree)
                if isinstance(node, ast.Import)
                for alias in node.names
            }
            uses_time = any(
                isinstance(node, ast.Attribute)
                and isinstance(node.value, ast.Name)
                and node.value.id == "time"
                and isinstance(node.value.ctx, ast.Load)
                for node in ast.walk(tree)
            )
            if uses_time:
                self.assertIn("time", imported, f"workflow heredoc #{index} uses time without importing it")

    def test_large_priority_targets_are_resolved_into_complete_ttl_safe_chunks(self) -> None:
        """The resolver and every pre-invalidator cover 41 and 600 current PRs."""
        resolver = re.search(
            r"- name: Re-enumerate every current local governance pull request.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(resolver); assert resolver is not None
        pre_blocks = re.findall(
            r"- name: Pre-invalidate priority event heads.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        # One token pair cannot safely span 600 writes.  The workflow must
        # expose two independently tokenized 300-head pre-invalidation steps.
        self.assertGreaterEqual(len(pre_blocks), 2)
        self.assertIn("terminal_batch_numbers", self.workflow)
        self.assertIn("continuation_index", self.workflow)

        def pull(number: int) -> dict[str, object]:
            head = f"{number:040x}"
            return {
                "number": number,
                "state": "open",
                "body": "Fixes #64",
                "draft": False,
                "base": {"ref": "master", "repo": {"full_name": "owner/repository"}},
                "head": {"sha": head, "repo": {"full_name": "owner/repository"}},
            }

        for total in (41, 600):
            with self.subTest(total=total), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); output = directory / "output"
                pages = [
                    [pull(number) for number in range(start, min(start + 100, total + 1))]
                    for start in range(1, total + 1, 100)
                ]
                fake = directory / "gh"
                fake.write_text(
                    "#!/bin/sh\ncase \"$*\" in\n"
                    "  *'pulls?state=open'*) cat \"${PULLS_FILE}\" ;;\n"
                    "  *'git/ref/heads/master'*) printf '%s' '{\"object\":{\"sha\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}}' ;;\n"
                    "  *'repos/owner/repository'*) printf '%s' '{\"default_branch\":\"master\"}' ;;\n"
                    "  *) exit 91 ;;\nesac\n",
                    encoding="utf-8",
                )
                fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master",
                    "EVENT_NAME": "issues", "ISSUE_NUMBER": "64", "ISSUE_PULL_REQUEST_URL": "",
                    "EVENT_TARGETS": json.dumps(list(range(1, total + 1)), separators=(",", ":")),
                    "EVENT_PRIORITY_TARGETS": json.dumps(list(range(1, total + 1)), separators=(",", ":")),
                    "GITHUB_OUTPUT": str(output),
                    "PULLS_FILE": str(directory / "pulls.json"),
                    "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                pulls_json = json.dumps(pages, separators=(",", ":"))
                (directory / "pulls.json").write_text(pulls_json, encoding="utf-8")
                if total == 600:
                    self.assertGreater(len(pulls_json.encode("utf-8")), 128 * 1024)
                self.assertNotIn("PULLS", environment)
                self.assertTrue(all(len(os.fsencode(key)) + len(os.fsencode(value)) < 128 * 1024 for key, value in environment.items()))
                result = subprocess.run(
                    [sys.executable, "-c", self._workflow_program(resolver)],
                    env=environment, capture_output=True, text=True, check=False,
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
                targets = json.loads(values["targets"])
                priority = json.loads(values["priority_targets"])
                pre_targets = json.loads(values["preinvalidate_targets"])
                self.assertEqual(targets, list(range(1, total + 1)))
                self.assertEqual(pre_targets, targets)
                self.assertEqual(priority, list(range(1, min(total, 40) + 1)))
                chunks = [
                    json.loads(values[f"preinvalidate_chunk_{index}"])
                    for index in (1, 2)
                ]
                self.assertEqual([len(chunk) for chunk in chunks], [min(total, 300), max(0, total - 300)])
                self.assertEqual(chunks[0] + chunks[1], targets)
                snapshots = [
                    json.loads(values[f"preinvalidate_chunk_{index}_snapshots"])
                    for index in (1, 2)
                ]
                self.assertEqual([entry[0] for chunk in snapshots for entry in chunk], targets)
                self.assertTrue(all(len(chunk) <= 300 for chunk in chunks))

    def test_large_preinvalidation_posts_each_distinct_head_once_with_fresh_token_pair(self) -> None:
        """Both 300-head chunks must be executable and never duplicate an external ID."""
        resolver = re.search(
            r"- name: Re-enumerate every current local governance pull request.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(resolver); assert resolver is not None
        pre_blocks = re.findall(
            r"- name: Pre-invalidate priority event heads.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertGreaterEqual(len(pre_blocks), 2)
        total = 600
        heads = {number: f"{number:040x}" for number in range(1, total + 1)}
        pages = [[
            {
                "number": number, "state": "open", "body": "Fixes #64", "draft": False,
                "base": {"ref": "master", "repo": {"id": 101, "full_name": "owner/repository"}},
                "head": {"sha": heads[number], "repo": {"id": 101, "full_name": "owner/repository"}},
            }
            for number in range(start, min(start + 100, total + 1))
        ] for start in range(1, total + 1, 100)]
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); resolver_output = directory / "resolver-output"
            fake = directory / "gh"
            fake.write_text(
                "#!/bin/sh\ncase \"$*\" in\n"
                "  *'pulls?state=open'*) cat \"${PULLS_FILE}\" ;;\n"
                "  *'git/ref/heads/master'*) printf '%s' '{\"object\":{\"sha\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}}' ;;\n"
                "  *'repos/owner/repository'*) printf '%s' '{\"default_branch\":\"master\"}' ;;\n"
                "  *) exit 91 ;;\nesac\n",
                encoding="utf-8",
            )
            fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            resolver_environment = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master",
                "EVENT_NAME": "issues", "ISSUE_NUMBER": "64", "ISSUE_PULL_REQUEST_URL": "",
                "EVENT_TARGETS": json.dumps(list(heads), separators=(",", ":")),
                "EVENT_PRIORITY_TARGETS": json.dumps(list(heads), separators=(",", ":")),
                "GITHUB_OUTPUT": str(resolver_output), "PULLS_FILE": str(directory / "pulls.json"),
                "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            pulls_json = json.dumps(pages, separators=(",", ":"))
            (directory / "pulls.json").write_text(pulls_json, encoding="utf-8")
            self.assertGreater(len(pulls_json.encode("utf-8")), 128 * 1024)
            self.assertNotIn("PULLS", resolver_environment)
            self.assertTrue(all(len(os.fsencode(key)) + len(os.fsencode(value)) < 128 * 1024 for key, value in resolver_environment.items()))
            result = subprocess.run(
                [sys.executable, "-c", self._workflow_program(resolver)],
                env=resolver_environment, capture_output=True, text=True, check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            values = dict(line.split("=", 1) for line in resolver_output.read_text(encoding="utf-8").splitlines())
            posts: list[list[str]] = []
            created: dict[int, dict[str, object]] = {}
            read_tokens: set[str] = set()
            write_tokens: set[str] = set()

            def response(value: object, code: int = 0) -> subprocess.CompletedProcess[str]:
                return subprocess.CompletedProcess([], code, json.dumps(value), "")

            def fake_run(arguments: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                endpoint = arguments[-1]
                supplied = kwargs.get("env")
                if "--method" in arguments and "POST" in arguments:
                    self.assertIsInstance(supplied, dict)
                    assert isinstance(supplied, dict)
                    write_token = supplied.get("GH_TOKEN")
                    self.assertIsInstance(write_token, str)
                    assert isinstance(write_token, str)
                    self.assertRegex(write_token, r"^write-[12]$")
                    write_tokens.add(write_token)
                    self.assertEqual(supplied.get("PATH"), os.environ["PATH"])
                    posts.append(arguments)
                    fields = {item.split("=", 1)[0]: item.split("=", 1)[1] for item in arguments if "=" in item}
                    identifier = 10_000 + len(posts)
                    check: dict[str, object] = {
                        "id": identifier, "app": {"id": 42},
                        "name": "KRR / PR governance (trusted check)", "head_sha": fields["head_sha"],
                        "external_id": fields["external_id"], "status": "in_progress", "conclusion": None,
                        "details_url": fields["details_url"],
                    }
                    created[identifier] = check
                    return response(check)
                if isinstance(endpoint, str) and "/check-runs/" in endpoint:
                    self.assertIsInstance(supplied, dict)
                    assert isinstance(supplied, dict)
                    read_token = supplied.get("GH_TOKEN")
                    self.assertIsInstance(read_token, str)
                    assert isinstance(read_token, str)
                    self.assertRegex(read_token, r"^read-[12]$")
                    read_tokens.add(read_token)
                    self.assertEqual(supplied.get("PATH"), os.environ["PATH"])
                    return response(created[int(endpoint.rsplit("/", 1)[1])])
                if isinstance(endpoint, str) and "/pulls/" in endpoint:
                    self.assertIsInstance(supplied, dict)
                    assert isinstance(supplied, dict)
                    read_token = supplied.get("GH_TOKEN")
                    self.assertIsInstance(read_token, str)
                    assert isinstance(read_token, str)
                    self.assertRegex(read_token, r"^read-[12]$")
                    read_tokens.add(read_token)
                    self.assertEqual(supplied.get("PATH"), os.environ["PATH"])
                    number = int(endpoint.rsplit("/", 1)[1])
                    return response({
                        "number": number, "state": "open", "draft": False,
                        "base": {"ref": "master", "repo": {"full_name": "owner/repository"}},
                        "head": {"sha": heads[number], "repo": {"full_name": "owner/repository"}},
                    })
                raise AssertionError(arguments)

            for index, block in enumerate(pre_blocks[:2], 1):
                chunk = json.loads(values[f"preinvalidate_chunk_{index}"])
                if not chunk:
                    continue
                output = directory / f"pre-{index}-output"
                environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com",
                    "GH_TOKEN": f"read-{index}", "CHECK_WRITE_TOKEN": f"write-{index}", "CHECK_APP_ID": "42",
                    "TARGETS": values[f"preinvalidate_chunk_{index}"],
                    "TARGET_SNAPSHOTS": values[f"preinvalidate_chunk_{index}_snapshots"],
                    "DEFAULT_BRANCH": "master", "DISPATCHER_RUN_ID": "9", "GITHUB_OUTPUT": str(output),
                    "PATH": os.environ["PATH"],
                }
                # The production script must use a read-only token for every
                # reread and a different fresh write token for each chunk.
                with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run), patch("time.sleep"):
                    exec(textwrap.dedent(block), {"__name__": "__main__"})
            self.assertEqual(len(posts), total)
            self.assertEqual(len({next(item.split("=", 1)[1] for item in post if item.startswith("external_id=")) for post in posts}), total)
            self.assertEqual(read_tokens, {"read-1", "read-2"})
            self.assertEqual(write_tokens, {"write-1", "write-2"})
            self.assertEqual(
                [next(item.split("=", 1)[1] for item in post if item.startswith("head_sha=")) for post in posts],
                [heads[number] for number in range(1, total + 1)],
            )

    def test_oversized_all_invalidation_keeps_affected_prechunk_and_holds_barrier(self) -> None:
        """The bounded six-page snapshot carries 600 unique heads to both invalidation chunks."""
        resolver = re.search(
            r"- name: Re-enumerate every current local governance pull request.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(resolver); assert resolver is not None
        hold = re.search(
            r"- name: Hold global merge barrier for an oversized governed snapshot.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(hold); assert hold is not None
        activation = self.workflow.index("- name: Activate complete affected-head merge barrier")
        hold_position = self.workflow.index("- name: Hold global merge barrier for an oversized governed snapshot")
        dispatcher = self.workflow.index("- name: Create dispatcher App token after priority preinvalidation")
        self.assertLess(activation, hold_position)
        self.assertLess(hold_position, dispatcher)
        self.assertIn("invalidation_head_cap_exceeded == 'true'", self.workflow[hold_position:])

        total = 600

        def pull(number: int) -> dict[str, object]:
            return {
                "number": number,
                "state": "open",
                "body": "Fixes #64",
                "draft": False,
                "base": {"ref": "master", "repo": {"full_name": "owner/repository"}},
                "head": {"sha": f"{number:040x}", "repo": {"full_name": "owner/repository"}},
            }

        pages = [[pull(number) for number in range(start, min(start + 100, total + 1))] for start in range(1, total + 1, 100)]
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            output = directory / "output"
            pulls_file = directory / "pulls.json"
            pulls_json = json.dumps(pages, separators=(",", ":"))
            pulls_file.write_text(pulls_json, encoding="utf-8")
            fake = directory / "gh"
            fake.write_text(
                "#!/bin/sh\ncase \"$*\" in\n"
                "  *'pulls?state=open'*) cat \"${PULLS_FILE}\" ;;\n"
                "  *'git/ref/heads/master'*) printf '%s' '{\"object\":{\"sha\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}}' ;;\n"
                "  *'repos/owner/repository'*) printf '%s' '{\"default_branch\":\"master\"}' ;;\n"
                "  *) exit 91 ;;\nesac\n",
                encoding="utf-8",
            )
            fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            environment = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository",
                "DEFAULT_BRANCH": "master",
                "EVENT_NAME": "pull_request_target",
                "EVENT_TARGETS": "[1]",
                "EVENT_PRIORITY_TARGETS": "[1]",
                "GITHUB_OUTPUT": str(output),
                "PULLS_FILE": str(pulls_file),
                "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            self.assertGreater(len(pulls_json.encode("utf-8")), 128 * 1024)
            self.assertNotIn("PULLS", environment)
            self.assertTrue(all(len(os.fsencode(key)) + len(os.fsencode(value)) < 128 * 1024 for key, value in environment.items()))
            result = subprocess.run(
                [sys.executable, "-c", self._workflow_program(resolver)],
                env=environment,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
            targets = json.loads(values["targets"])
            snapshots = json.loads(values["target_snapshots"])
            self.assertEqual(targets, list(range(1, total + 1)))
            self.assertEqual([entry[0] for entry in snapshots], targets)
            self.assertEqual(len(snapshots), total)
            self.assertEqual(json.loads(values["preinvalidate_targets"]), [1])
            self.assertEqual(json.loads(values["preinvalidate_chunk_1"]), [1])
            self.assertEqual(json.loads(values["preinvalidate_chunk_1_snapshots"])[0][0], 1)
            self.assertEqual(json.loads(values["preinvalidate_chunk_2"]), [])
            self.assertEqual(json.loads(values["preinvalidate_chunk_2_snapshots"]), [])
            self.assertEqual(json.loads(values["all_invalidation_targets"]), list(range(2, total + 1)))
            self.assertEqual(json.loads(values["all_invalidation_target_snapshots"]), [[number, f"{number:040x}", False] for number in range(2, total + 1)])
            self.assertEqual(json.loads(values["all_invalidation_chunk_1"]), list(range(2, 302)))
            self.assertEqual(json.loads(values["all_invalidation_chunk_2"]), list(range(302, 601)))
            self.assertEqual(values["invalidation_head_cap_exceeded"], "false")

        hold_program = self._workflow_program(hold)
        base_environment = {
            "BARRIER_SOURCE_OUTCOME": "success",
            "BARRIER_TOKEN_OUTCOME": "success",
            "BARRIER_ACTIVATE_OUTCOME": "success",
        }
        for active, message in (("false", "before the affected-head barrier was active"), ("true", "head cap exceeded")):
            with self.subTest(active=active):
                hold_environment = base_environment | {"BARRIER_ACTIVE": active}
                with patch.dict(os.environ, hold_environment, clear=True):
                    with self.assertRaisesRegex(SystemExit, message):
                        exec(hold_program, {"__name__": "__main__"})
                self.assertEqual(hold_environment["BARRIER_ACTIVE"], active)

    def test_oversized_snapshot_over_600_heads_fails_closed(self) -> None:
        match = re.search(
            r"- name: Re-enumerate every current local governance pull request.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        total = 602
        pages = [[
            {"number": number, "state": "open", "body": "Fixes #64", "draft": False,
             "base": {"ref": "master", "repo": {"full_name": "owner/repository"}},
             "head": {"sha": f"{number:040x}", "repo": {"full_name": "owner/repository"}}}
            for number in range(start, min(start + 100, total + 1))
        ] for start in range(1, total + 1, 100)]
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); output = directory / "output"; payload = directory / "pulls.json"
            payload.write_text(json.dumps(pages), encoding="utf-8")
            fake = directory / "gh"
            fake.write_text(
                "#!/bin/sh\ncase \"$*\" in\n"
                "  *'--include'*'pulls?state=open'*) printf 'HTTP/2 200 OK\\nLink: <https://api.github.com/repos/owner/repository/pulls?state=open&per_page=100&page=7>; rel=\"next\"\\n\\n'; cat \"${PULLS_FILE}\" ;;\n"
                "  *'pulls?state=open'*) cat \"${PULLS_FILE}\" ;;\n"
                "  *'git/ref/heads/master'*) printf '%s' '{\"object\":{\"sha\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}}' ;;\n"
                "  *'repos/owner/repository'*) printf '%s' '{\"default_branch\":\"master\"}' ;;\n"
                "  *) exit 91 ;;\nesac\n", encoding="utf-8",
            ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            environment = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master",
                "EVENT_NAME": "pull_request_target", "EVENT_TARGETS": "[1]",
                "EVENT_PRIORITY_TARGETS": "[1]", "GITHUB_OUTPUT": str(output),
                "PULLS_FILE": str(payload), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            result = subprocess.run(
                [sys.executable, "-c", self._workflow_program(match)], env=environment,
                capture_output=True, text=True, check=False,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("fixed page window", result.stderr.lower())

    def test_six_page_continuation_excludes_unavailable_fork_within_600_target_bound(self) -> None:
        match = re.search(
            r"- name: Re-enumerate every current local governance pull request.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        governed = list(range(1, 600))
        pages = [[
            {"number": number, "state": "open", "body": "Fixes #64", "draft": False,
             "base": {"ref": "master", "repo": {"full_name": "owner/repository"}},
             "head": {"sha": f"{number:040x}", "repo": {"full_name": "owner/repository"}}}
            for number in governed[start:start + 100]
        ] for start in range(0, len(governed), 100)]
        pages[-1].append({
            "number": 1001, "state": "open", "body": "Fixes #64", "draft": False,
            "base": {"ref": "master", "repo": {"full_name": "owner/repository"}},
            "head": {"sha": "f" * 40, "repo": None},
        })
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); output = directory / "output"; payload = directory / "pulls.json"
            payload.write_text(json.dumps(pages), encoding="utf-8")
            fake = directory / "gh"
            fake.write_text(
                "#!/bin/sh\ncase \"$*\" in\n"
                "  *'pulls?state=open'*) cat \"${PULLS_FILE}\" ;;\n"
                "  *'git/ref/heads/master'*) printf '%s' '{\"object\":{\"sha\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}}' ;;\n"
                "  *'repos/owner/repository'*) printf '%s' '{\"default_branch\":\"master\"}' ;;\n"
                "  *) exit 91 ;;\nesac\n", encoding="utf-8",
            ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            environment = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master",
                "EVENT_NAME": "workflow_run", "EVENT_TARGETS": "[]",
                "EVENT_PRIORITY_TARGETS": "[]", "GITHUB_OUTPUT": str(output),
                "PULLS_FILE": str(payload), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            result = subprocess.run(
                [sys.executable, "-c", self._workflow_program(match)], env=environment,
                capture_output=True, text=True, check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
            targets = json.loads(values["targets"])
            self.assertEqual(targets, governed)
            self.assertNotIn(1001, targets)
            self.assertEqual(len(json.loads(values["target_snapshots"])), len(governed))

    def test_nonpriority_workflow_run_oversized_snapshot_seeds_barrier_and_stops_before_dispatch(self) -> None:
        """A non-priority CI workflow_run still carries the bounded 600-head snapshot."""
        source_resolver = re.search(
            r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        current_resolver = re.search(
            r"- name: Re-enumerate every current local governance pull request.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        marker = re.search(
            r"- name: Publish periodic static affected-head barrier App marker.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        hold = re.search(
            r"- name: Hold global merge barrier for an oversized governed snapshot.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        for match in (source_resolver, current_resolver, marker, hold):
            self.assertIsNotNone(match)
        assert source_resolver is not None and current_resolver is not None and marker is not None and hold is not None

        marker_condition = "(github.event_name == 'schedule' || github.event_name == 'workflow_dispatch' || steps.current-targets.outputs.has_preinvalidate_targets == 'true' || steps.current-targets.outputs.invalidation_head_cap_exceeded == 'true') && steps.barrier-source.outcome == 'success'"
        self.assertEqual(self.workflow.count(marker_condition), 3)
        activation_match = re.search(
            r"^      - name: Activate complete affected-head merge barrier\n(?P<body>.*?)(?=^      - name: )",
            self.workflow,
            re.MULTILINE | re.DOTALL,
        )
        self.assertIsNotNone(activation_match); assert activation_match is not None
        self.assertIn(
            "PRIORITY: ${{ steps.current-targets.outputs.has_preinvalidate_targets == 'true' || steps.current-targets.outputs.invalidation_head_cap_exceeded == 'true' }}",
            activation_match.group("body"),
        )

        total = 600
        heads = {number: f"{number:040x}" for number in range(1, total + 1)}
        pages = [[
            {
                "number": number,
                "state": "open",
                "body": "Fixes #64",
                "draft": False,
                "base": {"ref": "master", "repo": {"full_name": "owner/repository"}},
                "head": {"sha": heads[number], "repo": {"full_name": "owner/repository"}},
            }
            for number in range(start, min(start + 100, total + 1))
        ] for start in range(1, total + 1, 100)]
        run = {
            "name": "CI",
            "path": ".github/workflows/test-and-build.yml@master",
            "event": "pull_request",
            "status": "completed",
            "id": 9,
            "run_number": 1,
            "run_attempt": 1,
            "head_sha": heads[1],
            "repository": {"id": 101, "full_name": "owner/repository"},
            "pull_requests": [{
                "number": 1,
                "base": {"sha": "b" * 40, "ref": "master", "repo": {"id": 101, "name": "repository", "url": "https://api.github.com/repos/owner/repository"}},
                "head": {"sha": heads[1], "repo": {"id": 101, "name": "repository", "url": "https://api.github.com/repos/owner/repository"}},
            }],
        }
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            pulls_file = directory / "pulls.json"
            pulls_json = json.dumps(pages, separators=(",", ":"))
            pulls_file.write_text(pulls_json, encoding="utf-8")
            source_output = directory / "source-output"
            current_output = directory / "current-output"
            marker_output = directory / "marker-output"
            fake = directory / "gh"
            fake.write_text(
                "#!/bin/sh\ncase \"$*\" in\n"
                "  *'/actions/runs/9'*) printf '%s' \"${RUN}\" ;;\n"
                "  *'/pulls/1'*) printf '%s' \"${PULL}\" ;;\n"
                "  *'/compare/'*) printf '%s' '{\"status\":\"identical\",\"base_commit\":{\"sha\":\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"},\"merge_base_commit\":{\"sha\":\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"},\"head_commit\":{\"sha\":\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"}}' ;;\n"
                "  *'/contents/'*) printf '%s' '{\"sha\":\"cccccccccccccccccccccccccccccccccccccccc\"}' ;;\n"
                "  *'pulls?state=open'*) cat \"${PULLS_FILE}\" ;;\n"
                "  *'git/ref/heads/master'*) printf '%s' '{\"object\":{\"sha\":\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"}}' ;;\n"
                "  *'repos/owner/repository'*) printf '%s' '{\"default_branch\":\"master\"}' ;;\n"
                "  *) exit 91 ;;\nesac\n",
                encoding="utf-8",
            )
            fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            base_environment = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository",
                "DEFAULT_BRANCH": "master",
                "EVENT_NAME": "workflow_run", "EVENT_ACTION": "completed",
                "WORKFLOW_RUN_ID": "9", "WORKFLOW_RUN_ATTEMPT": "1",
                "RUN": json.dumps(run, separators=(",", ":")),
                "PULL": json.dumps({"number": 1, "state": "open", "base": {"sha": "b" * 40, "ref": "master", "repo": {"id": 101, "full_name": "owner/repository"}}, "head": {"sha": heads[1], "repo": {"id": 101, "full_name": "owner/repository"}}}, separators=(",", ":")),
                "PULLS_FILE": str(pulls_file),
                "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            self.assertGreater(len(pulls_json.encode("utf-8")), 128 * 1024)
            self.assertNotIn("PULLS", base_environment)
            self.assertTrue(all(len(os.fsencode(key)) + len(os.fsencode(value)) < 128 * 1024 for key, value in base_environment.items()))
            source_environment = base_environment | {"GITHUB_OUTPUT": str(source_output)}
            result = subprocess.run(
                [sys.executable, "-c", self._workflow_program(source_resolver)],
                env=source_environment,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            source_values = dict(line.split("=", 1) for line in source_output.read_text(encoding="utf-8").splitlines())
            self.assertEqual(json.loads(source_values["event_targets"]), list(range(1, total + 1)))
            self.assertEqual(json.loads(source_values["priority_targets"]), [])
            current_environment = base_environment | {
                "GITHUB_OUTPUT": str(current_output),
                "EVENT_TARGETS": source_values["event_targets"],
                "EVENT_PRIORITY_TARGETS": source_values["priority_targets"],
            }
            result = subprocess.run(
                [sys.executable, "-c", self._workflow_program(current_resolver)],
                env=current_environment,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            values = dict(line.split("=", 1) for line in current_output.read_text(encoding="utf-8").splitlines())
            self.assertEqual(json.loads(values["targets"]), list(range(1, total + 1)))
            self.assertEqual(len(json.loads(values["target_snapshots"])), total)
            self.assertEqual(json.loads(values["priority_targets"]), [])
            self.assertEqual(json.loads(values["preinvalidate_targets"]), [])
            self.assertEqual(json.loads(values["all_invalidation_targets"]), list(range(1, total + 1)))
            self.assertEqual(len(json.loads(values["all_invalidation_target_snapshots"])), total)
            self.assertEqual(values["invalidation_head_cap_exceeded"], "false")
            self.assertEqual(json.loads(values["all_invalidation_chunk_1"]), list(range(1, 301)))
            self.assertEqual(json.loads(values["all_invalidation_chunk_2"]), list(range(301, 601)))

            marker_calls: list[list[str]] = []

            def marker_run(arguments: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                marker_calls.append(arguments)
                self.assertEqual(kwargs.get("env"), {"GH_TOKEN": "marker-write" if len(marker_calls) == 1 else "marker-read", "PATH": os.environ["PATH"]})
                if len(marker_calls) == 1:
                    return subprocess.CompletedProcess([], 0, json.dumps({
                        "id": 501,
                        "name": "KRR / PR governance affected-head barrier",
                        "head_sha": "a" * 40,
                        "external_id": "krr-governance-affected-head-barrier/v1/" + "a" * 40 + "/scheduler-9",
                        "status": "completed",
                        "conclusion": "success",
                        "details_url": "https://github.com/owner/repository/actions/runs/9?barrier_marker=periodic",
                        "app": {"id": 4_766_933},
                    }), "")
                return subprocess.CompletedProcess([], 0, json.dumps({
                    "id": 501,
                    "name": "KRR / PR governance affected-head barrier",
                    "head_sha": "a" * 40,
                    "external_id": "krr-governance-affected-head-barrier/v1/" + "a" * 40 + "/scheduler-9",
                    "status": "completed",
                    "conclusion": "success",
                    "details_url": "https://github.com/owner/repository/actions/runs/9?barrier_marker=periodic",
                    "app": {"id": 4_766_933},
                }), "")

            marker_environment = base_environment | {
                "GITHUB_OUTPUT": str(marker_output),
                "GITHUB_SERVER_URL": "https://github.com",
                "DEFAULT_HEAD": "a" * 40,
                "DISPATCHER_RUN_ID": "9",
                "CHECK_APP_ID": "4766933",
                "CHECK_WRITE_TOKEN": "marker-write",
                "CHECK_READ_TOKEN": "marker-read",
                "INVALIDATION_HEAD_CAP_EXCEEDED": values["invalidation_head_cap_exceeded"],
            }
            with patch.dict(os.environ, marker_environment, clear=True), patch("subprocess.run", side_effect=marker_run):
                exec(self._workflow_program(marker), {"__name__": "__main__"})
            self.assertEqual(len(marker_calls), 2)
            self.assertEqual(marker_environment["INVALIDATION_HEAD_CAP_EXCEEDED"], "false")

        hold_program = self._workflow_program(hold)
        for active, message in (("false", "before the affected-head barrier was active"), ("true", "head cap exceeded")):
            with self.subTest(active=active):
                with patch.dict(os.environ, {
                    "BARRIER_SOURCE_OUTCOME": "success",
                    "BARRIER_TOKEN_OUTCOME": "success",
                    "BARRIER_ACTIVATE_OUTCOME": "success",
                    "BARRIER_ACTIVE": active,
                }, clear=True):
                    with self.assertRaisesRegex(SystemExit, message):
                        exec(hold_program, {"__name__": "__main__"})

    def test_terminal_writer_segments_are_contiguous_bounded_and_fail_closed(self) -> None:
        """A 600-target manifest is partitioned into at most four ordered 150 slices."""
        writer = (ROOT / "scripts/review/pr_governance_status_writer.py").read_text(encoding="utf-8")
        workflow = self.workflow
        for required in (
            "GOVERNANCE_TERMINAL_BATCH_NUMBERS", "GOVERNANCE_CONTINUATION_INDEX",
            "len(terminal_batch) > 150", "continuation_index - 1) * 150",
            "Writer terminal segment boundary is invalid.",
        ):
            self.assertIn(required, writer)
        self.assertGreaterEqual(workflow.lower().count("terminal_batch_numbers"), 2)
        self.assertGreaterEqual(workflow.lower().count("continuation_index"), 2)
        dispatch_match = re.search(
            r"- name: [^\n]*repository-wide governance arbiter segment[^\n]*\n.*?python3 - <<'PY'\n(.*?)\n          PY",
            workflow, re.DOTALL | re.IGNORECASE,
        )
        self.assertIsNotNone(dispatch_match); assert dispatch_match is not None
        self.assertNotIn("KRR_GOVERNANCE_APP_PRIVATE_KEY", dispatch_match.group(1))
        self.assertNotIn("private-key", dispatch_match.group(1).lower())
        # Exercise the production ordering helper so the segment check does
        # not merely bless a hand-written numeric range in this test.
        spec = importlib.util.spec_from_file_location("krr_status_writer", ROOT / "scripts/review/pr_governance_status_writer.py")
        self.assertIsNotNone(spec); assert spec is not None and spec.loader is not None
        module = importlib.util.module_from_spec(spec)
        sys.modules[spec.name] = module
        spec.loader.exec_module(module)
        for total in (41, 600):
            pulls = tuple({"number": number, "isDraft": False} for number in range(1, total + 1))
            snapshot = module.OpenSnapshot(tuple(range(1, total + 1)), {}, pulls)
            order = module.governance_order(snapshot, frozenset(), tuple(range(1, min(total, 40) + 1)))
            early = order[:40]
            tail = order[40:]
            self.assertEqual(early, tuple(range(1, min(total, 40) + 1)))
            self.assertEqual(len(tail), max(0, total - 40))
            self.assertEqual(order, tuple(range(1, total + 1)))
            segments = [order[start:start + 150] for start in range(0, len(order), 150)]
            self.assertEqual([len(segment) for segment in segments], [150] * (total // 150) + ([total % 150] if total % 150 else []))
            self.assertEqual(tuple(number for segment in segments for number in segment), order)
            self.assertTrue(all(len(segment) <= 150 for segment in segments))
            self.assertLessEqual(len(segments), 4)
            if total == 41:
                self.assertEqual(tail, (41,))

    def test_all_writer_dispatches_sequential_terminal_segments_and_stops_after_failure(self) -> None:
        """Each terminal segment has its own token/dispatch/await fail-closed chain."""
        steps = list(re.finditer(
            r"^      - name: (?P<name>[^\n]+)\n(?P<body>.*?)(?=^      - name: |\Z)",
            self.workflow, re.MULTILINE | re.DOTALL,
        ))
        dispatch_steps = [step for step in steps if "terminal_batch_numbers" in step.group("body") and "Dispatch" in step.group("name") and "governance arbiter segment" in step.group("name")]
        await_steps = [step for step in steps if "Await" in step.group("name") and "governance arbiter segment" in step.group("name")]
        self.assertEqual(len(dispatch_steps), 4)
        self.assertEqual(len(await_steps), 4)
        dispatch_indices: list[int] = []
        for position, step in enumerate(dispatch_steps):
            body = step.group("body")
            step_number = steps.index(step)
            self.assertGreater(step_number, 0)
            # A conservative terminal-window marker may sit between the
            # dispatcher token and its POST.  The token must nevertheless be
            # created in this same fail-closed continuation chain.
            prior_token = next(
                (candidate for candidate in reversed(steps[:step_number])
                 if "actions/create-github-app-token" in candidate.group("body")),
                None,
            )
            self.assertIsNotNone(prior_token)
            index_match = re.search(r"inputs\[continuation_index\][^\n]*?=([1-4])(?:\"|')?", body)
            self.assertIsNotNone(index_match, step.group("name")); assert index_match is not None
            index = int(index_match.group(1)); dispatch_indices.append(index)
            self.assertEqual(index, position + 1)
            batch_match = re.search(r"inputs\[terminal_batch_numbers\].{0,180}", body)
            self.assertIsNotNone(batch_match)
            self.assertIn("inputs[terminal_order_numbers]", body)
            self.assertIn("inputs[completed_writer_run_ids]", body)
            self.assertNotIn("KRR_GOVERNANCE_APP_PRIVATE_KEY", body)
            # Every segment must be gated by the preceding await, so a failed
            # or NoPost segment cannot enqueue the next writer.
            if position:
                prior_id = re.search(r"\bid:\s*([A-Za-z0-9_-]+)", await_steps[position - 1].group("body"))
                self.assertIsNotNone(prior_id); assert prior_id is not None
                step_if = re.search(r"^\s*if:\s*(.+)$", body, re.MULTILINE)
                self.assertIsNotNone(step_if); assert step_if is not None
                self.assertIn(f"steps.{prior_id.group(1)}.outcome", step_if.group(1))
                self.assertNotIn("always()", step_if.group(1))
        self.assertEqual(dispatch_indices, [1, 2, 3, 4])
        for index, step in enumerate(await_steps, 1):
            body = step.group("body")
            self.assertIn(f"segment={index}", body)
            for required in ("run.get(\"head_sha\")", "run.get(\"status\")", "run.get(\"conclusion\")", "run.get(\"event\")"):
                self.assertIn(required, body)
            self.assertRegex(body, r"run\.get\(\"(?:actor|triggering_actor)\"\)")
            self.assertIn('run.get("status") == "completed"', body)
            self.assertIn('run.get("conclusion") != "success"', body)
            gate = re.search(r"^\s*if:\s*(.+)$", body, re.MULTILINE)
            self.assertIsNotNone(gate); assert gate is not None
            dispatch_id = re.search(r"\bid:\s*([A-Za-z0-9_-]+)", dispatch_steps[index - 1].group("body"))
            self.assertIsNotNone(dispatch_id); assert dispatch_id is not None
            self.assertIn(f"steps.{dispatch_id.group(1)}.outputs", gate.group(1))

        # Execute the dispatch and await blocks against a complete 600-target
        # fixture. This catches missing source/snapshot/carry API reads.
        all_targets = list(range(1, 601)); preserved = all_targets[:40]
        heads = {number: f"{number:040x}" for number in all_targets}
        rest_repository = {"id": 101, "name": "repository", "url": "https://api.github.com/repos/owner/repository", "full_name": "owner/repository"}
        snapshots = [[number, heads[number], False] for number in all_targets]
        pre_manifest = [[number, 50_000 + number] for number in all_targets]
        batches = [all_targets[40 + start:40 + start + 150] for start in range(0, 560, 150)]
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); output = directory / "output"
            candidates: list[dict[str, object]] = []; dispatches: list[list[str]] = []
            pages = [[
                {"number": number, "state": "open", "body": "Fixes #64", "draft": False,
                 "base": {"ref": "master", "repo": rest_repository},
                 "head": {"sha": heads[number], "repo": rest_repository}}
                for number in range(start, min(start + 100, 601))
            ] for start in range(1, 601, 100)]
            def response(value: object, code: int = 0) -> subprocess.CompletedProcess[str]:
                body = json.dumps(value)
                if current_arguments[0] is not None and "--include" in current_arguments[0]:
                    body = f"HTTP/2 200 OK\n\n{body}"
                return subprocess.CompletedProcess([], code, body, "")

            current_arguments: list[list[str] | None] = [None]
            def fake_run(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
                current_arguments[0] = arguments
                endpoint = next((item for item in arguments if isinstance(item, str) and item.startswith("repos/")), arguments[-1])
                if isinstance(endpoint, str) and "pulls?state=open" in endpoint:
                    page = int(parse_qs(urlparse(endpoint).query).get("page", ["1"])[0])
                    return response(pages[page - 1] if page <= len(pages) else [])
                if isinstance(endpoint, str) and endpoint.endswith("/git/ref/heads/master"):
                    return response({"object": {"sha": "a" * 40}})
                if isinstance(endpoint, str) and endpoint == "repos/owner/repository":
                    return response({**rest_repository, "full_name": "owner/repository", "default_branch": "master"})
                if isinstance(endpoint, str) and "/actions/runs/" in endpoint:
                    identifier = int(endpoint.rsplit("/", 1)[1])
                    if identifier == 9:
                        return response({"id": 9, "name": "PR governance dispatcher", "event": "issues", "status": "in_progress", "run_number": 1, "run_attempt": 1, "head_branch": "master", "head_sha": "a" * 40, "repository": rest_repository, "created_at": "2026-09-01T00:00:00Z"})
                    return response(next(candidate for candidate in candidates if candidate["id"] == identifier))
                if isinstance(endpoint, str) and "actions/workflows/pr-governance-status-writer.yml/runs?" in endpoint:
                    return response({"total_count": len(candidates), "workflow_runs": candidates})
                if "--method" in arguments and "POST" in arguments and any(isinstance(item, str) and "/dispatches" in item for item in arguments):
                    dispatches.append(arguments)
                    fields = {item.split("=", 1)[0]: item.split("=", 1)[1] for item in arguments if "=" in item}
                    index = fields.get("inputs[continuation_index]", "")
                    order = json.loads(fields["inputs[terminal_order_numbers]"])
                    completed = json.loads(fields["inputs[completed_writer_run_ids]"])
                    self.assertEqual(order, all_targets[40:])
                    self.assertEqual(completed, [70_000 + prior for prior in range(1, len(dispatches))])
                    candidate = {"id": 70_000 + len(dispatches), "name": "PR governance status writer", "display_title": f"source=9 scope=all segment={index}", "path": ".github/workflows/pr-governance-status-writer.yml@master", "event": "workflow_dispatch", "repository": rest_repository, "head_branch": "master", "head_sha": "a" * 40, "status": "completed", "conclusion": "success", "run_number": len(dispatches), "run_attempt": 1, "actor": {"login": "katana-rust-pr-governance-hf[bot]", "type": "Bot"}, "triggering_actor": {"login": "katana-rust-pr-governance-hf[bot]", "type": "Bot"}}
                    candidates.append(candidate)
                    return response({})
                raise AssertionError(arguments)

            common = {
                "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "WRITER_HEAD": "a" * 40,
                "DISPATCHER_RUN_ID": "9", "WRITER_SCOPE": "all", "WRITER_TARGETS": json.dumps(all_targets, separators=(",", ":")),
                "WRITER_ALL_OPEN_TARGETS": json.dumps(all_targets, separators=(",", ":")), "WRITER_ALL_OPEN_SNAPSHOTS": json.dumps(snapshots, separators=(",", ":")),
                "WRITER_PRESERVED_TARGETS": json.dumps(preserved, separators=(",", ":")), "PRESERVED_WRITER_RUN_ID": "71",
                "WRITER_CARRY_TARGET_NUMBERS": "[]", "WRITER_CARRY_TARGET_NUMBERS_1": "[]", "WRITER_CARRY_TARGET_NUMBERS_2": "[]", "WRITER_PREINVALIDATE_TARGETS": json.dumps(all_targets, separators=(",", ":")),
                "WRITER_PRE_CHECK_MANIFEST_1": json.dumps(pre_manifest[:300], separators=(",", ":")), "WRITER_PRE_CHECK_MANIFEST_2": json.dumps(pre_manifest[300:], separators=(",", ":")),
                "WRITER_TAIL_CHECK_MANIFEST_1": "[]", "WRITER_TAIL_CHECK_MANIFEST_2": "[]", "WRITER_PRESERVED_CHECK_MANIFEST": json.dumps(pre_manifest[:40], separators=(",", ":")),
                "WRITER_TERMINAL_ORDER": json.dumps(all_targets[40:], separators=(",", ":")), "COMPLETED_WRITER_RUN_IDS": "[]",
                "APP_BOT_LOGIN": "katana-rust-pr-governance-hf[bot]", "GITHUB_OUTPUT": str(output), "PATH": os.environ["PATH"],
            }
            for index, step in enumerate(dispatch_steps, 1):
                environment = os.environ | common | {
                    "WRITER_TERMINAL_BATCH_NUMBERS": json.dumps(batches[index - 1], separators=(",", ":")),
                    "WRITER_CONTINUATION_INDEX": str(index), "TERMINAL_BATCH": json.dumps(batches[index - 1], separators=(",", ":")),
                    "CONTINUATION_INDEX": str(index), "WRITER_CHECK_MANIFEST": json.dumps(pre_manifest, separators=(",", ":")),
                    "COMPLETED_WRITER_RUN_IDS": json.dumps([70_000 + prior for prior in range(1, index)], separators=(",", ":")),
                }
                with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run):
                    try:
                        exec(self._workflow_program(re.search(r"python3 - <<'PY'\n(.*?)\n          PY", step.group("body"), re.DOTALL)), {"__name__": "__main__"})  # type: ignore[arg-type]
                    except SystemExit as error:
                        self.assertEqual(error.code, 0)
                await_body = await_steps[index - 1].group("body")
                await_program_match = re.search(r"python3 - <<'PY'\n(.*?)\n          PY", await_body, re.DOTALL)
                self.assertIsNotNone(await_program_match); assert await_program_match is not None
                writer_id = str(70_000 + index)
                await_environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "WRITER_HEAD": "a" * 40,
                    "WRITER_RUN_ID": writer_id, "DISPATCHER_RUN_ID": "9", "CONTINUATION_INDEX": str(index),
                    "GH_TOKEN": "read", "GITHUB_SERVER_URL": "https://github.com", "APP_BOT_LOGIN": "katana-rust-pr-governance-hf[bot]", "GITHUB_OUTPUT": str(output),
                    "PATH": os.environ["PATH"],
                }
                with patch.dict(os.environ, await_environment, clear=True), patch("subprocess.run", side_effect=fake_run), patch("time.sleep"):
                    try:
                        exec(self._workflow_program(await_program_match), {"__name__": "__main__"})
                    except SystemExit as error:
                        self.assertEqual(error.code, 0)
            self.assertEqual(len(dispatches), 4)
            sent_batches = [json.loads(next(item.split("=", 1)[1] for item in dispatch if item.startswith("inputs[terminal_batch_numbers]="))) for dispatch in dispatches]
            self.assertEqual([number for batch in sent_batches for number in batch], all_targets[40:])
            self.assertTrue(all(len(batch) <= 150 for batch in sent_batches))
            next_segment_if = self._step_if("Dispatch second repository-wide governance arbiter segment")
            success_values = {
                "steps.dispatch-all-1.outputs.has_terminal_batch_2": "true",
                "steps.await-all-1.outcome": "success",
                "steps.await-all-1.outputs.success": "true",
            }
            self.assertTrue(self._github_if(next_segment_if, success_values))
            self.assertEqual(sum(
                next(item.split("=", 1)[1] for item in dispatch if item.startswith("inputs[continuation_index]=")) == "2"
                for dispatch in dispatches
            ), 1)

        # Exercise the hand-off rather than only its individual YAML snippets:
        # the second TTL chunk may be the sole carry source.  The dispatcher
        # has to retain it in the first terminal dispatch, and the actual
        # writer main has to accept precisely that carry-first canonical order.
        first_program_match = re.search(r"python3 - <<'PY'\n(.*?)\n          PY", dispatch_steps[0].group("body"), re.DOTALL)
        self.assertIsNotNone(first_program_match); assert first_program_match is not None
        carry_heads = {1: f"{1:040x}", 301: f"{301:040x}"}
        carry_pulls = [{
            "number": number, "state": "open", "body": "Fixes #64", "draft": False,
            "base": {"sha": "a" * 40, "ref": "master", "repo": rest_repository},
            "head": {"sha": head, "ref": f"issue/{number}", "repo": rest_repository},
        } for number, head in carry_heads.items()]
        carry_snapshots = [[number, head, False] for number, head in carry_heads.items()]
        carry_manifest = [[1, 91], [301, 92]]
        carried_dispatches: list[list[str]] = []
        registered: list[dict[str, object]] = []

        def response(value: object, code: int = 0) -> subprocess.CompletedProcess[str]:
            return subprocess.CompletedProcess([], code, json.dumps(value), "")

        dispatcher_source_active = {
            "id": 9, "workflow_id": 8, "name": "PR governance dispatcher",
            "path": ".github/workflows/pr-governance.yml@master", "event": "issues",
            "head_branch": "master", "head_sha": "a" * 40,
            "repository": rest_repository, "run_number": 1,
            "run_attempt": 1, "status": "in_progress", "conclusion": None,
            "created_at": "2026-08-30T00:00:00Z",
        }

        def dispatch_transport(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
            endpoint = next((item for item in arguments if isinstance(item, str) and item.startswith("repos/")), arguments[-1])
            if isinstance(endpoint, str) and endpoint == "repos/owner/repository":
                return response({**rest_repository, "full_name": "owner/repository", "default_branch": "master"})
            if isinstance(endpoint, str) and endpoint.endswith("/git/ref/heads/master"):
                return response({"object": {"sha": "a" * 40}})
            if isinstance(endpoint, str) and endpoint.endswith("/actions/runs/9"):
                return response(dispatcher_source_active)
            if isinstance(endpoint, str) and "pulls?state=open" in endpoint:
                return response(carry_pulls)
            if isinstance(endpoint, str) and "pr-governance-status-writer.yml/runs?" in endpoint:
                return response({"total_count": len(registered), "workflow_runs": registered})
            if "--method" in arguments and "POST" in arguments and any(isinstance(item, str) and "/dispatches" in item for item in arguments):
                carried_dispatches.append(arguments)
                registered.append({
                    "id": 77, "name": "PR governance status writer",
                    "display_title": "source=9 scope=all segment=1",
                    "path": ".github/workflows/pr-governance-status-writer.yml@master",
                    "event": "workflow_dispatch", "repository": rest_repository,
                    "head_branch": "master", "head_sha": "a" * 40, "status": "queued",
                    "run_number": 1, "run_attempt": 1,
                })
                return response({})
            raise AssertionError(arguments)

        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); handoff_output = directory / "handoff-output"
            handoff_env = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com",
                "DEFAULT_BRANCH": "master", "WRITER_HEAD": "a" * 40, "DISPATCHER_RUN_ID": "9",
                "WRITER_SCOPE": "all", "WRITER_TARGETS": "[]",
                "WRITER_ALL_OPEN_TARGETS": "[1,301]",
                "WRITER_ALL_OPEN_SNAPSHOTS": json.dumps(carry_snapshots, separators=(",", ":")),
                "WRITER_PRESERVED_TARGETS": "[]", "PRESERVED_WRITER_RUN_ID": "0",
                "WRITER_PREINVALIDATE_TARGETS": "[]", "WRITER_PRE_CHECK_MANIFEST_1": "[]",
                "WRITER_PRE_CHECK_MANIFEST_2": "[]",
                "WRITER_TAIL_CHECK_MANIFEST_1": json.dumps(carry_manifest, separators=(",", ":")),
                "WRITER_TAIL_CHECK_MANIFEST_2": "[]", "WRITER_PRESERVED_CHECK_MANIFEST": "[]",
                "WRITER_CARRY_TARGET_NUMBERS_1": "[]", "WRITER_CARRY_TARGET_NUMBERS_2": "[301]",
                "GITHUB_OUTPUT": str(handoff_output), "PATH": os.environ["PATH"],
            }
            with patch.dict(os.environ, handoff_env, clear=True), patch("subprocess.run", side_effect=dispatch_transport):
                exec(self._workflow_program(first_program_match), {"__name__": "__main__"})
            self.assertEqual(len(carried_dispatches), 1)
            dispatched_fields = {
                item.split("=", 1)[0]: item.split("=", 1)[1]
                for item in carried_dispatches[0] if isinstance(item, str) and "=" in item
            }
            self.assertEqual(json.loads(dispatched_fields["inputs[terminal_order_numbers]"]), [301, 1])
            self.assertEqual(json.loads(dispatched_fields["inputs[terminal_batch_numbers]"]), [301, 1])
            self.assertEqual(json.loads(dispatched_fields["inputs[completed_writer_run_ids]"]), [])

            # This is intentionally the production writer main, with only
            # GitHub transport and delay mocked.  A contract failure gives a
            # bounded terminal failure path without replacing internal helpers.
            check_runs: dict[int, dict[str, object]] = {}
            for number, identifier in carry_manifest:
                head = carry_heads[number]
                carry = number == 301
                check_runs[identifier] = {
                    "id": identifier, "app": {"id": 42},
                    "name": "KRR / PR governance (trusted check)", "head_sha": head,
                    "external_id": f"krr-governance/v1/{head}/dispatcher-9",
                    "status": "in_progress", "conclusion": None,
                    "details_url": f"https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending={int(carry)}",
                    "updated_at": "2026-08-30T00:00:00Z",
                }
            terminal_writes: list[int] = []

            def writer_transport(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
                if arguments and arguments[0] == sys.executable:
                    # verify_push_issue is an external contract command. Its
                    # failure exercises the writer's ordinary fail-closed path.
                    return subprocess.CompletedProcess(arguments, 1, "", "")
                endpoint = next((item for item in arguments if isinstance(item, str) and item.startswith("repos/")), arguments[-1])
                if isinstance(endpoint, str) and "pulls?state=open" in endpoint:
                    return response(carry_pulls)
                if isinstance(endpoint, str) and "/pulls/" in endpoint:
                    number = int(endpoint.rsplit("/", 1)[1])
                    return response(next(pull for pull in carry_pulls if pull["number"] == number))
                if isinstance(endpoint, str) and endpoint == "repos/owner/repository":
                    return response({"default_branch": "master"})
                if isinstance(endpoint, str) and endpoint.endswith("/git/ref/heads/master"):
                    return response({"object": {"sha": "a" * 40}})
                if isinstance(endpoint, str) and endpoint.endswith("/actions/runs/9"):
                    return response({**dispatcher_source_active, "status": "completed", "conclusion": "success"})
                if isinstance(endpoint, str) and endpoint.endswith("/actions/runs/77"):
                    return response({
                        "id": 77, "name": "PR governance status writer",
                        "path": ".github/workflows/pr-governance-status-writer.yml@master",
                        "event": "workflow_dispatch", "head_sha": "a" * 40,
                        "repository": rest_repository, "status": "in_progress", "run_attempt": 1,
                    })
                if isinstance(endpoint, str) and "/actions/workflows/8/runs?" in endpoint:
                    return response({"workflow_runs": [{**dispatcher_source_active, "status": "completed", "conclusion": "success"}], "total_count": 1})
                if "--method" in arguments and "PATCH" in arguments and isinstance(endpoint, str) and "/check-runs/" in endpoint:
                    identifier = int(endpoint.rsplit("/", 1)[1]); value = dict(check_runs[identifier])
                    fields = {
                        item.split("=", 1)[0]: item.split("=", 1)[1]
                        for item in arguments if isinstance(item, str) and "=" in item
                    }
                    value.update({"status": "completed", "conclusion": "failure", "details_url": fields["details_url"], "updated_at": f"2026-08-30T00:00:0{len(terminal_writes) + 1}Z"})
                    check_runs[identifier] = value; terminal_writes.append(identifier)
                    return response(value)
                if isinstance(endpoint, str) and endpoint.startswith("repos/owner/repository/check-runs/"):
                    return response(check_runs[int(endpoint.rsplit("/", 1)[1])])
                if isinstance(endpoint, str) and "pr-governance-review-events.yml/runs?" in endpoint:
                    query = parse_qs(urlparse(endpoint).query)
                    self.assertEqual(set(query), {"head_sha", "per_page"})
                    self.assertIn(query["head_sha"][0], set(carry_heads.values()))
                    self.assertEqual(query["per_page"], ["100"])
                    return response({"total_count": 0, "workflow_runs": []})
                if isinstance(endpoint, str) and endpoint.endswith("test-and-build.yml"):
                    return response({"id": 31})
                if isinstance(endpoint, str) and endpoint.endswith("release-preflight.yml"):
                    return response({"id": 32})
                if isinstance(endpoint, str) and "/actions/workflows/" in endpoint and "/runs?" in endpoint:
                    query = parse_qs(urlparse(endpoint).query)
                    self.assertEqual(set(query), {"event", "head_sha", "per_page"})
                    self.assertEqual(query["event"], ["pull_request"])
                    self.assertIn(query["head_sha"][0], set(carry_heads.values()))
                    self.assertEqual(query["per_page"], ["100"])
                    return response({"total_count": 0, "workflow_runs": []})
                raise AssertionError(arguments)

            writer_env = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com",
                "GITHUB_RUN_ID": "77", "GITHUB_REF_NAME": "master", "GITHUB_SHA": "a" * 40,
                "GITHUB_ACTIONS": "true", "GH_TOKEN": "read", "DEFAULT_READ_TOKEN": "default-read",
                "CHECK_WRITE_TOKEN": "write", "KRR_GOVERNANCE_CHECK_APP_ID": "42",
                "KRR_GOVERNANCE_APP_BOT_LOGIN": "katana-rust-pr-governance-hf[bot]",
                "GOVERNANCE_DISPATCHER_RUN_ID": dispatched_fields["inputs[dispatcher_run_id]"],
                "GOVERNANCE_SCOPE": dispatched_fields["inputs[scope]"],
                "GOVERNANCE_TARGET_NUMBERS": dispatched_fields["inputs[target_numbers]"],
                "GOVERNANCE_PRESERVED_TARGET_NUMBERS": dispatched_fields["inputs[preserved_target_numbers]"],
                "GOVERNANCE_PRESERVED_WRITER_RUN_ID": dispatched_fields["inputs[preserved_writer_run_id]"],
                "GOVERNANCE_CHECK_MANIFEST": dispatched_fields["inputs[check_manifest]"],
                "GOVERNANCE_TERMINAL_BATCH_NUMBERS": dispatched_fields["inputs[terminal_batch_numbers]"],
                "GOVERNANCE_CONTINUATION_INDEX": dispatched_fields["inputs[continuation_index]"],
                "GOVERNANCE_TERMINAL_ORDER_NUMBERS": dispatched_fields["inputs[terminal_order_numbers]"],
                "GOVERNANCE_COMPLETED_WRITER_RUN_IDS": dispatched_fields["inputs[completed_writer_run_ids]"],
                "PATH": os.environ["PATH"],
            }
            module_name = "krr_status_writer_event_handoff"
            with patch.dict(os.environ, writer_env, clear=True), patch("subprocess.run", side_effect=writer_transport), patch("time.sleep"):
                writer_spec = importlib.util.spec_from_file_location(module_name, ROOT / "scripts/review/pr_governance_status_writer.py")
                self.assertIsNotNone(writer_spec); assert writer_spec is not None and writer_spec.loader is not None
                writer_module = importlib.util.module_from_spec(writer_spec); sys.modules[module_name] = writer_module
                try:
                    writer_spec.loader.exec_module(writer_module)
                    self.assertEqual(writer_module.main(), 0)
                finally:
                    sys.modules.pop(module_name, None)
            self.assertEqual(terminal_writes, [92, 91])

            # A non-success first writer, an old writer seeing a newer
            # dispatcher generation, and a default-branch drift are all
            # terminal stop conditions.  The workflow's next segment is only
            # entered after a zero exit, so its dispatch endpoint remains
            # untouched for each failed run.
            for mode in ("failure", "newer-generation", "default-branch-drift"):
                with self.subTest(mode=mode):
                    next_segment_dispatches: list[list[str]] = []
                    if mode == "failure":
                        # The actual first await program rejects a failed
                        # registered writer before the segment-2 step is eligible.
                        failed = dict(registered[0], status="completed", conclusion="failure")
                        await_program = re.search(r"python3 - <<'PY'\n(.*?)\n          PY", await_steps[0].group("body"), re.DOTALL)
                        self.assertIsNotNone(await_program); assert await_program is not None
                        def failed_await_transport(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
                            endpoint = arguments[-1]
                            if isinstance(endpoint, str) and endpoint.endswith("/actions/runs/77"):
                                return response(failed)
                            raise AssertionError(arguments)
                        await_env = os.environ | {
                            "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "WRITER_HEAD": "a" * 40,
                            "WRITER_RUN_ID": "77", "DISPATCHER_RUN_ID": "9", "CONTINUATION_INDEX": "1", "GH_TOKEN": "read",
                            "GITHUB_SERVER_URL": "https://github.com", "APP_BOT_LOGIN": "katana-rust-pr-governance-hf[bot]",
                            "GITHUB_OUTPUT": str(directory / "failure-await"), "PATH": os.environ["PATH"],
                        }
                        with patch.dict(os.environ, await_env, clear=True), patch("subprocess.run", side_effect=failed_await_transport), patch("time.sleep"):
                            with self.assertRaises(SystemExit):
                                exec(self._workflow_program(await_program), {"__name__": "__main__"})
                    else:
                        # The production writer itself owns the terminal
                        # barrier; a failed main never makes a continuation
                        # dispatch eligible.
                        def stopping_transport(arguments: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                            if mode == "default-branch-drift":
                                endpoint = next((item for item in arguments if isinstance(item, str) and item.startswith("repos/")), arguments[-1])
                                if isinstance(endpoint, str) and endpoint.endswith("/git/ref/heads/master"):
                                    return response({"object": {"sha": "b" * 40}})
                            if mode == "newer-generation":
                                endpoint = next((item for item in arguments if isinstance(item, str) and item.startswith("repos/")), arguments[-1])
                                if isinstance(endpoint, str) and "/actions/workflows/8/runs?" in endpoint:
                                    newer = {**dispatcher_source_active, "id": 10, "created_at": "2026-08-30T00:00:01Z", "status": "queued", "conclusion": None}
                                    return response({"workflow_runs": [{**dispatcher_source_active, "status": "completed", "conclusion": "success"}, newer], "total_count": 2})
                            return writer_transport(arguments, **kwargs)
                        # Reset the mutable Check Run fixture so each mode has
                        # a valid dispatcher-pending baseline.
                        for number, identifier in carry_manifest:
                            head = carry_heads[number]
                            check_runs[identifier].update({"status": "in_progress", "conclusion": None, "details_url": f"https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending={int(number == 301)}", "updated_at": "2026-08-30T00:00:00Z"})
                        with patch.dict(os.environ, writer_env, clear=True), patch("subprocess.run", side_effect=stopping_transport), patch("time.sleep"):
                            writer_spec = importlib.util.spec_from_file_location(f"{module_name}_{mode}", ROOT / "scripts/review/pr_governance_status_writer.py")
                            self.assertIsNotNone(writer_spec); assert writer_spec is not None and writer_spec.loader is not None
                            writer_module = importlib.util.module_from_spec(writer_spec); sys.modules[writer_spec.name] = writer_module
                            try:
                                writer_spec.loader.exec_module(writer_module)
                                self.assertEqual(writer_module.main(), 1)
                            finally:
                                sys.modules.pop(writer_spec.name, None)
                    failed_values = {
                        "steps.dispatch-all-1.outputs.has_terminal_batch_2": "true",
                        "steps.await-all-1.outcome": "failure",
                        "steps.await-all-1.outputs.success": "false",
                    }
                    # The values are the actual failed preceding result; use
                    # the workflow expression itself, never a local proxy
                    # condition, to decide whether the next dispatch runs.
                    self.assertFalse(self._github_if(next_segment_if, failed_values))
                    if self._github_if(next_segment_if, failed_values):
                        next_program = re.search(r"python3 - <<'PY'\n(.*?)\n          PY", dispatch_steps[1].group("body"), re.DOTALL)
                        self.assertIsNotNone(next_program); assert next_program is not None
                        exec(self._workflow_program(next_program), {"__name__": "__main__"})  # pragma: no cover
                    self.assertEqual(next_segment_dispatches, [])


if __name__ == "__main__":
    unittest.main()
