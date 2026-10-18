"""Pure validation and construction helpers for capacity guard state records."""
from __future__ import annotations

import hashlib
import json
import math
import re
import stat
from pathlib import Path
from typing import cast

SCHEMA = "krr-capacity-state-v2"
REFERENCE_SCHEMA = "krr-capacity-reference-v1"
METRIC = "unique-inode-allocated-bytes"
METRIC_VERSION = 1
REFERENCE_POLICY = "scripts/build/capacity-reference.json"
REFERENCE_ID = "krr-v0.4.24-fixed-capacity-v1"
REFERENCE_BASELINE_BYTES = 2_363_047_936
REFERENCE_FACTOR = 2.0
REFERENCE_OBSERVED_AT = 1_791_358_141.1059809
REFERENCE_SOURCE_STATE_SHA256 = "b623867f57b0d00a08838e11e8fb7d83f461daf2cdb03a82b94dccae4ffb7188"
_SHA256 = re.compile(r"[0-9a-f]{64}\Z")
JsonObject = dict[str, object]


def _object(value: object, label: str) -> JsonObject:
    if not isinstance(value, dict) or any(not isinstance(key, str) for key in value):
        raise ValueError(f"{label} must be an object")
    return cast(JsonObject, value)


def _exact_keys(value: JsonObject, keys: set[str], label: str) -> None:
    if value.keys() != keys:
        raise ValueError(f"{label} has unexpected or missing keys")


def _integer(value: object, label: str, *, positive: bool = False) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or (positive and value <= 0):
        raise ValueError(f"{label} must be a positive integer")
    return value


def _finite_number(value: object, label: str, *, positive: bool = False) -> int | float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f"{label} must be finite")
    try:
        finite = math.isfinite(value)
    except OverflowError as error:
        raise ValueError(f"{label} must be finite") from error
    if not finite:
        raise ValueError(f"{label} must be finite")
    if positive and value <= 0:
        raise ValueError(f"{label} must be positive")
    return value


def _canonical_root_target(
    root: Path,
    target: Path,
    *,
    allow_missing_target: bool,
    require_default_target: bool = False,
) -> tuple[Path, Path]:
    if not root.is_absolute() or ".." in root.parts:
        raise ValueError("root must be canonical and absolute")
    canonical_root = root.resolve(strict=True)
    if canonical_root != root or not canonical_root.is_dir():
        raise ValueError("root must be a canonical directory")
    if not target.is_absolute() or ".." in target.parts:
        raise ValueError("target must be canonical and absolute")
    expected = canonical_root / "target"
    try:
        relative_target = target.relative_to(canonical_root)
    except ValueError as error:
        raise ValueError("target must be inside repository root") from error
    if not relative_target.parts:
        raise ValueError("target must be a strict descendant of repository root")
    if require_default_target and target != expected:
        raise ValueError("reference target must be root/target")
    cursor = canonical_root
    for part in target.relative_to(canonical_root).parts:
        cursor = cursor / part
        if cursor.is_symlink():
            raise ValueError("target must not contain symlinks")
    if target.exists() and not target.is_dir():
        raise ValueError("target must be a directory")
    if not target.exists() and not allow_missing_target:
        raise ValueError("runner target must exist")
    return canonical_root, target


def _policy_path(root: Path) -> Path:
    policy = root / REFERENCE_POLICY
    cursor = root
    for part in Path(REFERENCE_POLICY).parts:
        cursor /= part
        if cursor.is_symlink():
            raise ValueError("reference policy path must not contain symlinks")
    try:
        mode = policy.stat(follow_symlinks=False).st_mode
    except OSError as error:
        raise ValueError("reference policy is unavailable") from error
    if not stat.S_ISREG(mode):
        raise ValueError("reference policy must be a regular file")
    return policy


def _load_policy(root: Path) -> tuple[JsonObject, str]:
    path = _policy_path(root)
    try:
        raw = path.read_bytes()
        policy = _object(json.loads(raw), "reference policy")
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError("reference policy is not valid JSON") from error
    expected_keys = {
        "schema", "id", "baseline_bytes", "factor", "metric", "metric_version",
        "baseline_observed_at", "source_state_sha256",
    }
    _exact_keys(policy, expected_keys, "reference policy")
    if policy["schema"] != REFERENCE_SCHEMA or policy["id"] != REFERENCE_ID:
        raise ValueError("reference policy identity differs")
    baseline = _integer(policy["baseline_bytes"], "reference baseline", positive=True)
    factor = _finite_number(policy["factor"], "reference factor")
    observed_at = _finite_number(policy["baseline_observed_at"], "reference time", positive=True)
    if (baseline != REFERENCE_BASELINE_BYTES or factor != REFERENCE_FACTOR
            or observed_at != REFERENCE_OBSERVED_AT):
        raise ValueError("reference policy does not match the fixed baseline")
    metric_version = _integer(policy["metric_version"], "reference metric version")
    if policy["metric"] != METRIC or metric_version != METRIC_VERSION:
        raise ValueError("reference metric differs")
    source_sha = policy["source_state_sha256"]
    if (not isinstance(source_sha, str) or _SHA256.fullmatch(source_sha) is None
            or source_sha != REFERENCE_SOURCE_STATE_SHA256):
        raise ValueError("reference source state digest is invalid")
    return policy, hashlib.sha256(raw).hexdigest()


def validate_state(value: object, root: Path, target: Path) -> JsonObject:
    """Validate v2 records; pass legacy unversioned dictionaries through unchanged."""
    state = _object(value, "capacity state")
    if "schema" not in state:
        if "kind" in state or "reference" in state:
            raise ValueError("versioned capacity state is missing its schema")
        return state
    if state["schema"] != SCHEMA:
        raise ValueError("unknown capacity state schema")
    kind = state.get("kind")
    base_keys = {
        "schema", "kind", "target", "baseline_bytes", "factor", "metric",
        "metric_version", "baseline_observed_at",
    }
    if kind == "runner_measurement":
        _exact_keys(state, base_keys, "runner measurement")
        _canonical_root_target(root, target, allow_missing_target=True)
    elif kind == "repository_reference":
        _exact_keys(state, base_keys | {"reference"}, "repository reference")
        canonical_root, _ = _canonical_root_target(
            root, target, allow_missing_target=True, require_default_target=True
        )
        reference = _object(state["reference"], "reference")
        _exact_keys(reference, {"policy", "sha256", "id"}, "reference")
        if reference["policy"] != REFERENCE_POLICY or reference["id"] != REFERENCE_ID:
            raise ValueError("reference identity differs")
        digest = reference["sha256"]
        if not isinstance(digest, str) or _SHA256.fullmatch(digest) is None:
            raise ValueError("reference digest is invalid")
        policy, actual_digest = _load_policy(canonical_root)
        if digest != actual_digest:
            raise ValueError("reference policy digest differs")
        for key in ("baseline_bytes", "factor", "metric", "metric_version", "baseline_observed_at"):
            if state[key] != policy[key] or isinstance(state[key], bool):
                raise ValueError(f"reference state {key} differs from policy")
    else:
        raise ValueError("unknown capacity state kind")
    recorded_target = state["target"]
    if not isinstance(recorded_target, str) or recorded_target != str(target):
        raise ValueError("capacity state target differs")
    _integer(state["baseline_bytes"], "baseline bytes", positive=True)
    factor = _finite_number(state["factor"], "factor")
    if factor <= 1:
        raise ValueError("factor must be greater than one")
    metric_version = _integer(state["metric_version"], "metric version")
    if state["metric"] != METRIC or metric_version != METRIC_VERSION:
        raise ValueError("capacity metric differs")
    _finite_number(state["baseline_observed_at"], "baseline time", positive=True)
    return state


def create_runner_measurement(
    root: Path, target: Path, measured_bytes: int, observed_at: float,
    factor: float = REFERENCE_FACTOR,
) -> JsonObject:
    """Build a runner measurement only from values supplied by the caller."""
    canonical_root, canonical_target = _canonical_root_target(root, target, allow_missing_target=False)
    if canonical_root != root:
        raise ValueError("root must be canonical")
    _integer(measured_bytes, "measured bytes", positive=True)
    _finite_number(observed_at, "observed time", positive=True)
    _finite_number(factor, "factor")
    if factor <= 1:
        raise ValueError("factor must be greater than one")
    return {
        "schema": SCHEMA, "kind": "runner_measurement", "target": str(canonical_target),
        "baseline_bytes": measured_bytes, "factor": factor, "metric": METRIC,
        "metric_version": METRIC_VERSION, "baseline_observed_at": observed_at,
    }


def create_repository_reference(root: Path, target: Path) -> JsonObject:
    """Build a pinned reference state from the repository policy, without writing state."""
    canonical_root, canonical_target = _canonical_root_target(
        root, target, allow_missing_target=True, require_default_target=True
    )
    if canonical_root != root:
        raise ValueError("root must be canonical")
    policy, digest = _load_policy(canonical_root)
    return {
        "schema": SCHEMA, "kind": "repository_reference", "target": str(canonical_target),
        "baseline_bytes": policy["baseline_bytes"], "factor": policy["factor"],
        "metric": policy["metric"], "metric_version": policy["metric_version"],
        "baseline_observed_at": policy["baseline_observed_at"],
        "reference": {"policy": REFERENCE_POLICY, "sha256": digest, "id": REFERENCE_ID},
    }
