"""Collect a conservative current identity for the runtime-unused test leaf."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import TypedDict, cast

import native_coverage_local_result as coverage_context
import runtime_script_profiles as profiles
import runtime_script_recipe as recipe_reader
import runtime_script_result as result


SCHEMA = "krr-runtime-script-identity-v1"
PROFILE = profiles.DEFAULT_PRODUCER
TOOL_FINGERPRINT = "scripts/runtime-bundles/runtime-bundle-tool-fingerprint.ts"
RUST_FACTS_HOST = "scripts/runtime-bundles/runtime-rust-facts-host.ts"
MATHJAX_EVIDENCE = "scripts/runtime-bundles/runtime-mathjax-audit-evidence.ts"
PREPARED_RUNTIME = "tmp/quality/classic-generation-evidence/current.json"
SHA256_RE = re.compile(r"[0-9a-f]{64}\Z")
MOON_TASK_ID_RE = re.compile(r"krr:[a-z0-9-]{1,100}\Z")


class IdentityReport(TypedDict):
    schema: str
    producer: str
    cacheable: bool
    missReasons: list[str]
    sourceManifestSha256: str | None
    recipeSha256: str | None
    toolEnvironmentSha256: str | None
    inputSha256: str | None
    identitySha256: str | None
    moonTaskHash: str | None


class _CollectionFailure(ValueError):
    def __init__(self, reason: str) -> None:
        super().__init__(reason)
        self.reason = reason


def _digest_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _canonical_digest(value: object) -> str:
    encoded = json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    return _digest_bytes(encoded.encode())


def _miss(producer: str, reason: str, moon_task_hash: str | None = None) -> IdentityReport:
    return {
        "schema": SCHEMA,
        "producer": producer,
        "cacheable": False,
        "missReasons": [reason],
        "sourceManifestSha256": None,
        "recipeSha256": None,
        "toolEnvironmentSha256": None,
        "inputSha256": None,
        "identitySha256": None,
        "moonTaskHash": moon_task_hash,
    }


def _run_json(
    repo: Path,
    argv: list[str],
    failure_reason: str,
    env_updates: dict[str, str] | None = None,
) -> object:
    try:
        process = subprocess.run(
            argv,
            cwd=repo,
            capture_output=True,
            check=False,
            env={**os.environ, **(env_updates or {})},
        )
    except OSError as error:
        raise _CollectionFailure(failure_reason) from error
    if process.returncode != 0:
        raise _CollectionFailure(failure_reason)
    try:
        return json.loads(process.stdout)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise _CollectionFailure(failure_reason) from error


def _source_digest(repo: Path) -> str:
    try:
        return coverage_context._source_digest(repo)
    except (OSError, ValueError) as error:
        raise _CollectionFailure("source_manifest_unavailable") from error


def _strict_fingerprint(
    repo: Path,
    argv: list[str],
    schema: str,
    failure_reason: str,
    env_updates: dict[str, str] | None = None,
) -> str:
    fingerprint = _run_json(
        repo,
        argv,
        failure_reason,
        env_updates,
    )
    raw_fingerprint = fingerprint.get("fingerprint") if isinstance(fingerprint, dict) else None
    if (
        not isinstance(fingerprint, dict)
        or set(fingerprint) != {"schema", "fingerprint"}
        or fingerprint.get("schema") != schema
        or not isinstance(raw_fingerprint, str)
        or SHA256_RE.fullmatch(raw_fingerprint) is None
    ):
        raise _CollectionFailure(failure_reason.replace("unavailable", "shape_unsupported"))
    return raw_fingerprint


def _rust_host_fingerprint(repo: Path) -> str:
    return _strict_fingerprint(
        repo,
        ["bun", RUST_FACTS_HOST, "fingerprint", str(repo)],
        "krr-rust-facts-host-fingerprint-v1",
        "rust_facts_host_unavailable",
    )


def _mathjax_evidence_fingerprint(repo: Path) -> str:
    return _strict_fingerprint(
        repo,
        ["bun", MATHJAX_EVIDENCE, "fingerprint"],
        "krr-mathjax-evidence-fingerprint-v1",
        "mathjax_evidence_unavailable",
        {"NODE_ENV": "test"},
    )


def _tool_fingerprint(repo: Path) -> dict[str, object]:
    fingerprint = _run_json(
        repo,
        ["bun", TOOL_FINGERPRINT],
        "tool_fingerprint_unavailable",
    )
    if (
        not isinstance(fingerprint, dict)
        or fingerprint.get("schema") != "krr-runtime-bundle-tool-fingerprint-v1"
        or not isinstance(fingerprint.get("tools"), dict)
        or not isinstance(fingerprint.get("environment"), list)
    ):
        raise _CollectionFailure("tool_fingerprint_shape_unsupported")
    try:
        python_digest = _digest_bytes(Path(sys.executable).resolve(strict=True).read_bytes())
        prepared_digest = _digest_bytes((repo / PREPARED_RUNTIME).read_bytes())
    except OSError as error:
        raise _CollectionFailure("producer_context_unavailable") from error
    return {
        "runtimeBundle": cast(dict[str, object], fingerprint),
        "rustFactsHostFingerprint": _rust_host_fingerprint(repo),
        "mathjaxEvidenceFingerprint": _mathjax_evidence_fingerprint(repo),
        "pythonVersion": sys.version,
        "pythonExecutableSha256": python_digest,
        "pythonEnvironmentSha256": _canonical_digest(
            {name: value for name, value in sorted(os.environ.items()) if name.startswith("PYTHON")}
        ),
        "pythonFlagsSha256": _digest_bytes(repr(sys.flags).encode()),
        "pythonSearchPathSha256": _canonical_digest(sys.path),
        "preparedRuntimeSha256": prepared_digest,
    }


def _moon_context() -> tuple[str | None, str | None]:
    task_id = os.environ.get("MOON_TASK_ID")
    raw_hash = os.environ.get("MOON_TASK_HASH")
    safe_task_id = task_id if task_id is not None and MOON_TASK_ID_RE.fullmatch(task_id) else None
    safe_hash = raw_hash if raw_hash is not None and SHA256_RE.fullmatch(raw_hash) else None
    return safe_task_id, safe_hash


def _identity_from_facts(
    producer: str,
    source_digest: str,
    recipe: recipe_reader.RuntimeScriptRecipe,
    fingerprint: dict[str, object],
    moon_task_hash: str | None,
) -> IdentityReport:
    tool_digest = _canonical_digest(fingerprint)
    input_digest = _canonical_digest(
        {
            "schema": "krr-runtime-script-producer-input-v1",
            "producer": producer,
            "sourceManifestSha256": source_digest,
            "recipeSha256": recipe.recipe_sha256,
            "toolEnvironmentSha256": tool_digest,
        }
    )
    try:
        identity_digest = result.build_identity(producer, input_digest)
    except result.RuntimeScriptResultError:
        return _miss(producer, "producer_identity_rejected", moon_task_hash)
    return {
        "schema": SCHEMA,
        "producer": producer,
        "cacheable": True,
        "missReasons": [],
        "sourceManifestSha256": source_digest,
        "recipeSha256": recipe.recipe_sha256,
        "toolEnvironmentSha256": tool_digest,
        "inputSha256": input_digest,
        "identitySha256": identity_digest,
        "moonTaskHash": moon_task_hash,
    }


def _collect_facts(
    repo: Path, producers: tuple[str, ...]
) -> tuple[str, dict[str, recipe_reader.RuntimeScriptRecipe], dict[str, object], str | None]:
    if not repo.is_absolute() or repo.resolve(strict=True) != repo:
        raise _CollectionFailure("repository_root_not_canonical")
    source_before = _source_digest(repo)
    tool_before = _tool_fingerprint(repo)
    recipes = {
        producer: recipe_reader.read_runtime_script_recipe(repo, producer)
        for producer in producers
    }
    for producer, recipe in recipes.items():
        if recipe.recipe_name != profiles.producer_recipe(producer):
            raise _CollectionFailure("recipe_profile_mismatch")
    source_after = _source_digest(repo)
    tool_after = _tool_fingerprint(repo)
    if source_before != source_after or tool_before != tool_after:
        raise _CollectionFailure("source_changed_during_collection")
    _, moon_task_hash = _moon_context()
    return source_before, recipes, tool_before, moon_task_hash


def _read_execution_facts(
    repo: Path, producer: str
) -> tuple[str, recipe_reader.RuntimeScriptRecipe, dict[str, object]]:
    if not repo.is_absolute() or repo.resolve(strict=True) != repo:
        raise _CollectionFailure("repository_root_not_canonical")
    source_digest = _source_digest(repo)
    recipe = recipe_reader.read_runtime_script_recipe(repo, producer)
    if recipe.recipe_name != profiles.producer_recipe(producer):
        raise _CollectionFailure("recipe_profile_mismatch")
    return source_digest, recipe, _tool_fingerprint(repo)


def collect_execution_snapshot(repo: Path, producer: str) -> IdentityReport:
    """Read one execution-side identity snapshot; callers must pair it around a run."""
    if producer not in profiles.accepted_producers():
        return _miss(producer, "unknown_profile")
    _, moon_task_hash = _moon_context()
    try:
        source_digest, recipe, fingerprint = _read_execution_facts(repo, producer)
    except (OSError, _CollectionFailure, recipe_reader.RuntimeScriptRecipeError) as error:
        return _miss(producer, _collection_miss_reason(error), moon_task_hash)
    return _identity_from_facts(producer, source_digest, recipe, fingerprint, moon_task_hash)


def collect_fixed_leaf_identity_reports(repo: Path) -> dict[str, IdentityReport]:
    """Collect all fixed leaf identities behind one conservative stability window."""
    producers = tuple(profiles.RUNTIME_SCRIPT_PROFILES)
    try:
        source_digest, recipes, fingerprint, moon_task_hash = _collect_facts(repo, producers)
    except (OSError, _CollectionFailure, recipe_reader.RuntimeScriptRecipeError) as error:
        reason = _collection_miss_reason(error)
        return {producer: _miss(producer, reason) for producer in producers}
    return {
        producer: _identity_from_facts(
            producer, source_digest, recipes[producer], fingerprint, moon_task_hash
        )
        for producer in producers
    }


def _collection_miss_reason(
    error: OSError | _CollectionFailure | recipe_reader.RuntimeScriptRecipeError,
) -> str:
    if isinstance(error, _CollectionFailure):
        return error.reason
    if isinstance(error, recipe_reader.RuntimeScriptRecipeError):
        return str(error)
    return "repository_unavailable"


def collect_identity_report(repo: Path, producer: str) -> IdentityReport:
    """Return current source, recipe and tool identity; unknown inputs are a cache miss."""
    if producer not in profiles.accepted_producers():
        return _miss(producer, "unknown_profile")
    moon_task_hash: str | None = None
    try:
        _, moon_task_hash = _moon_context()
        before, recipes, fingerprint, moon_task_hash = _collect_facts(repo, (producer,))
    except (OSError, _CollectionFailure, recipe_reader.RuntimeScriptRecipeError) as error:
        return _miss(producer, _collection_miss_reason(error), moon_task_hash)
    return _identity_from_facts(producer, before, recipes[producer], fingerprint, moon_task_hash)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", required=True, type=Path)
    parser.add_argument("--producer", required=True)
    args = parser.parse_args(argv)
    report = collect_identity_report(args.repo, args.producer)
    sys.stdout.write(json.dumps(report, sort_keys=True, separators=(",", ":")) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
