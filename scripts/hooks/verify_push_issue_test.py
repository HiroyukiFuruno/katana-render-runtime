from __future__ import annotations

import ast
import io
import os
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parent))

import verify_push_issue as subject


class VerifyPushIssueTest(unittest.TestCase):
    @staticmethod
    def configure_live_fetch_remote(
        repository: Path,
        *,
        remote: str,
        push_url: str,
    ) -> None:
        """Give CLI tests a local fetch endpoint and a GitHub-shaped push endpoint."""
        remote_repository = repository.parent / f"{repository.name}-{remote}.git"
        subprocess.run(
            ["git", "init", "--bare", "--initial-branch=master", remote_repository],
            check=True,
            capture_output=True,
            text=True,
        )
        subprocess.run(
            ["git", "remote", "add", remote, str(remote_repository)],
            cwd=repository,
            check=True,
            capture_output=True,
            text=True,
        )
        subprocess.run(
            ["git", "push", remote, "HEAD:refs/heads/master"],
            cwd=repository,
            check=True,
            capture_output=True,
            text=True,
        )
        subprocess.run(
            ["git", "remote", "set-url", "--push", remote, push_url],
            cwd=repository,
            check=True,
            capture_output=True,
            text=True,
        )

    @staticmethod
    def install_fake_push_remote_git(binary_directory: Path) -> None:
        """Emulate direct GitHub transport while preserving the configured URL."""
        fake_git = binary_directory / "git"
        fake_git.write_text(
            "#!/bin/sh\n"
            "set -eu\n"
            'real_git="${KRR_TEST_REAL_GIT:?}"\n'
            'remote_ref="${KRR_TEST_PUSH_REMOTE_REF:?}"\n'
            'source_url=""\n'
            'last_argument=""\n'
            'for argument in "$@"; do\n'
            '  case "$argument" in https://github.com/*) source_url="$argument" ;; esac\n'
            '  last_argument="$argument"\n'
            "done\n"
            'if [ "$1" = "ls-remote" ] && [ "$2" = "--symref" ] && [ -n "$source_url" ]; then\n'
            '  sha="$("$real_git" rev-parse "$remote_ref")"\n'
            '  printf "ref: refs/heads/master\\tHEAD\\n%s\\tHEAD\\n" "$sha"\n'
            "  exit 0\n"
            "fi\n"
            'if [ "$1" = "fetch" ] && [ -n "$source_url" ]; then\n'
            '  destination="${last_argument#*:}"\n'
            '  sha="$("$real_git" rev-parse "$remote_ref")"\n'
            '  exec "$real_git" update-ref "$destination" "$sha"\n'
            "fi\n"
            'exec "$real_git" "$@"\n',
            encoding="utf-8",
        )
        fake_git.chmod(fake_git.stat().st_mode | stat.S_IXUSR)

    def test_read_push_input_does_not_wait_on_an_interactive_terminal(self) -> None:
        class InteractiveInput(io.StringIO):
            def isatty(self) -> bool:
                return True

            def read(self, *args: object, **kwargs: object) -> str:
                raise AssertionError("interactive stdin must not be read")

        self.assertEqual(subject._read_push_input(InteractiveInput()), "")
        self.assertEqual(
            subject._read_push_input(io.StringIO("push input\n")),
            "push input\n",
        )

    def test_parse_push_updates_rejects_nonempty_malformed_lines(self) -> None:
        with self.assertRaises(subject.ContractViolation):
            subject.parse_push_updates("refs/heads/topic deadbeef\n")

    def test_parse_push_updates_accepts_whitespace_only_input_as_empty(self) -> None:
        self.assertEqual(subject.parse_push_updates(" \n\t\n"), ())

    def test_parse_push_updates_rejects_non_forty_hex_sha(self) -> None:
        with self.assertRaises(subject.ContractViolation):
            subject.parse_push_updates(
                f"refs/heads/topic {'0123456789abcdef0123456789abcdef0123456g'} "
                f"refs/heads/topic {'0' * 40}\n"
            )

    def test_parse_push_updates_rejects_invalid_local_or_remote_refs(self) -> None:
        for local_ref, remote_ref in (
            ("topic", "refs/heads/topic"),
            ("refs/heads/topic", "refs/heads/"),
            ("refs/heads/topic space", "refs/heads/topic"),
        ):
            with self.subTest(local_ref=local_ref, remote_ref=remote_ref):
                with self.assertRaises(subject.ContractViolation):
                    subject.parse_push_updates(
                        f"{local_ref} {'1' * 40} {remote_ref} {'2' * 40}\n"
                    )

    def test_parse_push_updates_accepts_delete_marker_and_skips_deleted_branch(self) -> None:
        updates = subject.parse_push_updates(
            f"(delete) {'0' * 40} refs/heads/obsolete {'1' * 40}\n"
        )
        self.assertEqual(
            subject.pushed_branch_updates(updates, default_branch="master"),
            (),
        )

    def test_parse_push_updates_accepts_revision_expression_as_local_ref(self) -> None:
        local_sha = "1" * 40
        remote_sha = "0" * 40
        self.assertEqual(
            subject.parse_push_updates(
                f"HEAD~ {local_sha} refs/heads/topic {remote_sha}\n"
            ),
            (("HEAD~", local_sha, "refs/heads/topic", remote_sha),),
        )

    def test_parse_push_updates_accepts_object_id_and_revspec_local_refs(self) -> None:
        local_sha = "1" * 40
        remote_sha = "0" * 40
        updates = subject.parse_push_updates(
            "\n".join(
                (
                    f"{'a' * 40} {local_sha} refs/heads/topic {remote_sha}",
                    f"feature~2 {local_sha} refs/heads/other {remote_sha}",
                )
            )
            + "\n"
        )
        self.assertEqual(
            updates,
            (
                ("a" * 40, local_sha, "refs/heads/topic", remote_sha),
                ("feature~2", local_sha, "refs/heads/other", remote_sha),
            ),
        )

    def test_remote_default_head_parser_accepts_live_main_response(self) -> None:
        sha = "a" * 40
        self.assertEqual(
            subject._parse_remote_default_head(
                f"ref: refs/heads/main\tHEAD\n{sha}\tHEAD\n"
            ),
            ("main", sha),
        )

    def test_remote_default_head_parser_rejects_malformed_response(self) -> None:
        malformed_responses = (
            "",
            f"{('a' * 40)}\tHEAD\n",
            f"ref: refs/tags/v1\tHEAD\n{('a' * 40)}\tHEAD\n",
            f"ref: refs/heads/main\tHEAD\nnot-a-sha\tHEAD\n",
            f"ref: refs/heads/main\tHEAD\n{('a' * 40)}\tHEAD\nextra\n",
        )
        for raw in malformed_responses:
            with self.subTest(raw=raw):
                with self.assertRaises(subject.ContractViolation):
                    subject._parse_remote_default_head(raw)

    def test_default_ref_binding_fetches_stale_tracking_ref_from_live_remote(self) -> None:
        remote_sha = "a" * 40
        live_response = f"ref: refs/heads/main\tHEAD\n{remote_sha}\tHEAD"
        pushed_remote_url = "https://github.com/HiroyukiFuruno/katana-render-runtime.git"
        remote_calls: list[tuple[str, ...]] = []

        def run_git(_repository: Path, *arguments: str) -> str:
            if arguments == ("rev-parse", "refs/remotes/origin/main"):
                return remote_sha
            raise AssertionError(f"unexpected git invocation: {arguments}")

        def run_remote_git(_repository: Path, _failure: str, *arguments: str) -> str:
            remote_calls.append(arguments)
            if arguments == ("ls-remote", "--symref", pushed_remote_url, "HEAD"):
                return live_response
            if arguments == (
                "fetch",
                "--no-tags",
                "--no-write-fetch-head",
                pushed_remote_url,
                "+refs/heads/main:refs/remotes/origin/main",
            ):
                return ""
            raise AssertionError(f"unexpected remote git invocation: {arguments}")

        with (
            patch.object(subject, "_run_git", side_effect=run_git),
            patch.object(subject, "_run_remote_git", side_effect=run_remote_git),
        ):
            self.assertEqual(
                subject._bind_remote_default_ref(
                    Path("/tmp/repository"),
                    remote="origin",
                    pushed_remote_url=pushed_remote_url,
                    repository_name="HiroyukiFuruno/katana-render-runtime",
                ),
                ("main", "origin/main"),
            )
        self.assertEqual(
            remote_calls.count(("ls-remote", "--symref", pushed_remote_url, "HEAD")),
            2,
        )
        self.assertIn(
            (
                "fetch",
                "--no-tags",
                "--no-write-fetch-head",
                pushed_remote_url,
                "+refs/heads/main:refs/remotes/origin/main",
            ),
            remote_calls,
        )

    def test_default_ref_binding_rejects_remote_advance_during_fetch(self) -> None:
        old_sha = "a" * 40
        current_sha = "b" * 40
        pushed_remote_url = "https://github.com/HiroyukiFuruno/katana-render-runtime.git"
        responses = iter(
            (
                f"ref: refs/heads/master\tHEAD\n{old_sha}\tHEAD",
                f"ref: refs/heads/master\tHEAD\n{current_sha}\tHEAD",
            )
        )

        def run_git(_repository: Path, *arguments: str) -> str:
            if arguments == ("rev-parse", "refs/remotes/origin/master"):
                return current_sha
            raise AssertionError(f"unexpected git invocation: {arguments}")

        def run_remote_git(_repository: Path, _failure: str, *arguments: str) -> str:
            if arguments == ("ls-remote", "--symref", pushed_remote_url, "HEAD"):
                return next(responses)
            if arguments == (
                "fetch",
                "--no-tags",
                "--no-write-fetch-head",
                pushed_remote_url,
                "+refs/heads/master:refs/remotes/origin/master",
            ):
                return ""
            raise AssertionError(f"unexpected remote git invocation: {arguments}")

        with (
            patch.object(subject, "_run_git", side_effect=run_git),
            patch.object(subject, "_run_remote_git", side_effect=run_remote_git),
        ):
            with self.assertRaisesRegex(subject.ContractViolation, "検証中に更新"):
                subject._bind_remote_default_ref(
                    Path("/tmp/repository"),
                    remote="origin",
                    pushed_remote_url=pushed_remote_url,
                    repository_name="HiroyukiFuruno/katana-render-runtime",
                )

    def test_default_ref_binding_rejects_fetch_error(self) -> None:
        sha = "a" * 40
        pushed_remote_url = "https://github.com/HiroyukiFuruno/katana-render-runtime.git"

        def run_remote_git(_repository: Path, _failure: str, *arguments: str) -> str:
            if arguments == ("ls-remote", "--symref", pushed_remote_url, "HEAD"):
                return f"ref: refs/heads/master\tHEAD\n{sha}\tHEAD"
            if arguments[0] == "fetch":
                raise subject.ContractViolation("push remoteのdefault branch fetchに失敗しました")
            raise AssertionError(f"unexpected remote git invocation: {arguments}")

        with patch.object(subject, "_run_remote_git", side_effect=run_remote_git):
            with self.assertRaisesRegex(subject.ContractViolation, "fetchに失敗"):
                subject._bind_remote_default_ref(
                    Path("/tmp/repository"),
                    remote="origin",
                    pushed_remote_url=pushed_remote_url,
                    repository_name="HiroyukiFuruno/katana-render-runtime",
                )

    def test_default_ref_binding_rejects_push_repository_mismatch_before_network(self) -> None:
        with patch.object(subject, "_run_remote_git") as run_remote_git:
            with self.assertRaisesRegex(subject.ContractViolation, "一致しません"):
                subject._bind_remote_default_ref(
                    Path("/tmp/repository"),
                    remote="origin",
                    pushed_remote_url=(
                        "https://github.com/HiroyukiFuruno/other-repository.git"
                    ),
                    repository_name="HiroyukiFuruno/katana-render-runtime",
                )
        run_remote_git.assert_not_called()

    def test_live_remote_failures_do_not_expose_stdout_stderr_or_credential_url(self) -> None:
        credential_url = "https://token:top-secret@example.invalid/repository.git"
        completed = subprocess.CompletedProcess(
            args=["git"],
            returncode=128,
            stdout=f"stdout includes {credential_url}",
            stderr=f"stderr includes {credential_url}",
        )
        with patch.object(subject.subprocess, "run", return_value=completed):
            with self.assertRaises(subject.ContractViolation) as captured:
                subject._live_remote_default_head(Path("/tmp/repository"), credential_url)
        message = str(captured.exception)
        self.assertEqual(message, "push remoteのdefault branch取得に失敗しました")
        self.assertNotIn("top-secret", message)
        self.assertNotIn("example.invalid", message)

        with patch.object(subject.subprocess, "run", return_value=completed):
            with self.assertRaises(subject.ContractViolation) as captured:
                subject._run_remote_git(
                    Path("/tmp/repository"),
                    "push remoteのdefault branch fetchに失敗しました",
                    "fetch",
                    credential_url,
                    "+refs/heads/master:refs/remotes/origin/master",
                )
        message = str(captured.exception)
        self.assertEqual(message, "push remoteのdefault branch fetchに失敗しました")
        self.assertNotIn("top-secret", message)
        self.assertNotIn("example.invalid", message)

    def test_name_status_paths_keeps_normal_rename_and_copy_paths(self) -> None:
        paths = subject.parse_name_status_paths(
            "M\0src/main.rs\0"
            "R100\0Cargo.lock\0renamed.txt\0"
            "C075\0Cargo.toml\0fixtures/Cargo.toml\0"
        )
        self.assertEqual(
            paths,
            [
                "src/main.rs",
                "Cargo.lock",
                "renamed.txt",
                "Cargo.toml",
                "fixtures/Cargo.toml",
            ],
        )

    def test_name_status_paths_rejects_malformed_or_truncated_records(self) -> None:
        for raw in (
            "M\0src/main.rs",
            "R100\0Cargo.lock\0",
            "C\0Cargo.lock\0renamed.txt\0",
            "R101\0Cargo.lock\0renamed.txt\0",
            "M100\0Cargo.lock\0",
            "Z\0unknown\0",
            "A\0\0",
        ):
            with self.subTest(raw=raw):
                with self.assertRaises(subject.ContractViolation):
                    subject.parse_name_status_paths(raw)

    def test_commit_message_parser_preserves_empty_records_and_boundaries(self) -> None:
        messages = subject.parse_commit_messages(
            "feat: first\n\nRefs #64\n\0\0fix: third\n\nRefs #64\n\0"
        )
        self.assertEqual(
            messages,
            ["feat: first\n\nRefs #64\n", "", "fix: third\n\nRefs #64\n"],
        )

    def test_commit_message_parser_rejects_truncated_output(self) -> None:
        with self.assertRaisesRegex(subject.ContractViolation, "途中で切れています"):
            subject.parse_commit_messages("feat: missing NUL")

    def test_closing_issue_numbers_accepts_keywords_short_and_same_repo_urls(self) -> None:
        body = "\n".join(
            (
                "Closes #64",
                "fixed: https://github.com/HiroyukiFuruno/katana-render-runtime/issues/65",
                "RESOLVED #66",
                "Refs #67",
                "Fixes https://github.com/other/repository/issues/68",
            )
        )
        self.assertEqual(
            subject.closing_issue_numbers(
                body, "HiroyukiFuruno/katana-render-runtime"
            ),
            {64, 65, 66},
        )

    def test_closing_issue_numbers_requires_full_url_issue_number_boundary(self) -> None:
        body = "\n".join(
            (
                "Closes https://github.com/HiroyukiFuruno/katana-render-runtime/issues/64",
                "Fixes https://github.com/HiroyukiFuruno/katana-render-runtime/issues/640",
                "Resolves https://github.com/HiroyukiFuruno/katana-render-runtime/issues/64x",
            )
        )
        self.assertEqual(
            subject.closing_issue_numbers(
                body, "HiroyukiFuruno/katana-render-runtime"
            ),
            {64, 640},
        )

    def test_full_url_issue_references_require_a_strict_terminal(self) -> None:
        repository = "HiroyukiFuruno/katana-render-runtime"
        valid = (
            "Refs [https://github.com/HiroyukiFuruno/katana-render-runtime/issues/64].\n"
            "Refs (https://github.com/HiroyukiFuruno/katana-render-runtime/issues/640)\n"
            "Refs 'https://github.com/HiroyukiFuruno/katana-render-runtime/issues/641'\n"
            'Refs "https://github.com/HiroyukiFuruno/katana-render-runtime/issues/642"'
        )
        self.assertEqual(subject.issue_numbers(valid, repository), {64, 640, 641, 642})
        malformed = "Refs https://github.com/HiroyukiFuruno/katana-render-runtime/issues/64x"
        self.assertEqual(subject.issue_numbers(malformed, repository), set())
        with self.assertRaisesRegex(subject.ContractViolation, "Issue参照"):
            self.validate(messages=[malformed])

        closing = "\n".join(
            (
                "Closes https://github.com/HiroyukiFuruno/katana-render-runtime/issues/64)",
                "Fixes https://github.com/HiroyukiFuruno/katana-render-runtime/issues/640.",
                "Closes https://github.com/HiroyukiFuruno/katana-render-runtime/issues/641'",
                'Fixes https://github.com/HiroyukiFuruno/katana-render-runtime/issues/642"',
                "Resolves https://github.com/HiroyukiFuruno/katana-render-runtime/issues/64/extra",
            )
        )
        self.assertEqual(
            subject.closing_issue_numbers(closing, repository), {64, 640, 641, 642}
        )

    def test_remote_name_with_distinct_fetch_and_push_url_uses_push_url(self) -> None:
        fetch_url = "https://github.com/example/fetch-only.git"
        push_url = "https://github.com/HiroyukiFuruno/katana-render-runtime.git"

        def run_git(_repository: Path, *arguments: str) -> str:
            if arguments == ("remote", "get-url", "origin"):
                return fetch_url
            if arguments == ("remote", "get-url", "--push", "--all", "origin"):
                return push_url
            raise AssertionError(f"unexpected git invocation: {arguments}")

        with patch.object(subject, "_run_git", side_effect=run_git):
            self.assertEqual(
                subject._remote_for_push(
                    Path("/tmp/repository"),
                    remote_name="origin",
                    remote_url=push_url,
                    fallback_branch=None,
                ),
                ("origin", push_url),
            )

    def test_push_url_reverse_resolves_to_configured_remote(self) -> None:
        push_url = "https://github.com/HiroyukiFuruno/katana-render-runtime.git"

        def run_git(_repository: Path, *arguments: str) -> str:
            if arguments == ("remote",):
                return "origin\n"
            if arguments == ("remote", "get-url", "--push", "--all", "origin"):
                return push_url
            raise AssertionError(f"unexpected git invocation: {arguments}")

        with patch.object(subject, "_run_git", side_effect=run_git):
            self.assertEqual(
                subject._remote_for_push(
                    Path("/tmp/repository"),
                    remote_name=push_url,
                    remote_url=push_url,
                    fallback_branch=None,
                ),
                ("origin", push_url),
            )

    def test_mismatched_remote_name_and_push_url_fails_closed(self) -> None:
        fetch_url = "https://github.com/example/fetch-only.git"
        other_push_url = "https://github.com/example/other.git"
        requested_push_url = "https://github.com/HiroyukiFuruno/katana-render-runtime.git"

        def run_git(_repository: Path, *arguments: str) -> str:
            if arguments == ("remote", "get-url", "origin"):
                return fetch_url
            if arguments == ("remote", "get-url", "--push", "--all", "origin"):
                return other_push_url
            raise AssertionError(f"unexpected git invocation: {arguments}")

        with patch.object(subject, "_run_git", side_effect=run_git):
            with self.assertRaises(subject.ContractViolation):
                subject._remote_for_push(
                    Path("/tmp/repository"),
                    remote_name="origin",
                    remote_url=requested_push_url,
                    fallback_branch=None,
                )

    def test_matching_push_url_does_not_expose_credential_from_unrecognized_url(self) -> None:
        credential_url = "https://token:top-secret@github.invalid/owner/repository.git"

        def run_git(_repository: Path, *arguments: str) -> str:
            if arguments == ("remote", "get-url", "--push", "--all", "origin"):
                return credential_url
            raise AssertionError(f"unexpected git invocation: {arguments}")

        with patch.object(subject, "_run_git", side_effect=run_git):
            with self.assertRaises(subject.ContractViolation) as captured:
                subject._matching_push_url(Path("/tmp/repository"), "origin", None)
        self.assertEqual(
            str(captured.exception), "GitHub repositoryをremote URLから判定できません"
        )
        self.assertNotIn("top-secret", str(captured.exception))
        self.assertNotIn("github.invalid", str(captured.exception))

    def test_main_does_not_expose_credential_from_unrecognized_push_url(self) -> None:
        credential_url = "https://token:top-secret@github.invalid/owner/repository.git"
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory)
            subprocess.run(
                ["git", "init", "--initial-branch=master"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            subprocess.run(
                ["git", "remote", "add", "origin", credential_url],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            result = subprocess.run(
                [sys.executable, str(Path(subject.__file__)), "--remote", "origin"],
                cwd=repository,
                capture_output=True,
                text=True,
                check=False,
            )
        self.assertEqual(result.returncode, 1)
        self.assertEqual(
            result.stderr.strip(),
            "Issue contract failed: GitHub repositoryをremote URLから判定できません",
        )
        self.assertNotIn("top-secret", result.stderr)
        self.assertNotIn("github.invalid", result.stderr)

    def test_python_39_compatibility_contract_is_present_and_help_starts(self) -> None:
        source = Path(subject.__file__).read_text(encoding="utf-8")
        self.assertIn("from __future__ import annotations", source)
        python39 = Path("/usr/bin/python3")
        if python39.exists():
            result = subprocess.run(
                [str(python39), str(Path(subject.__file__)), "--help"],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_pushed_branch_updates_keeps_multiple_branches_and_skips_delete_default_and_tag(
        self,
    ) -> None:
        updates = subject.pushed_branch_updates(
            (
                ("refs/heads/feature-a", "a" * 40, "refs/heads/feature-a", "0" * 40),
                ("refs/heads/feature-b", "b" * 40, "refs/heads/feature-b", "0" * 40),
                ("refs/heads/master", "c" * 40, "refs/heads/master", "0" * 40),
                ("refs/heads/deleted", "0" * 40, "refs/heads/deleted", "d" * 40),
                ("refs/tags/v1", "e" * 40, "refs/tags/v1", "0" * 40),
                ("refs/heads/feature-a", "a" * 40, "refs/heads/feature-a", "0" * 40),
            ),
            default_branch="master",
        )
        self.assertEqual(updates, (("feature-a", "a" * 40), ("feature-b", "b" * 40)))

    def issue(
        self,
        number: int = 64,
        *,
        state: str = "OPEN",
        body: str = "Issue body",
    ) -> subject.Issue:
        return subject.Issue(
            number=number,
            state=state,
            body=body,
            url=f"https://github.com/HiroyukiFuruno/katana-render-runtime/issues/{number}",
            updated_at="2026-08-29T03:03:00Z",
        )

    def validate(
        self,
        *,
        branch: str = "feature/contract",
        messages: list[str] | None = None,
        changed_paths: list[str] | None = None,
        issue: subject.Issue | None = None,
    ) -> None:
        selected_issue = issue or self.issue()
        subject.validate_contract(
            branch=branch,
            default_branch="master",
            repository="HiroyukiFuruno/katana-render-runtime",
            commit_messages=messages or ["feat: add contract\n\nRefs #64"],
            changed_paths=changed_paths or ["scripts/hooks/pre-push.sh"],
            issue_loader=lambda number: selected_issue
            if number == selected_issue.number
            else None,
        )

    def test_default_branch_does_not_require_an_issue_reference(self) -> None:
        self.validate(branch="master", messages=["chore: direct maintenance"])

    def test_non_default_commit_requires_an_issue_reference(self) -> None:
        with self.assertRaisesRegex(subject.ContractViolation, "Issue参照"):
            self.validate(messages=["feat: missing issue"])

    def test_empty_commit_message_is_not_dropped_from_issue_contract(self) -> None:
        messages = subject.parse_commit_messages("\0")
        with self.assertRaisesRegex(subject.ContractViolation, "Issue参照"):
            self.validate(messages=messages)

    def test_multiple_commit_messages_keep_their_individual_contracts(self) -> None:
        messages = subject.parse_commit_messages(
            "feat: first\n\nRefs #64\n\0fix: second\n\nRefs #64\n\0"
        )
        self.validate(messages=messages)

    def test_push_contract_allows_multiple_commits_to_reference_different_issues(self) -> None:
        messages = [
            "feat: first\n\nRefs #64",
            "fix: second\n\nRefs #65",
        ]
        issues = {64: self.issue(64), 65: self.issue(65)}
        subject.validate_contract(
            branch="feature/contract",
            default_branch="master",
            repository="HiroyukiFuruno/katana-render-runtime",
            commit_messages=messages,
            changed_paths=["scripts/hooks/pre-push.sh"],
            issue_loader=issues.get,
        )

    def test_foreign_repository_issue_does_not_satisfy_the_contract(self) -> None:
        with self.assertRaisesRegex(subject.ContractViolation, "Issue参照"):
            self.validate(
                messages=[
                    "feat: wrong issue\n\n"
                    "Refs https://github.com/example/other/issues/64"
                ]
            )

    def test_referenced_issue_must_be_open(self) -> None:
        with self.assertRaisesRegex(subject.ContractViolation, "OPEN"):
            self.validate(issue=self.issue(state="CLOSED"))

    def test_lockfile_only_transitive_update_still_requires_dependency_evidence(self) -> None:
        with self.assertRaisesRegex(subject.ContractViolation, "依存更新証跡"):
            self.validate(changed_paths=["Cargo.lock"])

    def test_renamed_lockfile_requires_dependency_evidence_for_its_old_path(self) -> None:
        changed_paths = subject.parse_name_status_paths(
            "R100\0Cargo.lock\0renamed.txt\0"
        )
        with self.assertRaisesRegex(subject.ContractViolation, "依存更新証跡"):
            self.validate(changed_paths=changed_paths)

    def test_dependency_issue_requires_all_evidence_fields(self) -> None:
        body = """## 依存更新証跡
- 上流公開版: serde 2.0.0
- API移行: 互換変更なし
- 依存manifest: Cargo.toml
- lockfile: Cargo.lock
"""
        with self.assertRaisesRegex(subject.ContractViolation, "検証証跡"):
            self.validate(
                changed_paths=["Cargo.toml", "Cargo.lock"],
                issue=self.issue(body=body),
            )

    def test_dependency_non_path_evidence_rejects_placeholder_only_values(self) -> None:
        fields = (
            (
                "Upstream release",
                "上流公開版",
                "serde 2.0.0 https://crates.io/crates/serde/2.0.0",
            ),
            ("API migration", "API移行", "no migration required"),
            ("Verification", "検証証跡", "just check passed"),
        )
        for index, (label, expected_error, value) in enumerate(fields):
            for placeholder in ("N/A", "n/a", "NA", "n.a.", "none", "TBD"):
                with self.subTest(label=label, placeholder=placeholder):
                    rendered = list(fields)
                    rendered[index] = (label, expected_error, placeholder)
                    body = "## Dependency Update Evidence\n" + "\n".join(
                        f"- {field_label}: {field_value}" for field_label, _, field_value in rendered
                    ) + "\n- Dependency manifests: Cargo.toml\n- Lockfiles: Cargo.lock\n"
                    with self.assertRaisesRegex(subject.ContractViolation, expected_error):
                        self.validate(
                            changed_paths=["Cargo.toml", "Cargo.lock"],
                            issue=self.issue(body=body),
                        )

    def test_dependency_issue_must_name_changed_contract_files(self) -> None:
        body = """## Dependency Update Evidence
- Upstream release: serde 2.0.0
- API migration: no migration required
- Dependency manifests: package.json
- Lockfiles: bun.lock
- Verification: just check passed
"""
        with self.assertRaisesRegex(subject.ContractViolation, "Cargo.toml"):
            self.validate(
                changed_paths=["Cargo.toml", "Cargo.lock"],
                issue=self.issue(body=body),
            )

    def test_dependency_manifest_evidence_rejects_na_and_lookalike_paths(self) -> None:
        for manifest in (
            "N/A",
            "Cargo.toml.bak",
            "before-Cargo.toml",
            "./Cargo.toml.old",
            "Cargo.tomlα",
            "αCargo.toml",
            r"x\Cargo.toml",
            "x／Cargo.toml",
            "x@Cargo.toml",
            "Cargo.toml@x",
        ):
            with self.subTest(manifest=manifest):
                body = f"""## Dependency Update Evidence
- Upstream release: serde 2.0.0 https://crates.io/crates/serde/2.0.0
- API migration: no migration required
- Dependency manifests: {manifest}
- Lockfiles: Cargo.lock
- Verification: just check passed
"""
                with self.assertRaisesRegex(subject.ContractViolation, "Cargo.toml"):
                    self.validate(
                        changed_paths=["Cargo.toml", "Cargo.lock"],
                        issue=self.issue(body=body),
                    )

    def test_dependency_manifest_evidence_accepts_unicode_directory_path(self) -> None:
        body = """## Dependency Update Evidence
- Upstream release: serde 2.0.0 https://crates.io/crates/serde/2.0.0
- API migration: no migration required
- Dependency manifests: `設定/日本語/Cargo.toml`
- Lockfiles: `設定/日本語/Cargo.lock`
- Verification: just check passed
"""
        self.validate(
            changed_paths=["設定/日本語/Cargo.toml", "設定/日本語/Cargo.lock"],
            issue=self.issue(body=body),
        )

    def test_dependency_manifest_evidence_preserves_quoted_space_and_comma_paths(self) -> None:
        body = """## Dependency Update Evidence
- Upstream release: serde 2.0.0 https://crates.io/crates/serde/2.0.0
- API migration: no migration required
- Dependency manifests: `dir with space,comma/Cargo.toml`
- Lockfiles: `dir with space,comma/Cargo.lock`
- Verification: just check passed
"""
        self.validate(
            changed_paths=[
                "dir with space,comma/Cargo.toml",
                "dir with space,comma/Cargo.lock",
            ],
            issue=self.issue(body=body),
        )

    def test_dependency_manifest_evidence_rejects_malformed_quoted_tokens(self) -> None:
        for manifest in (
            "`Cargo.toml",
            "``",
            "`Cargo.toml`package.json",
            "Cargo.toml`",
        ):
            with self.subTest(manifest=manifest):
                body = f"""## Dependency Update Evidence
- Upstream release: serde 2.0.0 https://crates.io/crates/serde/2.0.0
- API migration: no migration required
- Dependency manifests: {manifest}
- Lockfiles: Cargo.lock
- Verification: just check passed
"""
                with self.assertRaises(subject.ContractViolation):
                    self.validate(
                        changed_paths=["Cargo.toml", "Cargo.lock"],
                        issue=self.issue(body=body),
                    )

    def test_dependency_manifest_evidence_rejects_extra_or_duplicate_tokens(self) -> None:
        for manifest in (
            "Cargo.toml, N/A",
            "Cargo.toml; package.json",
            "Cargo.toml, Cargo.toml",
        ):
            with self.subTest(manifest=manifest):
                body = f"""## Dependency Update Evidence
- Upstream release: serde 2.0.0 https://crates.io/crates/serde/2.0.0
- API migration: no migration required
- Dependency manifests: {manifest}
- Lockfiles: Cargo.lock
- Verification: just check passed
"""
                with self.assertRaisesRegex(subject.ContractViolation, "依存manifest"):
                    self.validate(
                        changed_paths=["Cargo.toml", "Cargo.lock"],
                        issue=self.issue(body=body),
                    )

    def test_dependency_lockfile_evidence_requires_exact_lockfile_field(self) -> None:
        bodies = (
            "other.lock (Cargo.lock verified)",
            "Cargo.lock, other.lock",
            "Cargo.lock, Cargo.lock",
            "N/A",
        )
        for lockfiles in bodies:
            with self.subTest(lockfiles=lockfiles):
                body = f"""## Dependency Update Evidence
- Upstream release: serde 2.0.0 https://crates.io/crates/serde/2.0.0
- API migration: no migration required
- Dependency manifests: Cargo.toml
- Lockfiles: {lockfiles}
- Verification: Cargo.lock just check passed
"""
                with self.assertRaisesRegex(subject.ContractViolation, "lockfile"):
                    self.validate(
                        changed_paths=["Cargo.toml", "Cargo.lock"],
                        issue=self.issue(body=body),
                    )
        for lockfiles in ("`Cargo.lock", "`Cargo.lock`other.lock"):
            with self.subTest(lockfiles=lockfiles):
                body = f"""## Dependency Update Evidence
- Upstream release: serde 2.0.0 https://crates.io/crates/serde/2.0.0
- API migration: no migration required
- Dependency manifests: Cargo.toml
- Lockfiles: {lockfiles}
- Verification: just check passed
"""
                with self.assertRaises(subject.ContractViolation):
                    self.validate(
                        changed_paths=["Cargo.toml", "Cargo.lock"],
                        issue=self.issue(body=body),
                    )

    def test_dependency_lockfile_path_elsewhere_does_not_satisfy_lockfile_field(self) -> None:
        body = """## Dependency Update Evidence
- Upstream release: serde 2.0.0 https://crates.io/crates/serde/2.0.0
- API migration: no migration required
- Dependency manifests: Cargo.toml
- Lockfiles: other.lock
- Verification: Cargo.lock just check passed
"""
        with self.assertRaisesRegex(subject.ContractViolation, "lockfile"):
            self.validate(
                changed_paths=["Cargo.toml", "Cargo.lock"],
                issue=self.issue(body=body),
            )

    def test_dependency_manifest_evidence_accepts_multiple_exact_paths(self) -> None:
        body = """## Dependency Update Evidence
- Upstream release: serde 2.0.0 https://crates.io/crates/serde/2.0.0
- API migration: no migration required
- Dependency manifests: `Cargo.toml`, `package.json`
- Lockfiles: `Cargo.lock`, `package-lock.json`
- Verification: just check passed
"""
        self.validate(
            changed_paths=[
                "Cargo.toml",
                "package.json",
                "Cargo.lock",
                "package-lock.json",
            ],
            issue=self.issue(body=body),
        )

    def test_manifest_path_elsewhere_does_not_satisfy_manifest_evidence(self) -> None:
        body = """## Dependency Update Evidence
- Upstream release: Cargo.toml was updated with serde 2.0.0
- API migration: no migration required
- Dependency manifests: N/A
- Lockfiles: Cargo.lock
- Verification: just check passed
"""
        with self.assertRaisesRegex(subject.ContractViolation, "Cargo.toml"):
            self.validate(
                changed_paths=["Cargo.toml", "Cargo.lock"],
                issue=self.issue(body=body),
            )

    def test_complete_dependency_evidence_satisfies_the_contract(self) -> None:
        body = """## 依存更新証跡
- 上流公開版: serde 2.0.0 https://crates.io/crates/serde/2.0.0
- API移行: 互換変更のため移行不要
- 依存manifest: Cargo.toml
- lockfile: Cargo.lock
- 検証証跡: just check 成功
"""
        self.validate(
            changed_paths=["Cargo.toml", "Cargo.lock"],
            issue=self.issue(body=body),
        )

    def test_lockfile_only_transitive_update_accepts_complete_evidence(self) -> None:
        body = """## Dependency Update Evidence
- Upstream release: serde 2.0.0 https://crates.io/crates/serde/2.0.0
- API migration: no migration required
- Dependency manifest: Cargo.toml
- Lockfiles: Cargo.lock
- Verification: just check passed
"""
        self.validate(
            changed_paths=["Cargo.lock"],
            issue=self.issue(body=body),
        )

    def test_lockfile_only_update_requires_the_real_origin_manifest(self) -> None:
        for manifest in ("N/A", "n/a", "NA", "na", " ", "package.json"):
            with self.subTest(manifest=manifest):
                body = f"""## Dependency Update Evidence
- Upstream release: serde 2.0.0 https://crates.io/crates/serde/2.0.0
- API migration: no migration required
- Dependency manifest: {manifest}
- Lockfiles: Cargo.lock
- Verification: just check passed
"""
                with self.assertRaisesRegex(subject.ContractViolation, "依存manifest"):
                    self.validate(
                        changed_paths=["Cargo.lock"],
                        issue=self.issue(body=body),
                    )

    def test_lockfile_only_update_keeps_real_origin_manifest_case_sensitive(self) -> None:
        body = """## 依存更新証跡
- 上流公開版: serde 2.0.0 https://crates.io/crates/serde/2.0.0
- API移行: 移行不要
- 依存manifest: cargo.toml
- lockfile: Cargo.lock
- 検証証跡: just check 成功
"""
        with self.assertRaisesRegex(subject.ContractViolation, "依存manifest"):
            self.validate(
                changed_paths=["Cargo.lock"],
                issue=self.issue(body=body),
            )

    def test_pr_range_validates_github_metadata_without_git_or_pr_checkout(self) -> None:
        base_sha = "a" * 40
        head_sha = "b" * 40
        compare = {
            "base_commit": {"sha": base_sha},
            "merge_base_commit": {"sha": base_sha},
            "status": "ahead",
            "ahead_by": 1,
            "behind_by": 0,
            "total_commits": 1,
            "commits": [
                {"sha": head_sha, "commit": {"message": "feat: contract\n\nRefs #64"}}
            ],
            "files": [{"filename": "scripts/hooks/pre-push.sh"}],
        }

        def gh_json(*arguments: str) -> object:
            if arguments == (
                f"repos/HiroyukiFuruno/katana-render-runtime/compare/{base_sha}...{head_sha}",
            ):
                return compare
            raise AssertionError(f"unexpected GitHub API request: {arguments}")

        with patch.object(subject, "_gh_json", side_effect=gh_json), patch.object(
            subject,
            "_run_git",
            side_effect=AssertionError("PR range mode must not invoke git or check out PR code"),
        ):
            references = subject.validate_pr_range(
                repository="HiroyukiFuruno/katana-render-runtime",
                pr_number=72,
                base_sha=base_sha,
                head_sha=head_sha,
                branch="fix/issue-contract",
                issue_loader=lambda number: self.issue(number),
            )

        self.assertEqual(references, {64})

    def test_pr_range_binds_changed_paths_to_the_same_immutable_base_and_head(self) -> None:
        base_sha = "a" * 40
        head_sha = "b" * 40
        comparison = {
            "base_commit": {"sha": base_sha},
            "merge_base_commit": {"sha": base_sha},
            "status": "ahead",
            "ahead_by": 1,
            "behind_by": 0,
            "total_commits": 1,
            "commits": [
                {"sha": head_sha, "commit": {"message": "fix: contract\n\nRefs #64"}}
            ],
            "files": [{"filename": "scripts/hooks/pre-push.sh"}],
        }
        calls: list[tuple[str, ...]] = []

        def gh_json(*arguments: str) -> object:
            calls.append(arguments)
            self.assertEqual(
                arguments,
                (f"repos/HiroyukiFuruno/katana-render-runtime/compare/{base_sha}...{head_sha}",),
            )
            return comparison

        with patch.object(subject, "_gh_json", side_effect=gh_json):
            self.assertEqual(
                subject.validate_pr_range(
                    repository="HiroyukiFuruno/katana-render-runtime",
                    pr_number=72,
                    base_sha=base_sha,
                    head_sha=head_sha,
                    branch="fix/issue-contract",
                    issue_loader=lambda number: self.issue(number),
                ),
                {64},
            )
        self.assertEqual(len(calls), 2)

    def test_pr_range_rejects_zero_referenced_issues(self) -> None:
        base_sha = "a" * 40
        head_sha = "b" * 40
        compare = {
            "base_commit": {"sha": base_sha},
            "merge_base_commit": {"sha": base_sha},
            "status": "ahead",
            "ahead_by": 1,
            "behind_by": 0,
            "total_commits": 1,
            "commits": [
                {"sha": head_sha, "commit": {"message": "fix: unlinked"}}
            ],
        }
        with patch.object(subject, "_gh_json", return_value=compare):
            with self.assertRaisesRegex(subject.ContractViolation, "1件"):
                subject.validate_pr_range(
                    repository="HiroyukiFuruno/katana-render-runtime",
                    pr_number=72,
                    base_sha=base_sha,
                    head_sha=head_sha,
                    branch="fix/issue-contract",
                    issue_loader=lambda number: self.issue(number),
                )

    def test_pr_range_rejects_multiple_referenced_issues(self) -> None:
        base_sha = "a" * 40
        head_sha = "b" * 40
        compare = {
            "base_commit": {"sha": base_sha},
            "merge_base_commit": {"sha": base_sha},
            "status": "ahead",
            "ahead_by": 1,
            "behind_by": 0,
            "total_commits": 1,
            "commits": [
                {
                    "sha": head_sha,
                    "commit": {"message": "fix: linked\n\nRefs #64 #65"},
                }
            ],
        }
        with patch.object(subject, "_gh_json", return_value=compare):
            with self.assertRaisesRegex(subject.ContractViolation, "1件"):
                subject.validate_pr_range(
                    repository="HiroyukiFuruno/katana-render-runtime",
                    pr_number=72,
                    base_sha=base_sha,
                    head_sha=head_sha,
                    branch="fix/issue-contract",
                    issue_loader=lambda number: self.issue(number),
                )

    def test_pr_range_rejects_noncanonical_issue_url(self) -> None:
        base_sha = "a" * 40
        head_sha = "b" * 40
        compare = {
            "base_commit": {"sha": base_sha},
            "merge_base_commit": {"sha": base_sha},
            "status": "ahead",
            "ahead_by": 1,
            "behind_by": 0,
            "total_commits": 1,
            "commits": [
                {"sha": head_sha, "commit": {"message": "fix: linked\n\nRefs #64"}}
            ],
        }
        noncanonical = self.issue(64)
        noncanonical = subject.Issue(
            number=noncanonical.number,
            state=noncanonical.state,
            body=noncanonical.body,
            url="https://github.com/example/other/issues/64",
            updated_at=noncanonical.updated_at,
        )
        with patch.object(subject, "_gh_json", return_value=compare):
            with self.assertRaisesRegex(subject.ContractViolation, "canonical"):
                subject.validate_pr_range(
                    repository="HiroyukiFuruno/katana-render-runtime",
                    pr_number=72,
                    base_sha=base_sha,
                    head_sha=head_sha,
                    branch="fix/issue-contract",
                    issue_loader=lambda _number: noncanonical,
                )

    def test_pr_range_rejects_non_integer_canonical_issue_number(self) -> None:
        base_sha = "a" * 40
        head_sha = "b" * 40
        compare = {
            "base_commit": {"sha": base_sha},
            "merge_base_commit": {"sha": base_sha},
            "status": "ahead",
            "ahead_by": 1,
            "behind_by": 0,
            "total_commits": 1,
            "commits": [
                {"sha": head_sha, "commit": {"message": "fix: linked\n\nRefs #64"}}
            ],
        }

        for invalid_number in (True, "64"):
            with self.subTest(invalid_number=invalid_number):
                invalid_issue = subject.Issue(
                    number=invalid_number,  # type: ignore[arg-type]
                    state="OPEN",
                    body="Issue body",
                    url="https://github.com/HiroyukiFuruno/katana-render-runtime/issues/64",
                    updated_at="2026-08-29T03:03:00Z",
                )
                with patch.object(subject, "_gh_json", return_value=compare):
                    with self.assertRaisesRegex(subject.ContractViolation, "snapshot番号"):
                        subject.validate_pr_range(
                            repository="HiroyukiFuruno/katana-render-runtime",
                            pr_number=72,
                            base_sha=base_sha,
                            head_sha=head_sha,
                            branch="fix/issue-contract",
                            issue_loader=lambda _number: invalid_issue,
                        )

    def test_referenced_issue_snapshot_uses_complete_base_to_head_commit_references(self) -> None:
        base_sha = "a" * 40
        head_sha = "b" * 40
        compare = {
            "base_commit": {"sha": base_sha},
            "merge_base_commit": {"sha": base_sha},
            "status": "ahead",
            "ahead_by": 2,
            "behind_by": 0,
            "total_commits": 2,
            "commits": [
                {"sha": "c" * 40, "commit": {"message": "feat: first\n\nRefs #64"}},
                {
                    "sha": head_sha,
                    "commit": {
                        "message": "fix: second\n\nRefs https://github.com/HiroyukiFuruno/katana-render-runtime/issues/65"
                    },
                },
            ],
        }
        with patch.object(subject, "_gh_json", return_value=compare):
            snapshot = subject.referenced_issue_snapshot(
                repository="HiroyukiFuruno/katana-render-runtime",
                base_sha=base_sha,
                head_sha=head_sha,
                issue_loader=lambda number: self.issue(number),
            )
        self.assertEqual([issue.number for issue in snapshot], [64, 65])

    def test_referenced_issue_snapshot_fails_closed_on_missing_updated_at(self) -> None:
        base_sha = "a" * 40
        head_sha = "b" * 40
        compare = {
            "base_commit": {"sha": base_sha},
            "merge_base_commit": {"sha": base_sha},
            "status": "ahead",
            "ahead_by": 1,
            "behind_by": 0,
            "total_commits": 1,
            "commits": [
                {"sha": head_sha, "commit": {"message": "fix: freshness\n\nRefs #64"}}
            ],
        }
        missing_time = subject.Issue(
            64,
            "OPEN",
            "body",
            "https://github.com/HiroyukiFuruno/katana-render-runtime/issues/64",
        )
        with patch.object(subject, "_gh_json", return_value=compare):
            with self.assertRaisesRegex(subject.ContractViolation, "updated_at"):
                subject.referenced_issue_snapshot(
                    repository="HiroyukiFuruno/katana-render-runtime",
                    base_sha=base_sha,
                    head_sha=head_sha,
                    issue_loader=lambda _number: missing_time,
                )

    def test_referenced_issue_snapshot_rejects_noncanonical_or_noninteger_issue(self) -> None:
        repository = "HiroyukiFuruno/katana-render-runtime"
        base_sha = "a" * 40
        head_sha = "b" * 40
        cases = (
            subject.Issue(
                True,
                "OPEN",
                "body",
                f"https://github.com/{repository}/issues/64",
                "2026-08-29T03:03:00Z",
            ),
            subject.Issue(
                64,
                "OPEN",
                "body",
                "https://github.com/example/other/issues/64",
                "2026-08-29T03:03:00Z",
            ),
        )
        with patch.object(
            subject,
            "_pr_commit_messages",
            return_value=["fix: canonical snapshot\n\nRefs #64"],
        ):
            for invalid_issue in cases:
                with self.subTest(issue=invalid_issue):
                    with self.assertRaisesRegex(subject.ContractViolation, "snapshot番号|canonical"):
                        subject.referenced_issue_snapshot(
                            repository=repository,
                            base_sha=base_sha,
                            head_sha=head_sha,
                            issue_loader=lambda _number: invalid_issue,
                        )

    def test_pr_range_fails_closed_when_compare_commits_are_truncated(self) -> None:
        base_sha = "a" * 40
        head_sha = "b" * 40
        compare = {
            "base_commit": {"sha": base_sha},
            "merge_base_commit": {"sha": base_sha},
            "status": "ahead",
            "ahead_by": 2,
            "behind_by": 0,
            "total_commits": 2,
            "commits": [
                {"sha": head_sha, "commit": {"message": "feat: only one\n\nRefs #64"}}
            ],
        }

        with patch.object(subject, "_gh_json", return_value=compare):
            with self.assertRaisesRegex(subject.ContractViolation, "全commit"):
                subject.validate_pr_range(
                    repository="HiroyukiFuruno/katana-render-runtime",
                    pr_number=72,
                    base_sha=base_sha,
                    head_sha=head_sha,
                    branch="fix/issue-contract",
                    issue_loader=lambda number: self.issue(number),
                )

    def test_pr_range_rejects_changes_to_any_workflow(self) -> None:
        base_sha = "a" * 40
        head_sha = "b" * 40
        compare = {
            "base_commit": {"sha": base_sha},
            "merge_base_commit": {"sha": base_sha},
            "status": "ahead",
            "ahead_by": 1,
            "behind_by": 0,
            "total_commits": 1,
            "commits": [
                {"sha": head_sha, "commit": {"message": "fix: contract\n\nRefs #64"}}
            ],
        }
        compare["files"] = [{"filename": ".github/workflows/forge-latch.yml"}]

        def gh_json(*arguments: str) -> object:
            return compare

        with patch.object(subject, "_gh_json", side_effect=gh_json):
            with self.assertRaisesRegex(subject.ContractViolation, "GitHub Actions workflow"):
                subject.validate_pr_range(
                    repository="HiroyukiFuruno/katana-render-runtime",
                    pr_number=72,
                    base_sha=base_sha,
                    head_sha=head_sha,
                    branch="fix/issue-contract",
                    issue_loader=lambda number: self.issue(number),
                )

    def test_pr_range_rejects_renaming_any_workflow(self) -> None:
        base_sha = "a" * 40
        head_sha = "b" * 40
        compare = {
            "base_commit": {"sha": base_sha},
            "merge_base_commit": {"sha": base_sha},
            "status": "ahead",
            "ahead_by": 1,
            "behind_by": 0,
            "total_commits": 1,
            "commits": [
                {"sha": head_sha, "commit": {"message": "fix: contract\n\nRefs #64"}}
            ],
        }
        compare["files"] = [
            {
                "filename": ".github/workflows/retired.yml",
                "previous_filename": ".github/workflows/forge-latch.yml",
            }
        ]

        def gh_json(*arguments: str) -> object:
            return compare

        with patch.object(subject, "_gh_json", side_effect=gh_json):
            with self.assertRaisesRegex(subject.ContractViolation, "GitHub Actions workflow"):
                subject.validate_pr_range(
                    repository="HiroyukiFuruno/katana-render-runtime",
                    pr_number=72,
                    base_sha=base_sha,
                    head_sha=head_sha,
                    branch="fix/issue-contract",
                    issue_loader=lambda number: self.issue(number),
                )

    def test_pr_range_rejects_modified_and_deleted_workflows(self) -> None:
        base_sha = "a" * 40
        head_sha = "b" * 40
        compare = {
            "base_commit": {"sha": base_sha},
            "merge_base_commit": {"sha": base_sha},
            "status": "ahead",
            "ahead_by": 1,
            "behind_by": 0,
            "total_commits": 1,
            "commits": [
                {"sha": head_sha, "commit": {"message": "fix: contract\n\nRefs #64"}}
            ],
        }
        cases = {
            "modified": {"filename": ".github/workflows/existing.yml", "status": "modified"},
            "deleted": {"filename": ".github/workflows/retired.yml", "status": "removed"},
        }

        for action, changed_file in cases.items():
            with self.subTest(action=action):
                compare["files"] = [changed_file]
                def gh_json(*arguments: str) -> object:
                    return compare

                with patch.object(subject, "_gh_json", side_effect=gh_json):
                    with self.assertRaisesRegex(
                        subject.ContractViolation,
                        "GitHub Actions workflow",
                    ):
                        subject.validate_pr_range(
                            repository="HiroyukiFuruno/katana-render-runtime",
                            pr_number=72,
                            base_sha=base_sha,
                            head_sha=head_sha,
                            branch="fix/issue-contract",
                            issue_loader=lambda number: self.issue(number),
                        )

    def test_pr_range_fails_when_any_base_to_head_commit_lacks_an_issue(self) -> None:
        base_sha = "a" * 40
        head_sha = "b" * 40
        compare = {
            "base_commit": {"sha": base_sha},
            "merge_base_commit": {"sha": base_sha},
            "status": "ahead",
            "ahead_by": 2,
            "behind_by": 0,
            "total_commits": 2,
            "commits": [
                {
                    "sha": "c" * 40,
                    "commit": {"message": "feat: linked\n\nRefs #64"},
                },
                {"sha": head_sha, "commit": {"message": "fix: unlinked"}},
            ],
            "files": [{"filename": "scripts/hooks/pre-push.sh"}],
        }

        def gh_json(*arguments: str) -> object:
            return compare

        with patch.object(subject, "_gh_json", side_effect=gh_json):
            with self.assertRaisesRegex(subject.ContractViolation, "Issue参照"):
                subject.validate_pr_range(
                    repository="HiroyukiFuruno/katana-render-runtime",
                    pr_number=72,
                    base_sha=base_sha,
                    head_sha=head_sha,
                    branch="fix/issue-contract",
                    issue_loader=lambda number: self.issue(number),
                )

    def test_pr_range_allows_a_base_advanced_after_branch_diverged(self) -> None:
        base_sha = "a" * 40
        head_sha = "b" * 40
        merge_base_sha = "c" * 40
        compare = {
            "base_commit": {"sha": base_sha},
            "merge_base_commit": {"sha": merge_base_sha},
            "status": "diverged",
            "ahead_by": 1,
            "behind_by": 2,
            "total_commits": 1,
            "commits": [
                {"sha": head_sha, "commit": {"message": "fix: contract\n\nRefs #64"}}
            ],
            "files": [{"filename": "scripts/hooks/pre-push.sh"}],
        }

        def gh_json(*arguments: str) -> object:
            return compare

        with patch.object(subject, "_gh_json", side_effect=gh_json):
            self.assertEqual(
                subject.validate_pr_range(
                    repository="HiroyukiFuruno/katana-render-runtime",
                    pr_number=72,
                    base_sha=base_sha,
                    head_sha=head_sha,
                    branch="fix/issue-contract",
                    issue_loader=lambda number: self.issue(number),
                ),
                {64},
            )

    def test_pr_range_fails_when_compare_final_commit_is_not_head(self) -> None:
        base_sha = "a" * 40
        head_sha = "b" * 40
        compare = {
            "base_commit": {"sha": base_sha},
            "merge_base_commit": {"sha": base_sha},
            "status": "ahead",
            "ahead_by": 1,
            "behind_by": 0,
            "total_commits": 1,
            "commits": [
                {"sha": "c" * 40, "commit": {"message": "fix: contract\n\nRefs #64"}}
            ],
        }

        with patch.object(subject, "_gh_json", return_value=compare):
            with self.assertRaisesRegex(subject.ContractViolation, "最終commit"):
                subject.validate_pr_range(
                    repository="HiroyukiFuruno/katana-render-runtime",
                    pr_number=72,
                    base_sha=base_sha,
                    head_sha=head_sha,
                    branch="fix/issue-contract",
                    issue_loader=lambda number: self.issue(number),
                )

    def test_pr_changed_paths_fails_closed_at_github_compare_files_limit(self) -> None:
        base_sha, head_sha = "a" * 40, "b" * 40
        compare = {
            "base_commit": {"sha": base_sha},
            "files": [{"filename": f"fixtures/{entry}.txt"} for entry in range(300)],
        }
        with patch.object(subject, "_gh_json", return_value=compare):
            with self.assertRaisesRegex(subject.ContractViolation, "300件上限"):
                subject._pr_changed_paths(
                    repository="HiroyukiFuruno/katana-render-runtime",
                    base_sha=base_sha,
                    head_sha=head_sha,
                )

    def test_trusted_default_advance_rejects_workflow_but_allows_nonworkflow_change(self) -> None:
        base, trusted = "a" * 40, "b" * 40
        def compare(files: list[dict[str, str]]) -> dict[str, object]:
            return {"base_commit": {"sha": base}, "files": files}
        with patch.object(subject, "_gh_json", return_value=compare([{"filename": ".github/workflows/ci.yml"}])):
            self.assertEqual(
                subject._default_advance_workflow_errors(
                    repository="HiroyukiFuruno/katana-render-runtime", base_sha=base, trusted_default_sha=trusted,
                ),
                [".github/workflows/ci.yml"],
            )
        with patch.object(subject, "_gh_json", return_value=compare([{"filename": "README.md"}])):
            self.assertEqual(
                subject._default_advance_workflow_errors(
                    repository="HiroyukiFuruno/katana-render-runtime", base_sha=base, trusted_default_sha=trusted,
                ),
                [],
            )
        with patch.object(subject, "_gh_json", return_value=compare([{"filename": ".github/workflows/new.yml", "previous_filename": ".github/workflows/old.yml"}])):
            self.assertEqual(
                subject._default_advance_workflow_errors(
                    repository="HiroyukiFuruno/katana-render-runtime", base_sha=base, trusted_default_sha=trusted,
                ),
                [".github/workflows/new.yml", ".github/workflows/old.yml"],
            )

    def test_trusted_dispatcher_observes_pull_request_target_without_checkout_or_pr_code(self) -> None:
        workflow = (
            Path(subject.__file__).parents[2] / ".github/workflows/pr-governance.yml"
        ).read_text(encoding="utf-8")
        writer = (
            Path(subject.__file__).parents[2] / "scripts/review/pr_governance_status_writer.py"
        ).read_text(encoding="utf-8")
        self.assertIn("workflow_run:", workflow)
        self.assertIn("pull_request_target:", workflow)
        self.assertNotIn("actions/checkout", workflow)
        self.assertIn('event_name == "pull_request_target"', workflow)
        self.assertIn('f"repos/{repository}/pulls/{source_number_value}"', workflow)
        self.assertIn("source_is_local = (", workflow)
        self.assertIn('current_base_repository.get("full_name") == repository', workflow)
        self.assertIn('current_head_repository.get("full_name") == repository', workflow)
        self.assertIn("or not source_is_local:", workflow)
        self.assertIn("scripts/hooks/verify_push_issue.py", writer)
        self.assertIn('"--pr-base-sha", base', writer)
        self.assertIn('"--pr-head-sha", head', writer)

    def test_final_single_arbiter_status_rechecks_resolved_base_sha(self) -> None:
        writer = (
            Path(subject.__file__).parents[2] / "scripts/review/pr_governance_status_writer.py"
        ).read_text(encoding="utf-8")
        module = ast.parse(writer)
        finalize = next(
            node for node in module.body
            if isinstance(node, ast.FunctionDef) and node.name == "finalize_decision"
        )
        calls = [node for node in ast.walk(finalize) if isinstance(node, ast.Call)]
        def named_call(node: ast.Call, name: str) -> bool:
            return isinstance(node.func, ast.Name) and node.func.id == name
        def decision_attribute(node: ast.expr, name: str) -> bool:
            return (
                isinstance(node, ast.Attribute) and node.attr == name
                and isinstance(node.value, ast.Name) and node.value.id == "decision"
            )

        closer_calls = [node for node in calls if named_call(node, "final_closer_is_unique")]
        # The final shared evidence/sensor/CI fence is complete before this
        # one closer reread. Duplicate earlier checks would only widen the
        # stale-evidence window.
        self.assertEqual(len(closer_calls), 1)
        closer = closer_calls[0]
        self.assertEqual(len(closer.args), 6)
        self.assertTrue(all(
            decision_attribute(argument, attribute) for argument, attribute in zip(
                closer.args[:5], ("number", "issue", "base", "head", "body_sha256"), strict=True,
            )
        ))
        self.assertIsInstance(closer.args[5], ast.Name)
        assert isinstance(closer.args[5], ast.Name)
        self.assertEqual(closer.args[5].id, "claimants")
        fence_lines = [node.lineno for node in calls if named_call(node, "check_fence")]
        self.assertEqual(len(fence_lines), 1)
        self.assertLess(fence_lines[0], closer.lineno)

        parents = {
            child: parent
            for parent in ast.walk(finalize)
            for child in ast.iter_child_nodes(parent)
        }
        def nearest_parent(node: ast.AST, kind: type[ast.AST]) -> ast.AST | None:
            current = parents.get(node)
            while current is not None:
                if isinstance(current, kind):
                    return current
                current = parents.get(current)
            return None
        success_guard = next(
            (
                parent for parent in ast.walk(finalize)
                if isinstance(parent, ast.If)
                and isinstance(parent.test, ast.BoolOp)
                and any(node is closer for node in ast.walk(parent.test))
            ),
            None,
        )
        self.assertIsNotNone(success_guard)
        assert isinstance(success_guard, ast.If)
        self.assertTrue(any(
            isinstance(node, ast.Compare)
            and isinstance(node.left, ast.Name) and node.left.id == "state"
            and len(node.ops) == len(node.comparators) == 1
            and isinstance(node.ops[0], ast.Eq)
            and isinstance(node.comparators[0], ast.Constant) and node.comparators[0].value == "success"
            for node in ast.walk(success_guard.test)
        ))
        rebinds = [node for node in calls if named_call(node, "rebind_trusted_default_writer")]
        self.assertEqual(len(rebinds), 2)
        terminal_try = next((node for node in finalize.body if isinstance(node, ast.Try)), None)
        self.assertIsInstance(terminal_try, ast.Try)
        assert isinstance(terminal_try, ast.Try)
        terminal_rebinds = [
            node for node in rebinds
            if nearest_parent(node, ast.Try) is terminal_try
            and nearest_parent(node, ast.ExceptHandler) is None
        ]
        self.assertEqual(len(terminal_rebinds), 1)
        terminal_rebind = terminal_rebinds[0]
        # Directly under try, not inside a state guard: every terminal state
        # must bind the current default ref immediately before its PATCH path.
        self.assertIsNone(nearest_parent(terminal_rebind, ast.If))
        self.assertGreater(terminal_rebind.lineno, closer.lineno)
        terminal_writes = [
            node for node in calls if named_call(node, "write_governance_check")
            and nearest_parent(node, ast.Try) is terminal_try
            and nearest_parent(node, ast.ExceptHandler) is None
        ]
        self.assertEqual(len(terminal_writes), 1)
        self.assertLess(terminal_rebind.lineno, terminal_writes[0].lineno)
        fallback_rebinds = [
            node for node in rebinds
            if isinstance(nearest_parent(node, ast.ExceptHandler), ast.ExceptHandler)
        ]
        self.assertEqual(len(fallback_rebinds), 1)
        fallback_parent = nearest_parent(fallback_rebinds[0], ast.ExceptHandler)
        self.assertIsInstance(fallback_parent, ast.ExceptHandler)
        assert isinstance(fallback_parent, ast.ExceptHandler)
        self.assertIsInstance(fallback_parent.type, ast.Name)
        assert isinstance(fallback_parent.type, ast.Name)
        self.assertEqual(fallback_parent.type.id, "GovernanceError")

    def test_new_branch_without_upstream_uses_origin(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory)
            subprocess.run(
                ["git", "init", "--initial-branch=master"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            subprocess.run(
                [
                    "git",
                    "remote",
                    "add",
                    "origin",
                    "https://github.com/HiroyukiFuruno/katana-render-runtime.git",
                ],
                cwd=repository,
                check=True,
            )
            self.assertEqual(subject.branch_remote(repository, "feature/new"), "origin")

    def test_cli_remote_option_uses_selected_remote_repository_and_default_ref(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repository = root / "repository"
            binary_directory = root / "bin"
            repository.mkdir()
            binary_directory.mkdir()

            def git(*arguments: str) -> str:
                result = subprocess.run(
                    ["git", *arguments],
                    cwd=repository,
                    check=True,
                    capture_output=True,
                    text=True,
                )
                return result.stdout.strip()

            git("init", "--initial-branch=master")
            git("config", "user.name", "Issue Contract Test")
            git("config", "user.email", "issue@example.com")
            (repository / "base.txt").write_text("base\n", encoding="utf-8")
            git("add", "base.txt")
            git("commit", "-m", "initial")
            base_sha = git("rev-parse", "HEAD")
            self.configure_live_fetch_remote(
                repository,
                remote="origin",
                push_url="https://github.com/example/wrong-repository.git",
            )
            self.configure_live_fetch_remote(
                repository,
                remote="upstream",
                push_url="https://github.com/HiroyukiFuruno/katana-render-runtime.git",
            )
            git("update-ref", "refs/remotes/upstream/master", base_sha)
            git("symbolic-ref", "refs/remotes/upstream/HEAD", "refs/remotes/upstream/master")
            git("update-ref", "refs/remotes/origin/master", base_sha)
            git("symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/master")
            git("switch", "-c", "topic")
            (repository / "topic.txt").write_text("topic\n", encoding="utf-8")
            git("add", "topic.txt")
            git("commit", "-m", "feat: contract", "-m", "Refs #64")
            topic_sha = git("rev-parse", "HEAD")
            fake_gh = binary_directory / "gh"
            fake_gh.write_text(
                "#!/bin/sh\n"
                "test \"$5\" = \"HiroyukiFuruno/katana-render-runtime\" || exit 21\n"
                "printf '%s\\n' "
                "'{\"number\":64,\"state\":\"OPEN\",\"body\":\"Issue body\","
                "\"url\":\"https://github.com/HiroyukiFuruno/katana-render-runtime/issues/64\"}'\n",
                encoding="utf-8",
            )
            fake_gh.chmod(fake_gh.stat().st_mode | stat.S_IXUSR)
            self.install_fake_push_remote_git(binary_directory)
            environment = os.environ.copy()
            environment["PATH"] = f"{binary_directory}:{environment['PATH']}"
            environment["KRR_TEST_REAL_GIT"] = str(shutil.which("git"))
            environment["KRR_TEST_PUSH_REMOTE_REF"] = "refs/remotes/upstream/master"
            result = subprocess.run(
                [sys.executable, str(Path(subject.__file__)), "--remote", "upstream"],
                cwd=repository,
                env=environment,
                input=f"refs/heads/topic {topic_sha} refs/heads/topic {'0' * 40}\n",
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("issues=#64", result.stdout)

    def test_cli_validates_the_first_push_of_a_new_branch(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repository = root / "repository"
            binary_directory = root / "bin"
            repository.mkdir()
            binary_directory.mkdir()
            commands = [
                ["git", "init", "--initial-branch=master"],
                ["git", "config", "user.name", "Issue Contract Test"],
                ["git", "config", "user.email", "issue@example.com"],
            ]
            for command in commands:
                subprocess.run(
                    command,
                    cwd=repository,
                    check=True,
                    capture_output=True,
                    text=True,
                )
            (repository / "base.txt").write_text("base\n", encoding="utf-8")
            subprocess.run(
                ["git", "add", "base.txt"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            subprocess.run(
                ["git", "commit", "-m", "initial"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            self.configure_live_fetch_remote(
                repository,
                remote="origin",
                push_url="https://github.com/HiroyukiFuruno/katana-render-runtime.git",
            )
            subprocess.run(
                ["git", "update-ref", "refs/remotes/origin/master", "HEAD"],
                cwd=repository,
                check=True,
            )
            subprocess.run(
                [
                    "git",
                    "symbolic-ref",
                    "refs/remotes/origin/HEAD",
                    "refs/remotes/origin/master",
                ],
                cwd=repository,
                check=True,
            )
            subprocess.run(
                ["git", "switch", "-c", "feature/issue-contract"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            (repository / "feature.txt").write_text("feature\n", encoding="utf-8")
            subprocess.run(
                ["git", "add", "feature.txt"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            subprocess.run(
                ["git", "commit", "-m", "feat: contract", "-m", "Refs #64"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            fake_gh = binary_directory / "gh"
            fake_gh.write_text(
                "#!/bin/sh\n"
                "printf '%s\\n' "
                "'{\"number\":64,\"state\":\"OPEN\",\"body\":\"Issue body\","
                "\"url\":\"https://github.com/HiroyukiFuruno/katana-render-runtime/issues/64\"}'\n",
                encoding="utf-8",
            )
            fake_gh.chmod(fake_gh.stat().st_mode | stat.S_IXUSR)
            self.install_fake_push_remote_git(binary_directory)
            environment = os.environ.copy()
            environment["PATH"] = f"{binary_directory}:{environment['PATH']}"
            environment["KRR_TEST_REAL_GIT"] = str(shutil.which("git"))
            environment["KRR_TEST_PUSH_REMOTE_REF"] = "refs/remotes/origin/master"
            result = subprocess.run(
                [sys.executable, str(Path(subject.__file__))],
                cwd=repository,
                env=environment,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("issues=#64", result.stdout)

    def test_cli_validates_the_pushed_topic_while_master_is_checked_out(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory)
            binary_directory = repository / "bin"
            binary_directory.mkdir()
            commands = [
                ["git", "init", "--initial-branch=master"],
                ["git", "config", "user.name", "Issue Contract Test"],
                ["git", "config", "user.email", "issue@example.com"],
            ]
            for command in commands:
                subprocess.run(
                    command,
                    cwd=repository,
                    check=True,
                    capture_output=True,
                    text=True,
                )
            (repository / "base.txt").write_text("base\n", encoding="utf-8")
            subprocess.run(
                ["git", "add", "base.txt"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            subprocess.run(
                ["git", "commit", "-m", "initial"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            self.configure_live_fetch_remote(
                repository,
                remote="origin",
                push_url="https://github.com/HiroyukiFuruno/katana-render-runtime.git",
            )
            subprocess.run(
                ["git", "update-ref", "refs/remotes/origin/master", "HEAD"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            subprocess.run(
                [
                    "git",
                    "symbolic-ref",
                    "refs/remotes/origin/HEAD",
                    "refs/remotes/origin/master",
                ],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            subprocess.run(
                ["git", "switch", "-c", "topic"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            (repository / "topic.txt").write_text("topic\n", encoding="utf-8")
            subprocess.run(
                ["git", "add", "topic.txt"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            subprocess.run(
                ["git", "commit", "-m", "feat: missing Issue reference"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            topic_sha = subprocess.run(
                ["git", "rev-parse", "HEAD"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
            subprocess.run(
                ["git", "switch", "master"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            push_update = (
                f"refs/heads/topic {topic_sha} refs/heads/topic {'0' * 40}\n"
            )
            self.install_fake_push_remote_git(binary_directory)
            environment = os.environ.copy()
            environment["PATH"] = f"{binary_directory}:{environment['PATH']}"
            environment["KRR_TEST_REAL_GIT"] = str(shutil.which("git"))
            environment["KRR_TEST_PUSH_REMOTE_REF"] = "refs/remotes/origin/master"
            result = subprocess.run(
                [sys.executable, str(Path(subject.__file__))],
                cwd=repository,
                env=environment,
                input=push_update,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(result.returncode, 1)
            self.assertIn("Issue参照", result.stderr)

    def test_cli_validates_push_update_from_detached_head(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repository = root / "repository"
            binary_directory = root / "bin"
            repository.mkdir()
            binary_directory.mkdir()

            def git(*arguments: str) -> str:
                result = subprocess.run(
                    ["git", *arguments],
                    cwd=repository,
                    check=True,
                    capture_output=True,
                    text=True,
                )
                return result.stdout.strip()

            git("init", "--initial-branch=master")
            git("config", "user.name", "Issue Contract Test")
            git("config", "user.email", "issue@example.com")
            (repository / "base.txt").write_text("base\n", encoding="utf-8")
            git("add", "base.txt")
            git("commit", "-m", "initial")
            base_sha = git("rev-parse", "HEAD")
            self.configure_live_fetch_remote(
                repository,
                remote="origin",
                push_url="https://github.com/HiroyukiFuruno/katana-render-runtime.git",
            )
            git("update-ref", "refs/remotes/origin/master", base_sha)
            git("symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/master")
            git("switch", "-c", "topic")
            (repository / "topic.txt").write_text("topic\n", encoding="utf-8")
            git("add", "topic.txt")
            git("commit", "-m", "feat: detached push", "-m", "Refs #64")
            topic_sha = git("rev-parse", "HEAD")
            git("switch", "--detach", base_sha)
            fake_gh = binary_directory / "gh"
            fake_gh.write_text(
                "#!/bin/sh\n"
                "printf '%s\\n' "
                "'{\"number\":64,\"state\":\"OPEN\",\"body\":\"Issue body\","
                "\"url\":\"https://github.com/HiroyukiFuruno/katana-render-runtime/issues/64\"}'\n",
                encoding="utf-8",
            )
            fake_gh.chmod(fake_gh.stat().st_mode | stat.S_IXUSR)
            self.install_fake_push_remote_git(binary_directory)
            environment = os.environ.copy()
            environment["PATH"] = f"{binary_directory}:{environment['PATH']}"
            environment["KRR_TEST_REAL_GIT"] = str(shutil.which("git"))
            environment["KRR_TEST_PUSH_REMOTE_REF"] = "refs/remotes/origin/master"
            result = subprocess.run(
                [sys.executable, str(Path(subject.__file__))],
                cwd=repository,
                env=environment,
                input=f"refs/heads/topic {topic_sha} refs/heads/topic {'0' * 40}\n",
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("issues=#64", result.stdout)

    def test_cli_accepts_remote_name_and_url_as_pre_push_arguments(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repository = root / "repository"
            binary_directory = root / "bin"
            repository.mkdir()
            binary_directory.mkdir()

            def git(*arguments: str) -> str:
                result = subprocess.run(
                    ["git", *arguments],
                    cwd=repository,
                    check=True,
                    capture_output=True,
                    text=True,
                )
                return result.stdout.strip()

            git("init", "--initial-branch=master")
            git("config", "user.name", "Issue Contract Test")
            git("config", "user.email", "issue@example.com")
            (repository / "base.txt").write_text("base\n", encoding="utf-8")
            git("add", "base.txt")
            git("commit", "-m", "initial")
            base_sha = git("rev-parse", "HEAD")
            remote_url = "https://github.com/HiroyukiFuruno/katana-render-runtime.git"
            self.configure_live_fetch_remote(
                repository,
                remote="origin",
                push_url=remote_url,
            )
            git("update-ref", "refs/remotes/origin/master", base_sha)
            git("symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/master")
            git("switch", "-c", "topic")
            (repository / "topic.txt").write_text("topic\n", encoding="utf-8")
            git("add", "topic.txt")
            git("commit", "-m", "feat: url push", "-m", "Refs #64")
            topic_sha = git("rev-parse", "HEAD")
            fake_gh = binary_directory / "gh"
            fake_gh.write_text(
                "#!/bin/sh\n"
                "printf '%s\\n' "
                "'{\"number\":64,\"state\":\"OPEN\",\"body\":\"Issue body\","
                "\"url\":\"https://github.com/HiroyukiFuruno/katana-render-runtime/issues/64\"}'\n",
                encoding="utf-8",
            )
            fake_gh.chmod(fake_gh.stat().st_mode | stat.S_IXUSR)
            self.install_fake_push_remote_git(binary_directory)
            environment = os.environ.copy()
            environment["PATH"] = f"{binary_directory}:{environment['PATH']}"
            environment["KRR_TEST_REAL_GIT"] = str(shutil.which("git"))
            environment["KRR_TEST_PUSH_REMOTE_REF"] = "refs/remotes/origin/master"
            result = subprocess.run(
                [
                    sys.executable,
                    str(Path(subject.__file__)),
                    "--remote",
                    remote_url,
                    "--remote-url",
                    remote_url,
                ],
                cwd=repository,
                env=environment,
                input=f"refs/heads/topic {topic_sha} refs/heads/topic {'0' * 40}\n",
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("issues=#64", result.stdout)

    def test_cli_skips_tag_only_push_while_topic_is_checked_out(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory)
            binary_directory = repository / "bin"
            binary_directory.mkdir()
            commands = [
                ["git", "init", "--initial-branch=master"],
                ["git", "config", "user.name", "Issue Contract Test"],
                ["git", "config", "user.email", "issue@example.com"],
            ]
            for command in commands:
                subprocess.run(
                    command,
                    cwd=repository,
                    check=True,
                    capture_output=True,
                    text=True,
                )
            (repository / "base.txt").write_text("base\n", encoding="utf-8")
            subprocess.run(
                ["git", "add", "base.txt"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            subprocess.run(
                ["git", "commit", "-m", "initial"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            self.configure_live_fetch_remote(
                repository,
                remote="origin",
                push_url="https://github.com/HiroyukiFuruno/katana-render-runtime.git",
            )
            subprocess.run(
                ["git", "update-ref", "refs/remotes/origin/master", "HEAD"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            subprocess.run(
                [
                    "git",
                    "symbolic-ref",
                    "refs/remotes/origin/HEAD",
                    "refs/remotes/origin/master",
                ],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            subprocess.run(
                ["git", "switch", "-c", "topic"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            (repository / "topic.txt").write_text("topic\n", encoding="utf-8")
            subprocess.run(
                ["git", "add", "topic.txt"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            subprocess.run(
                ["git", "commit", "-m", "feat: missing Issue reference"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            topic_sha = subprocess.run(
                ["git", "rev-parse", "HEAD"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
            self.install_fake_push_remote_git(binary_directory)
            environment = os.environ.copy()
            environment["PATH"] = f"{binary_directory}:{environment['PATH']}"
            environment["KRR_TEST_REAL_GIT"] = str(shutil.which("git"))
            environment["KRR_TEST_PUSH_REMOTE_REF"] = "refs/remotes/origin/master"
            result = subprocess.run(
                [sys.executable, str(Path(subject.__file__))],
                cwd=repository,
                env=environment,
                input=(
                    f"refs/tags/v0.0.0 {topic_sha} "
                    f"refs/tags/v0.0.0 {'0' * 40}\n"
                ),
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("Issue contract skipped", result.stdout)


if __name__ == "__main__":
    unittest.main()
