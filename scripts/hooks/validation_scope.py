"""Classify Git changes that can be validated without product builds."""

from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys
from collections.abc import Sequence


AUTOMATION_FILES = frozenset(
    {
        ".github/workflows/release-preflight.yml",
        ".github/workflows/release-publish-retry.yml",
        ".github/workflows/release.yml",
        ".github/workflows/test-and-build.yml",
        "docs/release.md",
        "scripts/hooks/check_automation_changes.sh",
        "scripts/hooks/pre-push.sh",
        "scripts/hooks/validation_scope.py",
        "scripts/hooks/validation_scope_test.py",
        "scripts/release/draft_ci_contract_test.py",
        "scripts/release/html_performance_observer_workflow_test.py",
        "scripts/release/local_html_performance_evidence_test.py",
        "scripts/release/local_macos_evidence.py",
        "scripts/release/local_macos_workflow_contract_test.py",
        "scripts/release/native_coverage_reuse_test.py",
        "scripts/release/publish-crates.sh",
        "scripts/release/publish_crates_test.py",
        "scripts/release/release_workflow_contract_test.py",
        "scripts/release/verify_ci_quality_evidence_test.py",
        "scripts/release/verify_completed_release_context.py",
        "scripts/release/verify_completed_release_context_test.py",
    }
)
SHA = re.compile(r"[0-9a-f]{40}")


def scope(repo: Path, base: str, head: str) -> str:
    if SHA.fullmatch(base) is None or SHA.fullmatch(head) is None:
        raise ValueError("validation scope requires full base and head SHAs")
    git = ["git", "--no-replace-objects", "-C", str(repo)]
    ancestor = subprocess.run(
        [*git, "merge-base", "--is-ancestor", base, head],
        check=False,
    )
    if ancestor.returncode == 1:
        return "full"
    ancestor.check_returncode()
    raw = subprocess.check_output(
        [*git, "diff", "--raw", "--no-ext-diff", "--no-renames", "-z", base, head, "--"],
    )
    rows = raw.removesuffix(b"\0").split(b"\0")
    if not raw:
        return "full"
    for meta, path in zip(rows[::2], rows[1::2], strict=True):
        old_mode, new_mode, _, _, status = meta[1:].split()
        filename = path.decode("utf-8", "surrogateescape")
        if status not in {b"A", b"M"} or new_mode not in {b"100644", b"100755"}:
            return "full"
        if old_mode not in {b"000000", new_mode} or filename not in AUTOMATION_FILES:
            return "full"
    return "automation"


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--base", required=True)
    parser.add_argument("--head", required=True)
    args = parser.parse_args(argv)
    try:
        print(scope(args.repo, args.base, args.head))
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f"validation scope failed: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
