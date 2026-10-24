#!/usr/bin/env python3
"""Native CLI観測から安全な実行要約だけを出力する。"""

from __future__ import annotations

import json
import re
import stat
import sys
from collections.abc import Sequence
from pathlib import Path
from typing import cast

import verify_html_performance_evidence as gate

OBSERVATION_FIELDS = {
    "argv", "stdout", "stderr", "label", "cwd", "attemptToken",
    "environment", "environmentSha256", "pid", "actualExit",
    "startedNs", "endedNs", "reaped", "errors",
}
ARTIFACT_FIELDS = {"path", "sha256", "size"}
EVIDENCE_FIELDS = {"observation", "lifecycle"}
HEX_SHA256 = re.compile(r"^[0-9a-f]{64}$")
DECIMAL = re.compile(r"^[0-9]+$")


class ObservationViewError(ValueError):
    """安全な表示用要約を作れない入力。"""


def _object(value: object) -> dict[str, object]:
    if type(value) is not dict or any(type(key) is not str for key in value):
        raise ObservationViewError("invalid observation")
    return cast(dict[str, object], value)


def _exact_fields(value: dict[str, object], fields: set[str]) -> None:
    if set(value) != fields:
        raise ObservationViewError("invalid observation")


def _artifact(value: object) -> dict[str, object]:
    artifact = _object(value)
    _exact_fields(artifact, ARTIFACT_FIELDS)
    if type(artifact["path"]) is not str or not artifact["path"]:
        raise ObservationViewError("invalid observation")
    if type(artifact["sha256"]) is not str or not HEX_SHA256.fullmatch(artifact["sha256"]):
        raise ObservationViewError("invalid observation")
    if type(artifact["size"]) is not int or artifact["size"] < 0:
        raise ObservationViewError("invalid observation")
    return artifact


def safe_summary(value: object) -> dict[str, object]:
    observation = _observation(value)
    _exact_fields(observation, OBSERVATION_FIELDS)
    _validate_private_fields(observation)
    started = _timestamp(observation["startedNs"])
    ended = _timestamp(observation["endedNs"])
    if ended < started:
        raise ObservationViewError("invalid observation")
    return {
        "actualExit": observation["actualExit"],
        "pid": observation["pid"],
        "startedNs": observation["startedNs"],
        "endedNs": observation["endedNs"],
        "stdout": _safe_artifact(observation["stdout"]),
        "stderr": _safe_artifact(observation["stderr"]),
    }


def _observation(value: object) -> dict[str, object]:
    if isinstance(value, (list, tuple)) and len(value) == 2:
        _object(value[1])
        return _object(value[0])
    evidence = _object(value)
    if set(evidence) == EVIDENCE_FIELDS:
        if type(evidence["lifecycle"]) is not dict:
            raise ObservationViewError("invalid observation")
        return _object(evidence["observation"])
    return evidence


def _validate_private_fields(observation: dict[str, object]) -> None:
    if type(observation["argv"]) is not list or any(
        type(item) is not str for item in observation["argv"]
    ):
        raise ObservationViewError("invalid observation")
    for field in ("label", "cwd", "attemptToken", "environmentSha256"):
        if type(observation[field]) is not str:
            raise ObservationViewError("invalid observation")
    environment = _object(observation["environment"])
    if any(type(key) is not str or type(item) is not str for key, item in environment.items()):
        raise ObservationViewError("invalid observation")
    pid = observation["pid"]
    actual_exit = observation["actualExit"]
    if type(pid) is not int or pid <= 0 or (actual_exit is not None and type(actual_exit) is not int):
        raise ObservationViewError("invalid observation")
    if type(observation["reaped"]) is not bool or observation["reaped"] is not True:
        raise ObservationViewError("invalid observation")
    if type(observation["errors"]) is not list or observation["errors"]:
        raise ObservationViewError("invalid observation")
    _artifact(observation["stdout"])
    _artifact(observation["stderr"])


def _timestamp(value: object) -> int:
    if type(value) is not str or not DECIMAL.fullmatch(value):
        raise ObservationViewError("invalid observation")
    return int(value)


def _safe_artifact(value: object) -> dict[str, object]:
    artifact = _artifact(value)
    return {field: artifact[field] for field in ("path", "sha256", "size")}


def _read_observation(path: Path) -> object:
    mode = path.lstat().st_mode
    if stat.S_ISLNK(mode) or not stat.S_ISREG(mode):
        raise ObservationViewError("invalid observation")
    try:
        raw = path.read_text(encoding="utf-8")
        decoded = json.loads(raw, object_pairs_hook=gate.unique_object)
        return cast(object, decoded)
    except (OSError, UnicodeError, ValueError, RecursionError) as error:
        raise ObservationViewError("invalid observation") from error


def main(arguments: Sequence[str] | None = None) -> int:
    supplied = list(sys.argv[1:] if arguments is None else arguments)
    if len(supplied) != 1:
        return _fail()
    try:
        summary = safe_summary(_read_observation(Path(supplied[0])))
    except (OSError, ValueError, RecursionError):
        return _fail()
    sys.stdout.write(json.dumps(summary, sort_keys=True, ensure_ascii=True) + "\n")
    return 0


def _fail() -> int:
    sys.stderr.write("invalid native CLI observation\n")
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
