from __future__ import annotations

import io
import base64
import hashlib
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
    _GIT_FIXTURE_ENVIRONMENT = (
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_CEILING_DIRECTORIES",
        "GIT_DISCOVERY_ACROSS_FILESYSTEM",
        "GIT_PREFIX",
    )

    def setUp(self) -> None:
        """Keep temporary Git fixtures outside a caller's hook repository."""
        saved = {
            name: os.environ[name]
            for name in self._GIT_FIXTURE_ENVIRONMENT
            if name in os.environ
        }
        for name in self._GIT_FIXTURE_ENVIRONMENT:
            os.environ.pop(name, None)

        def restore() -> None:
            for name in self._GIT_FIXTURE_ENVIRONMENT:
                os.environ.pop(name, None)
            os.environ.update(saved)

        self.addCleanup(restore)

    def test_temporary_git_fixtures_do_not_inherit_hook_repository_state(self) -> None:
        self.assertTrue(
            all(name not in os.environ for name in self._GIT_FIXTURE_ENVIRONMENT)
        )

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
        self.assertEqual(subject.issue_numbers("changelog: avoid #64", repository), set())
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

    def test_fast_forward_push_checks_only_the_new_remote_to_local_range(self) -> None:
        repository = Path("/tmp/verify-push-range")
        remote_sha = "a" * 40
        local_sha = "b" * 40
        with patch.object(subject, "_is_ancestor", side_effect=(True, False)) as ancestor:
            self.assertEqual(
                subject.push_validation_range_base(
                    repository,
                    remote_sha=remote_sha,
                    local_sha=local_sha,
                    default_range_base="origin/master",
                ),
                remote_sha,
            )
        self.assertEqual(
            ancestor.call_args_list,
            [
                ((repository, remote_sha, local_sha), {}),
                ((repository, "origin/master", local_sha), {}),
            ],
        )

    def test_fast_forward_push_after_merging_live_default_excludes_default_commits(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory)

            def git(*arguments: str) -> str:
                return subprocess.run(
                    ["git", *arguments],
                    cwd=repository,
                    check=True,
                    capture_output=True,
                    text=True,
                ).stdout.strip()

            git("init", "--initial-branch=master")
            git("config", "user.name", "Issue Contract Test")
            git("config", "user.email", "issue@example.com")
            (repository / "base.txt").write_text("base\n", encoding="utf-8")
            git("add", "base.txt")
            git("commit", "-m", "initial")
            git("switch", "-c", "feature/default-merge")
            (repository / "feature.txt").write_text("feature\n", encoding="utf-8")
            git("add", "feature.txt")
            git("commit", "-m", "feat: feature", "-m", "Refs #64")
            remote_sha = git("rev-parse", "HEAD")
            git("switch", "master")
            (repository / "default.txt").write_text("default\n", encoding="utf-8")
            git("add", "default.txt")
            git("commit", "-m", "chore: live default advance")
            default_sha = git("rev-parse", "HEAD")
            git("switch", "feature/default-merge")
            git("merge", "--no-ff", "master", "-m", "merge live default\n\nRefs #64")
            local_sha = git("rev-parse", "HEAD")

            range_base = subject.push_validation_range_base(
                repository,
                remote_sha=remote_sha,
                local_sha=local_sha,
                default_range_base=default_sha,
            )
            self.assertEqual(range_base, default_sha)
            messages = subject.parse_commit_messages(
                subject._run_git(
                    repository, "log", "--reverse", "-z", "--format=%B", f"{range_base}..{local_sha}"
                )
            )
            self.assertNotIn("chore: live default advance", messages)
            subject.validate_contract(
                branch="feature/default-merge",
                default_branch="master",
                repository="HiroyukiFuruno/katana-render-runtime",
                commit_messages=messages,
                changed_paths=["feature.txt"],
                issue_loader=lambda number: self.issue(number) if number == 64 else None,
            )

    def test_non_fast_forward_push_falls_back_to_live_default_range(self) -> None:
        with patch.object(subject, "_is_ancestor", return_value=False):
            self.assertEqual(
                subject.push_validation_range_base(
                    Path("/tmp/verify-push-range"),
                    remote_sha="a" * 40,
                    local_sha="b" * 40,
                    default_range_base="origin/master",
                ),
                "origin/master",
            )

    def test_nonatomic_combined_push_uses_remote_default_as_feature_base(self) -> None:
        """A rejected default update must not hide unreferenced local commits."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repository = root / "repository"
            binary_directory = root / "bin"
            repository.mkdir()
            binary_directory.mkdir()
            for command in (
                ["git", "init", "--initial-branch=master"],
                ["git", "config", "user.name", "Issue Contract Test"],
                ["git", "config", "user.email", "issue@example.com"],
            ):
                subprocess.run(command, cwd=repository, check=True, capture_output=True, text=True)
            (repository / "base.txt").write_text("base\n", encoding="utf-8")
            subprocess.run(["git", "add", "base.txt"], cwd=repository, check=True, capture_output=True, text=True)
            subprocess.run(["git", "commit", "-m", "initial"], cwd=repository, check=True, capture_output=True, text=True)
            remote_url = "https://github.com/HiroyukiFuruno/katana-render-runtime.git"
            self.configure_live_fetch_remote(repository, remote="origin", push_url=remote_url)
            subprocess.run(["git", "update-ref", "refs/remotes/origin/master", "HEAD"], cwd=repository, check=True)
            subprocess.run(["git", "symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/master"], cwd=repository, check=True)
            # This commit exists only in the local default branch.  If the
            # hook uses that local tip as base, the feature range omits it.
            (repository / "local-only.txt").write_text("must be checked\n", encoding="utf-8")
            subprocess.run(["git", "add", "local-only.txt"], cwd=repository, check=True, capture_output=True, text=True)
            subprocess.run(["git", "commit", "-m", "local default change"], cwd=repository, check=True, capture_output=True, text=True)
            subprocess.run(["git", "switch", "-c", "feature/non-atomic"], cwd=repository, check=True, capture_output=True, text=True)
            (repository / "feature.txt").write_text("feature\n", encoding="utf-8")
            subprocess.run(["git", "add", "feature.txt"], cwd=repository, check=True, capture_output=True, text=True)
            subprocess.run(["git", "commit", "-m", "feat: feature", "-m", "Refs #64"], cwd=repository, check=True, capture_output=True, text=True)
            feature_sha = subprocess.run(["git", "rev-parse", "HEAD"], cwd=repository, check=True, capture_output=True, text=True).stdout.strip()
            local_default_sha = subprocess.run(["git", "rev-parse", "master"], cwd=repository, check=True, capture_output=True, text=True).stdout.strip()
            fake_gh = binary_directory / "gh"
            fake_gh.write_text(
                "#!/bin/sh\n"
                "printf '%s\\n' '{\"number\":64,\"state\":\"OPEN\",\"body\":\"Issue body\",\"url\":\"https://github.com/HiroyukiFuruno/katana-render-runtime/issues/64\"}'\n",
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
                input=(
                    f"refs/heads/master {local_default_sha} refs/heads/master {'0' * 40}\n"
                    f"refs/heads/feature/non-atomic {feature_sha} refs/heads/feature/non-atomic {'0' * 40}\n"
                ),
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(result.returncode, 1)
            self.assertIn("commit 1に対象repositoryのIssue参照がありません", result.stderr)

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

    def test_release_branch_accepts_the_complete_issue_set_without_per_commit_refs(self) -> None:
        issues = {64: self.issue(64), 65: self.issue(65)}
        subject.validate_contract(
            branch="release/v0.4.22",
            default_branch="master",
            repository="HiroyukiFuruno/katana-render-runtime",
            commit_messages=[
                "release: prepare v0.4.22\n\nRefs #64 #65",
                "fix: apply release review feedback",
            ],
            changed_paths=["scripts/hooks/pre-push.sh"],
            issue_loader=issues.get,
        )

    def test_release_branch_requires_an_issue_somewhere_in_its_range(self) -> None:
        with self.assertRaisesRegex(subject.ContractViolation, "commit範囲"):
            self.validate(
                branch="release/v0.4.22",
                messages=["release: v0.4.22", "fix: review feedback"],
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

    def test_release_branch_rejects_closed_issue(self) -> None:
        with self.assertRaisesRegex(subject.ContractViolation, "OPEN"):
            self.validate(
                branch="release/v0.4.21",
                changed_paths=["Cargo.lock"],
                issue=self.issue(state="CLOSED"),
            )

    def test_closed_release_reference_is_rejected_when_its_commit_is_active(self) -> None:
        closed = self.issue(64, state="CLOSED")
        with self.assertRaisesRegex(subject.ContractViolation, "重複"):
            subject._effective_release_issue_numbers(
                repository="HiroyukiFuruno/katana-render-runtime",
                references_by_commit=[("a" * 40, {64})],
                net_paths=["src/active.rs"],
                issue_loader=lambda _number: closed,
                commit_paths=lambda _sha: ["src/active.rs"],
                commit_surviving_paths=lambda _sha, paths: paths,
            )

    def test_genuinely_obsolete_closed_release_reference_is_accepted(self) -> None:
        closed = self.issue(64, state="CLOSED")
        result = subject._effective_release_issue_numbers(
            repository="HiroyukiFuruno/katana-render-runtime",
            references_by_commit=[("a" * 40, {64})],
            net_paths=["src/current.rs"],
            issue_loader=lambda _number: closed,
            commit_paths=lambda _sha: ["old/removed-workflow.yml"],
            commit_surviving_paths=lambda _sha, _paths: [],
        )
        self.assertEqual(result, set())

    def test_local_commit_path_proof_uses_real_git_history(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory)
            subprocess.run(["git", "init", "-q"], cwd=repository, check=True)
            subprocess.run(["git", "config", "user.email", "test@example.com"], cwd=repository, check=True)
            subprocess.run(["git", "config", "user.name", "Hook test"], cwd=repository, check=True)
            old_path = repository / "old" / "obsolete.yml"
            old_path.parent.mkdir()
            old_path.write_text("baseline\n", encoding="utf-8")
            subprocess.run(["git", "add", "."], cwd=repository, check=True)
            subprocess.run(["git", "commit", "-qm", "baseline"], cwd=repository, check=True)
            old_path.write_text("obsolete change\n", encoding="utf-8")
            subprocess.run(["git", "add", "."], cwd=repository, check=True)
            subprocess.run(["git", "commit", "-qm", "old governance\n\nRefs #64"], cwd=repository, check=True)
            commit = subject._run_git(repository, "rev-parse", "HEAD")
            self.assertEqual(subject._local_commit_paths(repository, commit), ["old/obsolete.yml"])
            (repository / "src").mkdir()
            (repository / "src" / "active.rs").write_text("active\n", encoding="utf-8")
            old_path.write_text("baseline\n", encoding="utf-8")
            subprocess.run(["git", "add", "."], cwd=repository, check=True)
            subprocess.run(["git", "commit", "-qm", "active change\n\nRefs #65"], cwd=repository, check=True)
            net_paths = subject.parse_name_status_paths(
                subject._run_git(repository, "diff", "--name-status", "-z", "--find-renames", "HEAD~2...HEAD")
            )
            active_commit = subject._run_git(repository, "rev-parse", "HEAD")
            edge_cache: dict[tuple[str, str], tuple[tuple[str, tuple[str, ...]], ...]] = {}
            result = subject._effective_release_issue_numbers(
                repository="HiroyukiFuruno/katana-render-runtime",
                references_by_commit=[(commit, {64}), (active_commit, {65})],
                net_paths=net_paths,
                issue_loader=lambda number: self.issue(number, state="CLOSED" if number == 64 else "OPEN"),
                commit_paths=lambda sha: subject._local_commit_paths(repository, sha),
                commit_surviving_paths=lambda sha, paths: subject._local_surviving_paths_at_head(
                    repository, sha, active_commit, paths, edge_cache
                ),
            )
            self.assertEqual(result, {65})

    def test_closed_reference_rejects_issue_content_renamed_to_current_net_path(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory)
            subprocess.run(["git", "init", "-q"], cwd=repository, check=True)
            subprocess.run(["git", "config", "user.email", "test@example.com"], cwd=repository, check=True)
            subprocess.run(["git", "config", "user.name", "Hook test"], cwd=repository, check=True)
            (repository / "README.md").write_text("baseline\n", encoding="utf-8")
            subprocess.run(["git", "add", "."], cwd=repository, check=True)
            subprocess.run(["git", "commit", "-qm", "baseline"], cwd=repository, check=True)
            baseline = subject._run_git(repository, "rev-parse", "HEAD")
            old_path = repository / "old" / "name.rs"
            old_path.parent.mkdir()
            old_path.write_text("surviving issue content\n", encoding="utf-8")
            subprocess.run(["git", "add", "."], cwd=repository, check=True)
            subprocess.run(["git", "commit", "-qm", "add old source\n\nRefs #64"], cwd=repository, check=True)
            issue_commit = subject._run_git(repository, "rev-parse", "HEAD")
            new_path = repository / "new" / "name.rs"
            new_path.parent.mkdir()
            old_path.rename(new_path)
            subprocess.run(["git", "add", "-A"], cwd=repository, check=True)
            subprocess.run(["git", "commit", "-qm", "move source"], cwd=repository, check=True)
            new_path.write_text("surviving issue content\n" + ("edited line\n" * 30), encoding="utf-8")
            subprocess.run(["git", "add", "."], cwd=repository, check=True)
            subprocess.run(["git", "commit", "-qm", "edit moved source"], cwd=repository, check=True)
            head = subject._run_git(repository, "rev-parse", "HEAD")
            net_paths = subject.parse_name_status_paths(
                subject._run_git(
                    repository,
                    "diff",
                    "--name-status",
                    "-z",
                    "--find-renames",
                    baseline,
                    head,
                )
            )
            self.assertEqual(net_paths, ["new/name.rs"])
            self.assertEqual(subject._local_commit_paths(repository, issue_commit), ["old/name.rs"])
            edge_cache: dict[tuple[str, str], tuple[tuple[str, tuple[str, ...]], ...]] = {}
            surviving_paths = subject._local_surviving_paths_at_head(
                repository, issue_commit, head, ["old/name.rs"], edge_cache
            )
            self.assertEqual(surviving_paths, ["new/name.rs"])
            with self.assertRaisesRegex(subject.ContractViolation, "rename先"):
                subject._effective_release_issue_numbers(
                    repository="HiroyukiFuruno/katana-render-runtime",
                    references_by_commit=[(issue_commit, {64})],
                    net_paths=net_paths,
                    issue_loader=lambda number: self.issue(number, state="CLOSED"),
                    commit_paths=lambda sha: subject._local_commit_paths(repository, sha),
                    commit_surviving_paths=lambda sha, paths: subject._local_surviving_paths_at_head(
                        repository, sha, head, paths, edge_cache
                    ),
                )

    def test_local_second_parent_rename_survives_merge_and_remains_active(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory)
            subprocess.run(["git", "init", "-q", "--initial-branch=main"], cwd=repository, check=True)
            subprocess.run(["git", "config", "user.email", "test@example.com"], cwd=repository, check=True)
            subprocess.run(["git", "config", "user.name", "Hook test"], cwd=repository, check=True)
            (repository / "README.md").write_text("baseline\n", encoding="utf-8")
            subprocess.run(["git", "add", "."], cwd=repository, check=True)
            subprocess.run(["git", "commit", "-qm", "baseline"], cwd=repository, check=True)
            baseline = subject._run_git(repository, "rev-parse", "HEAD")
            old_path = repository / "old" / "name.rs"
            old_path.parent.mkdir()
            old_path.write_text("surviving issue content\n", encoding="utf-8")
            subprocess.run(["git", "add", "."], cwd=repository, check=True)
            subprocess.run(["git", "commit", "-qm", "add source\n\nRefs #64"], cwd=repository, check=True)
            issue_commit = subject._run_git(repository, "rev-parse", "HEAD")
            subprocess.run(["git", "switch", "-qc", "rename-side"], cwd=repository, check=True)
            new_path = repository / "new" / "name.rs"
            new_path.parent.mkdir()
            old_path.rename(new_path)
            subprocess.run(["git", "add", "-A"], cwd=repository, check=True)
            subprocess.run(["git", "commit", "-qm", "rename on second parent"], cwd=repository, check=True)
            rename_commit = subject._run_git(repository, "rev-parse", "HEAD")
            subprocess.run(["git", "switch", "main"], cwd=repository, check=True, capture_output=True)
            (repository / "README.md").write_text("mainline update\n", encoding="utf-8")
            subprocess.run(["git", "add", "README.md"], cwd=repository, check=True)
            subprocess.run(["git", "commit", "-qm", "mainline update"], cwd=repository, check=True)
            subprocess.run(
                ["git", "merge", "--no-ff", "-qm", "merge rename side", "rename-side"],
                cwd=repository,
                check=True,
            )
            head = subject._run_git(repository, "rev-parse", "HEAD")
            parents = subject._run_git(repository, "rev-list", "--parents", "-n", "1", head).split()
            self.assertEqual(len(parents), 3)
            self.assertEqual(parents[2], rename_commit)
            net_paths = subject.parse_name_status_paths(
                subject._run_git(repository, "diff", "--name-status", "-z", "--find-renames", baseline, head)
            )
            self.assertEqual(net_paths, ["README.md", "new/name.rs"])
            edge_cache: dict[tuple[str, str], tuple[tuple[str, tuple[str, ...]], ...]] = {}
            surviving = subject._local_surviving_paths_at_head(
                repository, issue_commit, head, ["old/name.rs"], edge_cache
            )
            self.assertEqual(surviving, ["new/name.rs"])
            self.assertNotEqual(issue_commit, rename_commit)

    def test_local_renamed_closed_reference_is_obsolete_after_surviving_path_deletion(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory)
            subprocess.run(["git", "init", "-q"], cwd=repository, check=True)
            subprocess.run(["git", "config", "user.email", "test@example.com"], cwd=repository, check=True)
            subprocess.run(["git", "config", "user.name", "Hook test"], cwd=repository, check=True)
            (repository / "README.md").write_text("baseline\n", encoding="utf-8")
            (repository / "src").mkdir()
            (repository / "src" / "active.rs").write_text("baseline active\n", encoding="utf-8")
            subprocess.run(["git", "add", "."], cwd=repository, check=True)
            subprocess.run(["git", "commit", "-qm", "baseline"], cwd=repository, check=True)
            baseline = subject._run_git(repository, "rev-parse", "HEAD")
            old_path = repository / "old" / "name.rs"
            old_path.parent.mkdir()
            old_path.write_text("obsolete governance content\n", encoding="utf-8")
            subprocess.run(["git", "add", "."], cwd=repository, check=True)
            subprocess.run(["git", "commit", "-qm", "add obsolete source\n\nRefs #64"], cwd=repository, check=True)
            issue_commit = subject._run_git(repository, "rev-parse", "HEAD")
            new_path = repository / "new" / "name.rs"
            new_path.parent.mkdir()
            old_path.rename(new_path)
            subprocess.run(["git", "add", "-A"], cwd=repository, check=True)
            subprocess.run(["git", "commit", "-qm", "rename source"], cwd=repository, check=True)
            new_path.unlink()
            (repository / "src" / "active.rs").write_text("current active\n", encoding="utf-8")
            subprocess.run(["git", "add", "-A"], cwd=repository, check=True)
            subprocess.run(["git", "commit", "-qm", "delete obsolete source\n\nRefs #65"], cwd=repository, check=True)
            head = subject._run_git(repository, "rev-parse", "HEAD")
            net_paths = subject.parse_name_status_paths(
                subject._run_git(
                    repository,
                    "diff",
                    "--name-status",
                    "-z",
                    "--find-renames",
                    baseline,
                    head,
                )
            )
            self.assertEqual(net_paths, ["src/active.rs"])
            edge_cache: dict[tuple[str, str], tuple[tuple[str, tuple[str, ...]], ...]] = {}
            result = subject._effective_release_issue_numbers(
                repository="HiroyukiFuruno/katana-render-runtime",
                references_by_commit=[(issue_commit, {64})],
                net_paths=net_paths,
                issue_loader=lambda number: self.issue(number, state="CLOSED"),
                commit_paths=lambda sha: subject._local_commit_paths(repository, sha),
                commit_surviving_paths=lambda sha, paths: subject._local_surviving_paths_at_head(
                    repository, sha, head, paths, edge_cache
                ),
            )
            self.assertEqual(result, set())

    def test_local_same_commit_heavy_move_rewrite_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory)
            subprocess.run(["git", "init", "-q"], cwd=repository, check=True)
            subprocess.run(["git", "config", "user.email", "test@example.com"], cwd=repository, check=True)
            subprocess.run(["git", "config", "user.name", "Hook test"], cwd=repository, check=True)
            (repository / "README.md").write_text("baseline\n", encoding="utf-8")
            subprocess.run(["git", "add", "."], cwd=repository, check=True)
            subprocess.run(["git", "commit", "-qm", "baseline"], cwd=repository, check=True)
            baseline = subject._run_git(repository, "rev-parse", "HEAD")
            old_path = repository / "old" / "name.rs"
            old_path.parent.mkdir()
            old_path.write_text("".join(f"old unique line {index}\n" for index in range(20)), encoding="utf-8")
            subprocess.run(["git", "add", "."], cwd=repository, check=True)
            subprocess.run(["git", "commit", "-qm", "add source\n\nRefs #64"], cwd=repository, check=True)
            issue_commit = subject._run_git(repository, "rev-parse", "HEAD")
            old_path.unlink()
            new_path = repository / "new" / "name.rs"
            new_path.parent.mkdir()
            new_path.write_text("".join(f"new unrelated rewrite {index}\n" for index in range(60)), encoding="utf-8")
            subprocess.run(["git", "add", "-A"], cwd=repository, check=True)
            subprocess.run(["git", "commit", "-qm", "move and rewrite in one commit"], cwd=repository, check=True)
            head = subject._run_git(repository, "rev-parse", "HEAD")
            net_paths = subject.parse_name_status_paths(
                subject._run_git(repository, "diff", "--name-status", "-z", "--find-renames", baseline, head)
            )
            self.assertEqual(net_paths, ["new/name.rs"])
            edge_cache: dict[tuple[str, str], tuple[tuple[str, tuple[str, ...]], ...]] = {}
            self.assertEqual(
                set(subject._local_path_edge_records(repository, issue_commit, head, edge_cache)),
                {("D", ("old/name.rs",)), ("A", ("new/name.rs",))},
            )
            with self.assertRaisesRegex(subject.ContractViolation, "削除と同一edgeの追加"):
                subject._effective_release_issue_numbers(
                    repository="HiroyukiFuruno/katana-render-runtime",
                    references_by_commit=[(issue_commit, {64})],
                    net_paths=net_paths,
                    issue_loader=lambda number: self.issue(number, state="CLOSED"),
                    commit_paths=lambda sha: subject._local_commit_paths(repository, sha),
                    commit_surviving_paths=lambda sha, paths: subject._local_surviving_paths_at_head(
                        repository, sha, head, paths, edge_cache
                    ),
                )

    @staticmethod
    def remote_git_history(repository: Path, calls: list[str]):
        """実Gitのimmutable objectをGitHubのcommit/tree/compare形式で返す。"""
        def gh_json(endpoint: str) -> object:
            calls.append(endpoint)
            if "/git/commits/" in endpoint:
                sha = endpoint.rsplit("/", 1)[1]
                parents = subject._run_git(repository, "rev-list", "--parents", "-n", "1", sha).split()[1:]
                tree = subject._run_git(repository, "rev-parse", f"{sha}^{{tree}}")
                return {"sha": sha, "parents": [{"sha": parent} for parent in parents], "tree": {"sha": tree}}
            if "/git/trees/" in endpoint:
                tree = endpoint.rsplit("/", 1)[1].split("?", 1)[0]
                entries = []
                for record in subject._run_git(repository, "ls-tree", "-rz", tree).split("\x00"):
                    if record:
                        metadata, path = record.split("\t", 1)
                        mode, entry_type, sha = metadata.split()
                        entries.append({"path": path, "mode": mode, "type": entry_type, "sha": sha})
                return {"sha": tree, "truncated": False, "tree": entries}
            if "/git/blobs/" in endpoint:
                sha = endpoint.rsplit("/", 1)[1]
                data = subprocess.check_output(["git", "cat-file", "blob", sha], cwd=repository)
                return {"sha": sha, "size": len(data), "encoding": "base64", "content": base64.b64encode(data).decode()}
            if "/compare/" in endpoint:
                base, head = endpoint.rsplit("/", 1)[1].split("...")
                commits = subject._run_git(repository, "rev-list", "--reverse", "--topo-order", f"{base}..{head}").splitlines()
                files = []
                for kind, paths in subject._local_path_edge_records(repository, base, head, {}):
                    status = {"A": "added", "C": "added", "D": "removed", "M": "modified", "R": "renamed"}[kind]
                    entry = {"filename": paths[-1], "status": status}
                    if kind == "R":
                        entry["previous_filename"] = paths[0]
                    files.append(entry)
                return {"base_commit": {"sha": base}, "total_commits": len(commits), "ahead_by": len(commits), "commits": [{"sha": sha} for sha in commits], "files": files}
            raise AssertionError(f"unexpected GitHub API request: {endpoint}")
        return gh_json

    def test_remote_added_copy_then_source_deletion_retains_closed_provenance(self) -> None:
        self.assert_added_copy_then_source_deletion_retains_closed_provenance(edited_copy=False)

    def test_remote_edited_copy_then_source_deletion_matches_native_git_provenance(self) -> None:
        self.assert_added_copy_then_source_deletion_retains_closed_provenance(edited_copy=True)

    def assert_added_copy_then_source_deletion_retains_closed_provenance(self, *, edited_copy: bool) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory)
            def git(*arguments: str) -> str:
                return subject._run_git(repository, *arguments)
            git("init", "-q", "--initial-branch=main")
            git("config", "user.email", "test@example.com")
            git("config", "user.name", "Hook test")
            original = "".join(f"closed issue implementation line {index}\n" for index in range(100))
            (repository / "old.rs").write_text(original, encoding="utf-8")
            git("add", ".")
            git("commit", "-qm", "source")
            source = git("rev-parse", "HEAD")
            shutil.copyfile(repository / "old.rs", repository / "copied.rs")
            (repository / "copied.rs").chmod(0o755)
            if edited_copy:
                (repository / "copied.rs").write_text(original.replace("line 99\n", "edited last line\n"), encoding="utf-8")
            for index in range(299):
                (repository / f"asset-{index}.txt").write_text(f"independent asset {index}\n", encoding="utf-8")
            git("add", ".")
            git("commit", "-qm", "copy")
            copied = git("rev-parse", "HEAD")
            self.assertIn(("C", ("old.rs", "copied.rs")), subject._local_path_edge_records(repository, source, copied, {}))
            git("rm", "old.rs")
            git("commit", "-qm", "delete original")
            head = git("rev-parse", "HEAD")
            local = subject._local_surviving_paths_at_head(repository, source, head, ["old.rs"], {})
            calls: list[str] = []
            with patch.object(subject, "_gh_json", side_effect=self.remote_git_history(repository, calls)):
                remote = subject._pr_surviving_paths_at_head("owner/repo", source, head, ["old.rs"], {})
            self.assertEqual(local, ["copied.rs"])
            self.assertEqual(remote, local)
            with self.assertRaisesRegex(subject.ContractViolation, "rename先"):
                subject._effective_release_issue_numbers(
                    repository="HiroyukiFuruno/katana-render-runtime",
                    references_by_commit=[(source, {64})], net_paths=["copied.rs"],
                    issue_loader=lambda number: self.issue(number, state="CLOSED"),
                    commit_paths=lambda _sha: ["old.rs"],
                    commit_surviving_paths=lambda _sha, _paths: remote,
                )

    def test_merge_independent_addition_and_retired_paths_have_local_remote_parity(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory)
            def git(*arguments: str) -> str:
                return subject._run_git(repository, *arguments)
            git("init", "-q", "--initial-branch=main")
            git("config", "user.email", "test@example.com")
            git("config", "user.name", "Hook test")
            (repository / "old.rs").write_text("retired closed issue implementation\n", encoding="utf-8")
            (repository / "README.md").write_text("baseline\n", encoding="utf-8")
            git("add", ".")
            git("commit", "-qm", "source")
            source = git("rev-parse", "HEAD")
            git("switch", "-qc", "retired")
            git("rm", "old.rs")
            git("commit", "-qm", "retire source")
            retired = git("rev-parse", "HEAD")
            (repository / "drawio.br").write_text("unrelated compressed drawio asset\n", encoding="utf-8")
            for index in range(300):
                (repository / f"drawio-asset-{index}.txt").write_text(f"drawio asset {index}\n", encoding="utf-8")
            git("add", ".")
            git("commit", "-qm", "add independent asset")
            added = git("rev-parse", "HEAD")
            git("switch", "main")
            (repository / "README.md").write_text("mainline update\n", encoding="utf-8")
            git("add", ".")
            git("commit", "-qm", "mainline")
            mainline = git("rev-parse", "HEAD")
            git("merge", "--no-ff", "-qm", "merge independent asset and retirement", "retired")
            head = git("rev-parse", "HEAD")
            records = subject._local_path_edge_records(repository, mainline, head, {})
            self.assertIn(("D", ("old.rs",)), records)
            self.assertIn(("A", ("drawio.br",)), records)
            self.assertGreaterEqual(len(records), 300)
            local_edges = {}
            local = subject._local_surviving_paths_at_head(repository, source, head, ["old.rs"], local_edges)
            self.assertEqual(local, [])
            self.assertNotIn((retired, added), local_edges)
            remote_edges = {}
            calls: list[str] = []
            with patch.object(subject, "_gh_json", side_effect=self.remote_git_history(repository, calls)):
                remote = subject._pr_surviving_paths_at_head("owner/repo", source, head, ["old.rs"], remote_edges)
            self.assertEqual(remote, local)
            self.assertNotIn((retired, added), remote_edges)
            self.assertIn(f"repos/owner/repo/git/commits/{added}", calls)
            tree_calls = [endpoint for endpoint in calls if "/git/trees/" in endpoint]
            self.assertEqual(len(tree_calls), len(set(tree_calls)))

    def test_merge_new_addition_without_other_parent_origin_stays_ambiguous(self) -> None:
        source_entry = ("blob", "100644", "a" * 40)
        new_entry = ("blob", "100644", "b" * 40)
        trees = {"parent": {"old.rs": source_entry}, "other": {}, "merge": {"new.rs": new_entry}}
        records = (("D", ("old.rs",)), ("A", ("new.rs",)))
        proven = subject._prove_path_additions(
            ["old.rs"], records, "parent", "merge", ["parent", "other"],
            {"parent": {"old.rs"}, "other": set()}, trees.__getitem__,
        )
        with self.assertRaisesRegex(subject.ContractViolation, "削除と同一edgeの追加"):
            subject._advance_path_state(["old.rs"], proven)

    def test_merge_other_parent_tracked_copy_is_not_independent_addition(self) -> None:
        source_entry = ("blob", "100644", "a" * 40)
        copied_entry = ("blob", "100644", "b" * 40)
        trees = {
            "parent": {"old.rs": source_entry},
            "other": {"copied.rs": copied_entry},
            "merge": {"copied.rs": copied_entry},
        }
        records = (("D", ("old.rs",)), ("A", ("copied.rs",)))
        proven = subject._prove_path_additions(
            ["old.rs"], records, "parent", "merge", ["parent", "other"],
            {"parent": {"old.rs"}, "other": {"copied.rs"}}, trees.__getitem__,
        )
        with self.assertRaisesRegex(subject.ContractViolation, "削除と同一edgeの追加"):
            subject._advance_path_state(["old.rs"], proven)

    def test_deleted_state_skips_large_edge_after_full_commit_dag_validation(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory)
            def git(*arguments: str) -> str:
                return subject._run_git(repository, *arguments)
            git("init", "-q", "--initial-branch=main")
            git("config", "user.email", "test@example.com")
            git("config", "user.name", "Hook test")
            (repository / "old.rs").write_text("obsolete implementation\n", encoding="utf-8")
            git("add", ".")
            git("commit", "-qm", "source")
            source = git("rev-parse", "HEAD")
            git("rm", "old.rs")
            git("commit", "-qm", "retire source")
            retired = git("rev-parse", "HEAD")
            for index in range(300):
                (repository / f"unrelated-{index}.txt").write_text(f"unrelated asset {index}\n", encoding="utf-8")
            git("add", ".")
            git("commit", "-qm", "large unrelated change")
            head = git("rev-parse", "HEAD")
            calls: list[str] = []
            with patch.object(subject, "_gh_json", side_effect=self.remote_git_history(repository, calls)):
                remote = subject._pr_surviving_paths_at_head("owner/repo", source, head, ["old.rs"], {})
            self.assertEqual(remote, [])
            self.assertEqual(subject._local_surviving_paths_at_head(repository, source, head, ["old.rs"], {}), remote)
            self.assertIn(f"repos/owner/repo/git/commits/{head}", calls)
            self.assertNotIn(f"repos/owner/repo/compare/{retired}...{head}", calls)

    def test_tree_proof_rejects_wrong_identity_and_malformed_entries(self) -> None:
        commit_sha, tree_sha, blob_sha = (character * 40 for character in "abc")
        entry = {"path": "source.rs", "type": "blob", "mode": "100644", "sha": blob_sha}
        valid = {"sha": tree_sha, "truncated": False, "tree": [entry]}
        invalid = [
            {**valid, "sha": "d" * 40},
            {**valid, "truncated": True},
            {**valid, "tree": [entry, entry]},
            {**valid, "tree": [{**entry, "mode": "040000"}]},
            {**valid, "tree": [{**entry, "type": "unknown"}]},
            {**valid, "tree": [{**entry, "path": "../source.rs"}]},
            {**valid, "tree": [{**entry, "sha": "invalid"}]},
        ]
        for payload in invalid:
            with self.subTest(payload=payload):
                with patch.object(subject, "_gh_json", return_value=payload):
                    with self.assertRaises(subject.ContractViolation):
                        subject._git_tree_entries(
                            repository="owner/repo", commit_sha=commit_sha,
                            label="closed Issue provenance",
                            commit_payload={"sha": commit_sha, "tree": {"sha": tree_sha}},
                        )

    def test_blob_proof_validates_identity_encoding_content_and_reuses_cache(self) -> None:
        data = b"immutable source\n"
        sha = hashlib.sha1(f"blob {len(data)}\0".encode() + data).hexdigest()
        valid = {"sha": sha, "size": len(data), "encoding": "base64", "content": base64.b64encode(data).decode()}
        cache = {}
        with patch.object(subject, "_gh_json", return_value=valid) as loader:
            self.assertEqual(subject._pr_blob_bytes("owner/repo", sha, cache), data)
            self.assertEqual(subject._pr_blob_bytes("owner/repo", sha, cache), data)
        self.assertEqual(loader.call_count, 1)
        invalid = [
            {**valid, "sha": "d" * 40}, {**valid, "encoding": "hex"},
            {**valid, "size": len(data) + 1}, {**valid, "size": subject._MAX_PROVENANCE_BLOB_BYTES + 1},
            {**valid, "content": "invalid base64!"},
            {**valid, "content": base64.b64encode(b"different source\n").decode()},
        ]
        for payload in invalid:
            with self.subTest(payload=payload):
                with patch.object(subject, "_gh_json", return_value=payload):
                    with self.assertRaises(subject.ContractViolation):
                        subject._pr_blob_bytes("owner/repo", sha, {})
        with patch.object(subject, "_gh_json", return_value=valid):
            with patch.object(subject, "_MAX_PROVENANCE_BLOB_BYTES", len(data)):
                with self.assertRaisesRegex(subject.ContractViolation, "cacheが上限超過"):
                    subject._pr_blob_bytes("owner/repo", sha, {"e" * 40: b"existing"})

    def test_native_copy_detector_isolates_inherited_git_environment_and_global_config(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory)
            subject._run_git(repository, "init", "-q", "--initial-branch=main")
            subject._run_git(repository, "config", "user.email", "test@example.com")
            subject._run_git(repository, "config", "user.name", "Hook test")
            (repository / "README.md").write_text("real repository\n", encoding="utf-8")
            subject._run_git(repository, "add", ".")
            subject._run_git(repository, "commit", "-qm", "baseline")
            git_directory = repository / ".git"
            head = subject._run_git(repository, "rev-parse", "HEAD")
            objects_before = {path.relative_to(git_directory / "objects") for path in (git_directory / "objects").rglob("*") if path.is_file()}
            sentinel = repository / "external-diff-ran"
            external = repository / "external.sh"
            external.write_text(f"#!/bin/sh\ntouch '{sentinel}'\n", encoding="utf-8")
            external.chmod(0o755)
            global_config = repository / "global-config"
            global_config.write_text(f"[diff]\n external = {external}\n", encoding="utf-8")
            data = "".join(f"source implementation line {index}\n" for index in range(100)).encode()
            edited = data.replace(b"line 99\n", b"changed final line\n")
            blobs = {hashlib.sha1(f"blob {len(blob)}\0".encode() + blob).hexdigest(): blob for blob in (data, edited)}
            original_sha, copied_sha = blobs
            inherited = {
                "GIT_DIR": str(git_directory), "GIT_COMMON_DIR": str(git_directory),
                "GIT_WORK_TREE": str(repository), "GIT_INDEX_FILE": str(git_directory / "index"),
                "GIT_OBJECT_DIRECTORY": str(git_directory / "objects"),
                "GIT_ALTERNATE_OBJECT_DIRECTORIES": str(git_directory / "objects"),
                "GIT_CONFIG_GLOBAL": str(global_config), "GIT_EXTERNAL_DIFF": str(external),
            }
            with patch.dict(os.environ, inherited):
                copies = subject._native_copy_sources(
                    {"old.rs": ("blob", "100644", original_sha)},
                    {"copied.rs": ("blob", "100755", copied_sha)}, blobs.__getitem__,
                )
            self.assertEqual(copies, {"copied.rs": "old.rs"})
            self.assertFalse(sentinel.exists())
            self.assertEqual(subject._run_git(repository, "rev-parse", "HEAD"), head)
            objects_after = {path.relative_to(git_directory / "objects") for path in (git_directory / "objects").rglob("*") if path.is_file()}
            self.assertEqual(objects_after, objects_before)

    def test_remote_commit_rename_provenance_rejects_current_net_path(self) -> None:
        repository = "HiroyukiFuruno/katana-render-runtime"
        issue_commit, head = "c" * 40, "d" * 40
        compare = {
            "base_commit": {"sha": issue_commit},
            "total_commits": 1,
            "ahead_by": 1,
            "commits": [{"sha": head}],
            "files": [{
                "filename": "new/name.rs",
                "previous_filename": "old/name.rs",
                "status": "renamed",
            }],
        }
        commit = {"sha": head, "parents": [{"sha": issue_commit}]}

        def gh_json(*arguments: str) -> object:
            if arguments[0] == f"repos/{repository}/git/commits/{head}":
                return commit
            if arguments[0] == f"repos/{repository}/compare/{issue_commit}...{head}":
                return compare
            raise AssertionError(f"unexpected GitHub API request: {arguments}")

        with patch.object(subject, "_gh_json", side_effect=gh_json):
            surviving_paths = subject._pr_surviving_paths_at_head(
                repository,
                issue_commit,
                head,
                ["old/name.rs"],
                {},
            )
        self.assertEqual(surviving_paths, ["new/name.rs"])
        with self.assertRaisesRegex(subject.ContractViolation, "rename先"):
            subject._effective_release_issue_numbers(
                repository=repository,
                references_by_commit=[(issue_commit, {64})],
                net_paths=["new/name.rs"],
                issue_loader=lambda number: self.issue(number, state="CLOSED"),
                commit_paths=lambda _sha: ["old/name.rs"],
                commit_surviving_paths=lambda _sha, _paths: surviving_paths,
            )

    def test_remote_rename_provenance_traces_both_merge_parents_and_later_edits(self) -> None:
        repository = "HiroyukiFuruno/katana-render-runtime"
        source, mainline, rename_side, merge, head = (
            character * 40 for character in ("1", "2", "3", "4", "5")
        )
        compare_range = {
            "base_commit": {"sha": source},
            "total_commits": 4,
            "ahead_by": 4,
            "commits": [{"sha": sha} for sha in (mainline, rename_side, merge, head)],
        }
        commits = {
            mainline: {"sha": mainline, "parents": [{"sha": source}]},
            rename_side: {"sha": rename_side, "parents": [{"sha": source}]},
            merge: {"sha": merge, "parents": [{"sha": mainline}, {"sha": rename_side}]},
            head: {"sha": head, "parents": [{"sha": merge}]},
        }
        edge_files = {
            (source, mainline): [],
            (source, rename_side): [{
                "filename": "new/name.rs",
                "previous_filename": "old/name.rs",
                "status": "renamed",
            }],
            (mainline, merge): [{
                "filename": "new/name.rs",
                "previous_filename": "old/name.rs",
                "status": "renamed",
            }],
            (rename_side, merge): [],
            (merge, head): [{"filename": "new/name.rs", "status": "modified"}],
        }
        calls: list[str] = []

        def gh_json(*arguments: str) -> object:
            endpoint = arguments[0]
            calls.append(endpoint)
            if endpoint == f"repos/{repository}/compare/{source}...{head}":
                return compare_range
            for commit_sha, payload in commits.items():
                if endpoint == f"repos/{repository}/git/commits/{commit_sha}":
                    return payload
            for (parent_sha, commit_sha), files in edge_files.items():
                if endpoint == f"repos/{repository}/compare/{parent_sha}...{commit_sha}":
                    return {"base_commit": {"sha": parent_sha}, "files": files}
            raise AssertionError(f"unexpected GitHub API request: {arguments}")

        edge_cache: dict[tuple[str, str], tuple[tuple[str, tuple[str, ...]], ...]] = {}
        with patch.object(subject, "_gh_json", side_effect=gh_json):
            with self.assertRaisesRegex(subject.ContractViolation, "rename先"):
                subject._effective_release_issue_numbers(
                    repository=repository,
                    references_by_commit=[(source, {64})],
                    net_paths=["new/name.rs"],
                    issue_loader=lambda number: self.issue(number, state="CLOSED"),
                    commit_paths=lambda _sha: ["old/name.rs"],
                    commit_surviving_paths=lambda sha, paths: subject._pr_surviving_paths_at_head(
                        repository, sha, head, paths, edge_cache
                    ),
                )
        self.assertIn((merge, head), edge_cache)
        self.assertEqual(len(edge_cache), len(edge_files))

    def test_remote_same_commit_heavy_move_rewrite_fails_closed(self) -> None:
        repository = "HiroyukiFuruno/katana-render-runtime"
        source, head = "6" * 40, "7" * 40
        compare = {
            "base_commit": {"sha": source},
            "total_commits": 1,
            "ahead_by": 1,
            "commits": [{"sha": head}],
            "files": [
                {"filename": "old/name.rs", "status": "removed"},
                {"filename": "new/name.rs", "status": "added"},
            ],
        }
        commit = {"sha": head, "parents": [{"sha": source}]}

        def gh_json(*arguments: str) -> object:
            endpoint = arguments[0]
            if endpoint == f"repos/{repository}/git/commits/{head}":
                return commit
            if endpoint == f"repos/{repository}/compare/{source}...{head}":
                return compare
            raise AssertionError(f"unexpected GitHub API request: {arguments}")

        edge_cache: dict[tuple[str, str], tuple[tuple[str, tuple[str, ...]], ...]] = {}
        with patch.object(subject, "_gh_json", side_effect=gh_json):
            with self.assertRaisesRegex(subject.ContractViolation, "削除と同一edgeの追加"):
                subject._effective_release_issue_numbers(
                    repository=repository,
                    references_by_commit=[(source, {64})],
                    net_paths=["new/name.rs"],
                    issue_loader=lambda number: self.issue(number, state="CLOSED"),
                    commit_paths=lambda _sha: ["old/name.rs"],
                    commit_surviving_paths=lambda sha, paths: subject._pr_surviving_paths_at_head(
                        repository, sha, head, paths, edge_cache
                    ),
                )

    def test_closed_release_reference_fails_closed_on_missing_or_truncated_provenance(self) -> None:
        closed = self.issue(64, state="CLOSED")
        for paths in ([],):
            with self.subTest(paths=paths), self.assertRaisesRegex(subject.ContractViolation, "証跡不足"):
                subject._effective_release_issue_numbers(
                    repository="HiroyukiFuruno/katana-render-runtime",
                    references_by_commit=[("a" * 40, {64})],
                    net_paths=["src/current.rs"],
                    issue_loader=lambda _number: closed,
                    commit_paths=lambda _sha: paths,
                    commit_surviving_paths=lambda _sha, _paths: [],
                )
        with self.assertRaisesRegex(subject.ContractViolation, "provenance incomplete"):
            subject._effective_release_issue_numbers(
                repository="HiroyukiFuruno/katana-render-runtime",
                references_by_commit=[("a" * 40, {64})],
                net_paths=["src/current.rs"],
                issue_loader=lambda _number: closed,
                commit_paths=lambda _sha: (_ for _ in ()).throw(
                    subject.ContractViolation("provenance incomplete")
                ),
                commit_surviving_paths=lambda _sha, _paths: [],
            )

    def test_closed_release_reference_intersects_renamed_source_path(self) -> None:
        closed = self.issue(64, state="CLOSED")
        renamed = subject.parse_name_status_paths("R100\0old/name.rs\0new/name.rs\0")
        with self.assertRaisesRegex(subject.ContractViolation, "重複"):
            subject._effective_release_issue_numbers(
                repository="HiroyukiFuruno/katana-render-runtime",
                references_by_commit=[("a" * 40, {64})],
                net_paths=renamed,
                issue_loader=lambda _number: closed,
                commit_paths=lambda _sha: ["old/name.rs"],
                commit_surviving_paths=lambda _sha, paths: paths,
            )

    def test_merge_commit_closed_reference_fails_conservatively(self) -> None:
        closed = self.issue(64, state="CLOSED")
        with self.assertRaisesRegex(subject.ContractViolation, "merge"):
            subject._effective_release_issue_numbers(
                repository="HiroyukiFuruno/katana-render-runtime",
                references_by_commit=[("a" * 40, {64})],
                net_paths=["src/current.rs"],
                issue_loader=lambda _number: closed,
                commit_paths=lambda _sha: (_ for _ in ()).throw(
                    subject.ContractViolation("merge commitのprovenanceを判定できません")
                ),
                commit_surviving_paths=lambda _sha, _paths: [],
            )

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

    def test_release_pr_range_accepts_obsolete_closed_reference_with_open_issue(self) -> None:
        repository = "HiroyukiFuruno/katana-render-runtime"
        base_sha, obsolete_sha, head_sha = "a" * 40, "c" * 40, "b" * 40
        main_compare = {
            "base_commit": {"sha": base_sha},
            "merge_base_commit": {"sha": base_sha},
            "status": "ahead",
            "ahead_by": 2,
            "behind_by": 0,
            "total_commits": 2,
            "commits": [
                {"sha": obsolete_sha, "commit": {"message": "old change\n\nRefs #64"}},
                {"sha": head_sha, "commit": {"message": "current change\n\nRefs #65"}},
            ],
            "files": [{"filename": "src/active.rs"}],
        }
        historical_compare = {
            "base_commit": {"sha": base_sha},
            "files": [{"filename": "old/obsolete.yml"}],
        }
        rename_compare = {
            "base_commit": {"sha": obsolete_sha},
            "total_commits": 1,
            "ahead_by": 1,
            "commits": [{"sha": head_sha}],
            "files": [
                {"filename": "old/obsolete.yml", "status": "removed"},
                {"filename": "src/active.rs", "status": "modified"},
            ],
        }

        def gh_json(*arguments: str) -> object:
            endpoint = arguments[0]
            if endpoint == f"repos/{repository}/compare/{base_sha}...{head_sha}":
                return main_compare
            if endpoint == f"repos/{repository}/git/commits/{obsolete_sha}":
                return {"sha": obsolete_sha, "parents": [{"sha": base_sha}]}
            if endpoint == f"repos/{repository}/compare/{base_sha}...{obsolete_sha}":
                return historical_compare
            if endpoint == f"repos/{repository}/compare/{obsolete_sha}...{head_sha}":
                return rename_compare
            if endpoint == f"repos/{repository}/git/commits/{head_sha}":
                return {"sha": head_sha, "parents": [{"sha": obsolete_sha}]}
            raise AssertionError(f"unexpected GitHub API request: {arguments}")

        def issue_loader(number: int) -> subject.Issue:
            return self.issue(number, state="CLOSED" if number == 64 else "OPEN")

        with patch.object(subject, "_gh_json", side_effect=gh_json):
            references = subject.validate_pr_range(
                repository=repository,
                pr_number=99,
                base_sha=base_sha,
                head_sha=head_sha,
                branch="release/v0.4.22",
                issue_loader=issue_loader,
            )
        self.assertEqual(references, {65})

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

    def test_release_pr_range_accepts_multiple_open_canonical_issues(self) -> None:
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
                {"sha": "c" * 40, "commit": {"message": "release: v0.4.22\n\nRefs #64 #65"}},
                {"sha": head_sha, "commit": {"message": "fix: review feedback"}},
            ],
            "files": [{"filename": "scripts/hooks/pre-push.sh"}],
        }
        issues = {64: self.issue(64), 65: self.issue(65)}
        with patch.object(subject, "_gh_json", return_value=compare):
            self.assertEqual(
                subject.validate_pr_range(
                    repository="HiroyukiFuruno/katana-render-runtime",
                    pr_number=99,
                    base_sha=base_sha,
                    head_sha=head_sha,
                    branch="release/v0.4.22",
                    issue_loader=issues.get,
                ),
                {64, 65},
            )

    def test_pr_range_rejects_closed_release_issue(self) -> None:
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
                    "commit": {"message": "release: v0.4.21\n\nRefs #64"},
                }
            ],
            "files": [{"filename": "Cargo.lock"}],
        }
        with patch.object(subject, "_gh_json", return_value=compare):
            with self.assertRaisesRegex(subject.ContractViolation, "provenance commit SHA"):
                subject.validate_pr_range(
                    repository="HiroyukiFuruno/katana-render-runtime",
                    pr_number=87,
                    base_sha=base_sha,
                    head_sha=head_sha,
                    branch="release/v0.4.21",
                    issue_loader=lambda number: self.issue(number, state="CLOSED"),
                )

    def test_pr_range_rejects_closed_issue_outside_release_branch(self) -> None:
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
            "files": [{"filename": "scripts/hooks/pre-push.sh"}],
        }
        with patch.object(subject, "_gh_json", return_value=compare):
            with self.assertRaisesRegex(subject.ContractViolation, "OPEN"):
                subject.validate_pr_range(
                    repository="HiroyukiFuruno/katana-render-runtime",
                    pr_number=87,
                    base_sha=base_sha,
                    head_sha=head_sha,
                    branch="fix/issue-contract",
                    issue_loader=lambda number: self.issue(number, state="CLOSED"),
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

    def test_pr_changed_paths_uses_complete_commit_trees_at_github_compare_files_limit(self) -> None:
        base_sha, merge_base_sha, head_sha = "a" * 40, "b" * 40, "c" * 40
        compare = {
            "base_commit": {"sha": base_sha},
            "merge_base_commit": {"sha": merge_base_sha},
            "files": [{"filename": f"fixtures/{entry}.txt"} for entry in range(300)],
        }
        merge_base_tree_sha, head_tree_sha = "d" * 40, "e" * 40
        merge_base_entries = [
            {"path": f"fixtures/{entry}.txt", "mode": "100644", "type": "blob", "sha": "e" * 40}
            for entry in range(301)
        ]
        head_entries = [*merge_base_entries]
        head_entries[0] = {**head_entries[0], "sha": "f" * 40}
        head_entries.append(
            {
                "path": ".github/workflows/new.yml",
                "mode": "100644",
                "type": "blob",
                "sha": "1" * 40,
            }
        )

        def gh_json(*arguments: str) -> object:
            endpoint = arguments[0]
            if "/compare/" in endpoint:
                return compare
            if f"/git/commits/{merge_base_sha}" in endpoint:
                return {"sha": merge_base_sha, "tree": {"sha": merge_base_tree_sha}}
            if f"/git/commits/{head_sha}" in endpoint:
                return {"sha": head_sha, "tree": {"sha": head_tree_sha}}
            if f"/git/trees/{merge_base_tree_sha}?recursive=1" in endpoint:
                return {"sha": merge_base_tree_sha, "truncated": False, "tree": merge_base_entries}
            if f"/git/trees/{head_tree_sha}?recursive=1" in endpoint:
                return {"sha": head_tree_sha, "truncated": False, "tree": head_entries}
            raise AssertionError(endpoint)

        with patch.object(subject, "_gh_json", side_effect=gh_json):
            self.assertEqual(
                subject._pr_changed_paths(
                    repository="HiroyukiFuruno/katana-render-runtime",
                    base_sha=base_sha,
                    head_sha=head_sha,
                ),
                [".github/workflows/new.yml", "fixtures/0.txt"],
            )

    def test_pr_changed_paths_fails_closed_when_fallback_tree_is_truncated(self) -> None:
        base_sha, head_sha = "a" * 40, "b" * 40
        base_tree_sha = "c" * 40
        compare = {
            "base_commit": {"sha": base_sha},
            "merge_base_commit": {"sha": base_sha},
            "files": [{"filename": f"fixtures/{entry}.txt"} for entry in range(300)],
        }

        def gh_json(*arguments: str) -> object:
            endpoint = arguments[0]
            if "/compare/" in endpoint:
                return compare
            if f"/git/commits/{base_sha}" in endpoint:
                return {"sha": base_sha, "tree": {"sha": base_tree_sha}}
            if f"/git/trees/{base_tree_sha}?recursive=1" in endpoint:
                return {"truncated": True, "tree": []}
            raise AssertionError(endpoint)

        with patch.object(subject, "_gh_json", side_effect=gh_json):
            with self.assertRaisesRegex(subject.ContractViolation, "打ち切られた"):
                subject._pr_changed_paths(
                    repository="HiroyukiFuruno/katana-render-runtime",
                    base_sha=base_sha,
                    head_sha=head_sha,
                )

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
