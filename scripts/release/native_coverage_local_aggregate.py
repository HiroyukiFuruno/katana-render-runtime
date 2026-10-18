"""KRRのローカルcoverage出力を再検証し、不正なcache結果は実検査に戻す。"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

PRODUCERS = {"root": "root-test-and-coverage", "performance": "performance-harness-coverage"}


def validate_or_refresh(root: Path, producer: str) -> dict[str, object]:
    recipe = PRODUCERS[producer]
    validate = [sys.executable, str(root / "scripts/release/native_coverage_result_cli.py"),
                "--repo", str(root), "local-validate", "--producer", producer]
    before = subprocess.run(validate, cwd=root, check=False, stdout=sys.stderr).returncode
    if before == 0:
        return {"producer": producer, "valid": True, "refreshed": False}
    # 再検査はキャッシュ済みtaskのhashから切り離し、新しいprobeで始める。
    environment = {name: value for name, value in os.environ.items()
                   if name not in {"MOON_TASK_ID", "MOON_TASK_HASH"}}
    fresh = subprocess.run(["just", recipe], cwd=root, check=False, stdout=sys.stderr,
                           env=environment).returncode
    if fresh != 0:
        return {"producer": producer, "valid": False, "refreshed": True,
                "validationBeforeExit": before, "producerExit": fresh}
    after = subprocess.run(validate, cwd=root, check=False, stdout=sys.stderr).returncode
    return {"producer": producer, "valid": after == 0, "refreshed": True,
            "validationBeforeExit": before, "producerExit": fresh, "validationAfterExit": after}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", type=Path, required=True)
    args = parser.parse_args(argv)
    root = args.repo.resolve(strict=True)
    rows = []
    for producer in PRODUCERS:
        row = validate_or_refresh(root, producer)
        rows.append(row)
        if row["valid"] is not True:
            break
    valid = len(rows) == len(PRODUCERS) and all(row["valid"] is True for row in rows)
    print(json.dumps({"schema": "krr-native-local-coverage-aggregate-v1",
                      "valid": valid, "producers": rows}, sort_keys=True))
    return 0 if valid else 1


if __name__ == "__main__":
    raise SystemExit(main())
