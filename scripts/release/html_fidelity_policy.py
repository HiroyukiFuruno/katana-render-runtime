#!/usr/bin/env python3
"""Select the explicitly authorized HTML release phase and bind it to source."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
POLICY_PATH = Path("scripts/release/html-fidelity-policy.json")
SOURCE_PATHS = (
    Path(".github/workflows/test-and-build.yml"), Path("scripts/release/verify_ci_quality_evidence.py"),
    POLICY_PATH, Path("scripts/release/html_fidelity_policy.py"), Path("Justfile"),
    Path("scripts/html/fidelity-suite.ts"), Path("scripts/html/interim-feature-results.ts"),
    Path("scripts/html/feature-catalog.ts"), Path("scripts/html/fidelity-phase.ts"),
    Path("scripts/html/dynamic-fidelity.ts"), Path("scripts/html/visual-state-fidelity.ts"),
    Path("tests/fixtures/html/feature-catalog.json"),
)


def select_html_policy(version: str, source_ref: str, root: Path = ROOT) -> dict[str, object]:
    raw = (root / POLICY_PATH).read_bytes()
    policy = json.loads(raw)
    interim_release = {
        "version": "0.4.24", "phase": "interim", "threshold": 90,
        "authorization": "https://github.com/HiroyukiFuruno/katana-render-runtime/issues/109",
    }
    required = {"schema": 1, "defaultPhase": "final", "interimRelease": interim_release}
    if json.dumps(policy, sort_keys=True) != json.dumps(required, sort_keys=True):
        raise ValueError("invalid or unauthorized HTML fidelity release policy")
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        raise ValueError("invalid release version")
    allowed = {"release/v0.4.24", "refs/heads/release/v0.4.24", "master", "refs/heads/master", "refs/tags/v0.4.24"}
    if source_ref.startswith(("release/v", "refs/heads/release/v", "refs/tags/v")):
        if source_ref.rsplit("v", 1)[1] != version:
            raise ValueError("release source ref and package version differ")
    interim = version == "0.4.24" and source_ref in allowed
    source_hash = hashlib.sha256()
    for name in SOURCE_PATHS:
        source_hash.update(str(name).encode() + b"\0" + (root / name).read_bytes() + b"\0")
    return {"phase": "interim" if interim else "final", "threshold": 90 if interim else 99,
            "version": version, "authorization": interim_release["authorization"] if interim else None,
            "policy_sha256": hashlib.sha256(raw).hexdigest(), "source_sha256": source_hash.hexdigest()}


def workspace_version(root: Path = ROOT) -> str:
    import tomllib
    value = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    if not isinstance(value, str):
        raise ValueError("workspace version is not a string")
    return value


def assert_head_source_binding(head_sha: str, root: Path = ROOT) -> None:
    if not re.fullmatch(r"[0-9a-f]{40}", head_sha):
        raise ValueError("invalid current source HEAD")
    # merge checkout も、選択・比較契約が現在の PR HEAD と一致する場合だけ再利用する。
    for name in (*SOURCE_PATHS, Path("Cargo.toml")):
        expected = subprocess.check_output(["git", "show", f"{head_sha}:{name}"], cwd=root)
        if expected != (root / name).read_bytes():
            raise ValueError(f"HTML source differs from current HEAD: {name}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ref", default=os.environ.get("GITHUB_HEAD_REF") or os.environ.get("GITHUB_REF"))
    parser.add_argument("--head-sha", default=os.environ.get("PR_HEAD_SHA") or os.environ.get("GITHUB_SHA"))
    parser.add_argument("--format", choices=("json", "recipe"), default="json")
    args = parser.parse_args()
    source_ref = args.ref or subprocess.check_output(["git", "symbolic-ref", "--short", "HEAD"], cwd=ROOT, text=True).strip()
    if args.head_sha:
        assert_head_source_binding(args.head_sha)
    selection = select_html_policy(workspace_version(), source_ref)
    if args.format == "recipe":
        print("html-fidelity-interim-check" if selection["phase"] == "interim" else "html-fidelity-check")
    else:
        print(json.dumps(selection, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
