#!/usr/bin/env python3
"""Regression tests for the paired html5ever dependency updater."""

from __future__ import annotations

import importlib.util
import subprocess
import unittest
from pathlib import Path
from unittest.mock import Mock


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/release/update_html5ever_pair.py"
SPEC = importlib.util.spec_from_file_location("update_html5ever_pair", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class UpdateHtml5everPairTest(unittest.TestCase):
    def test_wrapper_command_is_split_without_shell_execution(self) -> None:
        self.assertEqual(
            MODULE.parse_cargo_command('/opt/homebrew/bin/rtk cargo'),
            ["/opt/homebrew/bin/rtk", "cargo"],
        )

    def test_pair_update_keeps_parser_and_markup_model_together(self) -> None:
        commands = MODULE.update_commands(["rtk", "cargo"])
        self.assertEqual(
            commands,
            (
                [
                    "rtk",
                    "cargo",
                    "upgrade",
                    "--incompatible",
                    "allow",
                    "--package",
                    "html5ever",
                    "--package",
                    "markup5ever",
                ],
                ["rtk", "cargo", "update", "--recursive", "html5ever", "markup5ever"],
            ),
        )

    def test_runner_receives_argument_vectors_and_workspace_root(self) -> None:
        runner = Mock(return_value=subprocess.CompletedProcess([], 0))
        MODULE.run_updates(["rtk", "cargo"], runner)
        self.assertEqual(runner.call_count, 2)
        for call in runner.call_args_list:
            self.assertEqual(call.kwargs, {"cwd": MODULE.ROOT, "check": True})
            self.assertIsInstance(call.args[0], list)

    def test_dry_run_accepts_the_justfile_wrapper_command(self) -> None:
        result = subprocess.run(
            ["python3", str(SCRIPT), "--cargo", "/opt/homebrew/bin/rtk cargo", "--dry-run"],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("/opt/homebrew/bin/rtk cargo upgrade --incompatible", result.stdout)
        self.assertIn("--incompatible allow", result.stdout)
        self.assertIn("--package html5ever --package markup5ever", result.stdout)

    def test_depends_update_all_supplies_incompatible_policy_value(self) -> None:
        justfile = (ROOT / "Justfile").read_text(encoding="utf-8")
        self.assertIn(
            "{{CARGO}} upgrade -i allow --pinned allow --exclude html5ever",
            justfile,
        )


if __name__ == "__main__":
    unittest.main()
