"""KRRのローカルcoverage出力を再検証し、不正なcache結果は実検査に戻す。"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

PRODUCERS = {"root": "root-test-and-coverage", "performance": "performance-harness-coverage"}
AGGREGATE_SCHEMA = "krr-native-local-coverage-aggregate-v1"
PRODUCER_VALIDATION_SCHEMA = "krr-native-local-coverage-producer-validation-v1"


def _cli(root: Path, command: str, producer: str) -> tuple[int, dict[str, object]]:
    result = subprocess.run(
        [sys.executable, str(root / "scripts/release/native_coverage_result_cli.py"),
         "--repo", str(root), command, "--producer", producer],
        cwd=root, check=False, capture_output=True, text=True,
    )
    try:
        payload = json.loads(result.stdout)
    except json.JSONDecodeError:
        payload = {"valid": False, "failure": "invalid-collection", "stderr": result.stderr.strip()}
    if not isinstance(payload, dict):
        payload = {"valid": False, "failure": "invalid-collection"}
    return result.returncode, payload


def validate_or_refresh(root: Path, producer: str) -> dict[str, object]:
    recipe = PRODUCERS[producer]
    before, validation = _cli(root, "local-validate", producer)
    if before == 0 and validation.get("valid") is True:
        return {"producer": producer, "valid": True, "refreshed": False,
                "failure": None, "validation": validation}
    failure = validation.get("failure")
    if failure == "coverage-policy":
        return {"producer": producer, "valid": False, "refreshed": False,
                "failure": "coverage-policy", "validation": validation}
    if failure not in (None, "invalid-collection") and before == 0:
        return {"producer": producer, "valid": False, "refreshed": False,
                "failure": "invalid-collection", "validation": validation}
    # 再検査はキャッシュ済みtaskのhashから切り離し、新しいprobeで始める。
    environment = {name: value for name, value in os.environ.items()
                   if name not in {"MOON_TASK_ID", "MOON_TASK_HASH"}}
    fresh = subprocess.run(["just", recipe], cwd=root, check=False, stdout=sys.stderr,
                           env=environment).returncode
    if fresh != 0:
        return {"producer": producer, "valid": False, "refreshed": True,
                "failure": "invalid-collection", "validationBeforeExit": before,
                "producerExit": fresh, "validation": validation}
    after, refreshed_validation = _cli(root, "local-validate", producer)
    if after != 0 or refreshed_validation.get("valid") is not True:
        return {"producer": producer, "valid": False, "refreshed": True,
                "failure": refreshed_validation.get("failure", "invalid-collection"),
                "validationBeforeExit": before, "producerExit": fresh,
                "validation": refreshed_validation}
    return {"producer": producer, "valid": True, "refreshed": True,
            "failure": None,
            "validationBeforeExit": before, "producerExit": fresh,
            "validationAfterExit": after, "validation": refreshed_validation}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--producer", choices=tuple(PRODUCERS))
    args = parser.parse_args(argv)
    root = args.repo.resolve(strict=True)
    rows = []
    selected_producers = (args.producer,) if args.producer is not None else tuple(PRODUCERS)
    for producer in selected_producers:
        row = validate_or_refresh(root, producer)
        rows.append(row)
        if row["valid"] is not True:
            break
    valid = len(rows) == len(selected_producers) and all(row["valid"] is True for row in rows)
    if args.producer is None:
        result = {"schema": AGGREGATE_SCHEMA, "valid": valid, "producers": rows}
    else:
        result = {
            "schema": PRODUCER_VALIDATION_SCHEMA,
            "scope": {"kind": "producer-only", "producer": args.producer, "fullAggregate": False},
            "valid": valid,
            "producers": rows,
        }
    print(json.dumps(result, sort_keys=True))
    return 0 if valid else 1


if __name__ == "__main__":
    raise SystemExit(main())
