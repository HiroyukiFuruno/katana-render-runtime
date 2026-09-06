from __future__ import annotations

import os
import stat
import subprocess
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("pre-push.sh")


class PrePushDispatcherTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        self.repository = self.root / "repository"
        self.bin_directory = self.root / "bin"
        self.log = self.root / "order.log"
        self.issue_stdin = self.root / "issue-stdin.log"
        self.issue_arguments = self.root / "issue-arguments.log"
        self.repository.mkdir()
        self.bin_directory.mkdir()
        subprocess.run(
            ["git", "init", "--initial-branch=master"],
            cwd=self.repository,
            check=True,
            capture_output=True,
            text=True,
        )
        self.write_executable(
            "just",
            '#!/bin/sh\nprintf "check\\n" >> "$ORDER_LOG"\n'
            'if [ "${JUST_CONSUME_STDIN:-0}" = "1" ]; then cat >/dev/null; fi\n'
            'exit "${JUST_EXIT:-0}"\n',
        )
        self.write_executable(
            "python3",
            '#!/bin/sh\nprintf "issue\\n" >> "$ORDER_LOG"\n'
            'printf "%s\\n" "$*" > "$ISSUE_ARGUMENTS_LOG"\n'
            'cat > "$ISSUE_STDIN_LOG"\nexit 0\n',
        )

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def write_executable(self, name: str, content: str) -> None:
        path = self.bin_directory / name
        path.write_text(content, encoding="utf-8")
        path.chmod(path.stat().st_mode | stat.S_IXUSR)

    def run_dispatcher(
        self,
        *,
        just_exit: int = 0,
        push_input: str = "",
        just_consume_stdin: bool = False,
        dispatcher_arguments: tuple[str, ...] = (),
    ) -> subprocess.CompletedProcess[str]:
        environment = os.environ.copy()
        environment["PATH"] = f"{self.bin_directory}:{environment['PATH']}"
        environment["ORDER_LOG"] = str(self.log)
        environment["ISSUE_STDIN_LOG"] = str(self.issue_stdin)
        environment["ISSUE_ARGUMENTS_LOG"] = str(self.issue_arguments)
        environment["JUST_EXIT"] = str(just_exit)
        environment["JUST_CONSUME_STDIN"] = "1" if just_consume_stdin else "0"
        return subprocess.run(
            ["bash", str(SCRIPT), *dispatcher_arguments],
            cwd=self.repository,
            env=environment,
            input=push_input,
            capture_output=True,
            text=True,
            check=False,
        )

    def test_repository_check_runs_before_issue_contract(self) -> None:
        result = self.run_dispatcher()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.log.read_text(encoding="utf-8").splitlines(), ["check", "issue"])

    def test_issue_contract_does_not_run_when_repository_check_fails(self) -> None:
        result = self.run_dispatcher(just_exit=19)
        self.assertEqual(result.returncode, 19)
        self.assertEqual(self.log.read_text(encoding="utf-8").splitlines(), ["check"])

    def test_push_updates_survive_repository_check_stdin_consumption(self) -> None:
        update = (
            "refs/heads/topic "
            + "1" * 40
            + " refs/heads/topic "
            + "0" * 40
            + "\n"
        )
        result = self.run_dispatcher(
            push_input=update,
            just_consume_stdin=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.issue_stdin.read_text(encoding="utf-8"), update)

    def test_remote_name_is_forwarded_to_issue_contract(self) -> None:
        result = self.run_dispatcher(dispatcher_arguments=("upstream", "https://example.test/repo.git"))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.issue_arguments.read_text(encoding="utf-8"),
            "scripts/hooks/verify_push_issue.py --remote upstream --remote-url https://example.test/repo.git\n",
        )

    def test_url_push_forwards_url_as_both_remote_arguments(self) -> None:
        url = "https://example.test/repo.git"
        result = self.run_dispatcher(dispatcher_arguments=(url, url))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.issue_arguments.read_text(encoding="utf-8"),
            f"scripts/hooks/verify_push_issue.py --remote {url} --remote-url {url}\n",
        )

    def test_direct_execution_omits_remote_override(self) -> None:
        result = self.run_dispatcher()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.issue_arguments.read_text(encoding="utf-8"), "scripts/hooks/verify_push_issue.py\n")


if __name__ == "__main__":
    unittest.main()
