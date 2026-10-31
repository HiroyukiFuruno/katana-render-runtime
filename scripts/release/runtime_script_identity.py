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
import runtime_script_tool_fingerprint_inputs as pilot


SCHEMA = "krr-runtime-script-identity-v2"
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
    sourceInputsSha256: str | None
    sourceInputScope: str | None
    recipeSha256: str | None
    toolEnvironmentSha256: str | None
    inputSha256: str | None
    identitySha256: str | None
    moonTaskHash: str | None
    diagnostic: dict[str, object] | None


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
        "sourceInputsSha256": None,
        "sourceInputScope": None,
        "recipeSha256": None,
        "toolEnvironmentSha256": None,
        "inputSha256": None,
        "identitySha256": None,
        "moonTaskHash": moon_task_hash,
        "diagnostic": None,
    }


def _run_json(
    repo: Path,
    argv: list[str],
    failure_reason: str,
    env_updates: dict[str, str] | None = None,
) -> object:
    environment = {**os.environ, **(env_updates or {})}
    if any(value and (name.startswith("LD_") or name.startswith("DYLD_"))
           for name, value in environment.items()):
        raise _CollectionFailure("unsupported_loader_override")
    try:
        process = subprocess.run(
            argv,
            cwd=repo,
            capture_output=True,
            check=False,
            env=environment,
        )
    except OSError as error:
        raise _CollectionFailure(failure_reason) from error
    if process.returncode != 0:
        raise _CollectionFailure(failure_reason)
    try:
        return json.loads(process.stdout)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise _CollectionFailure(failure_reason) from error


def _source_state(repo: Path) -> tuple[str, str]:
    try:
        state = coverage_context._source_state(repo)
    except (OSError, ValueError) as error:
        raise _CollectionFailure("source_manifest_unavailable") from error
    source_digest = state.get("sourceDigest")
    source_inputs_digest = state.get("coverageInputDigest")
    if (
        not isinstance(source_digest, str)
        or SHA256_RE.fullmatch(source_digest) is None
        or not isinstance(source_inputs_digest, str)
        or SHA256_RE.fullmatch(source_inputs_digest) is None
    ):
        raise _CollectionFailure("source_manifest_unavailable")
    return source_digest, source_inputs_digest


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


def _python_context() -> dict[str, object]:
    try:
        python_digest = _digest_bytes(Path(sys.executable).resolve(strict=True).read_bytes())
    except OSError as error:
        raise _CollectionFailure("producer_context_unavailable") from error
    return {
        "pythonVersion": sys.version,
        "pythonExecutableSha256": python_digest,
        "pythonEnvironmentSha256": _canonical_digest(
            {name: value for name, value in sorted(os.environ.items()) if name.startswith("PYTHON")}
        ),
        "pythonFlagsSha256": _digest_bytes(repr(sys.flags).encode()),
        "pythonSearchPathSha256": _canonical_digest(sys.path),
    }


def _tool_fingerprint(repo: Path) -> dict[str, object]:
    fingerprint = _run_json(repo, ["bun", TOOL_FINGERPRINT], "tool_fingerprint_unavailable")
    if (
        not isinstance(fingerprint, dict)
        or fingerprint.get("schema") != "krr-runtime-bundle-tool-fingerprint-v1"
        or not isinstance(fingerprint.get("tools"), dict)
        or not isinstance(fingerprint.get("environment"), list)
    ):
        raise _CollectionFailure("tool_fingerprint_shape_unsupported")
    try:
        prepared_digest = _digest_bytes((repo / PREPARED_RUNTIME).read_bytes())
    except OSError as error:
        raise _CollectionFailure("producer_context_unavailable") from error
    return {
        "runtimeBundle": cast(dict[str, object], fingerprint),
        "rustFactsHostFingerprint": _rust_host_fingerprint(repo),
        "mathjaxEvidenceFingerprint": _mathjax_evidence_fingerprint(repo),
        **_python_context(),
        "preparedRuntimeSha256": prepared_digest,
    }


def _moon_context() -> tuple[str | None, str | None]:
    task_id = os.environ.get("MOON_TASK_ID")
    raw_hash = os.environ.get("MOON_TASK_HASH")
    safe_task_id = task_id if task_id is not None and MOON_TASK_ID_RE.fullmatch(task_id) else None
    safe_hash = raw_hash if raw_hash is not None and SHA256_RE.fullmatch(raw_hash) else None
    return safe_task_id, safe_hash


def _pilot_facts(repo: Path) -> tuple[str, dict[str, object]]:
    report = pilot.collect(repo, os.environ)
    digest = report.get("inventorySha256")
    if (report.get("schema") != pilot.SCHEMA or report.get("profile") != pilot.PROFILE
            or report.get("admission") != "candidate-only" or report.get("cacheable") is not False
            or report.get("inputsCollected") is not True or not isinstance(digest, str)
            or not SHA256_RE.fullmatch(digest)):
        raise _CollectionFailure("tool_profile_inputs_unavailable")
    tool = _run_json(repo, ["bun", TOOL_FINGERPRINT], "tool_fingerprint_unavailable")
    if (not isinstance(tool, dict) or tool.get("schema") != "krr-runtime-bundle-tool-fingerprint-v1"
            or not isinstance(tool.get("tools"), dict) or not isinstance(tool.get("environment"), list)):
        raise _CollectionFailure("tool_fingerprint_shape_unsupported")
    components = report.get("componentDigests")
    names = report.get("semanticEnvironmentNames")
    if (not isinstance(components, dict) or not isinstance(names, list)
            or any(not isinstance(name, str) for name in names)
            or any(not isinstance(value, str) or not SHA256_RE.fullmatch(value)
                   for value in components.values())):
        raise _CollectionFailure("tool_profile_diagnostics_unavailable")
    python_context = _python_context()
    return digest, {"scope": pilot.SEMANTIC_SCOPE, "runtimeBundle": tool, **python_context,
                    "diagnostic": {"inputComponents": components, "environmentNames": names,
                                   "runtimeTool": _canonical_digest(tool),
                                   "pythonContext": _canonical_digest(python_context)}}


def _identity_from_facts(
    producer: str,
    source_digest: str,
    source_inputs_digest: str,
    recipe: recipe_reader.RuntimeScriptRecipe,
    fingerprint: dict[str, object],
    moon_task_hash: str | None,
) -> IdentityReport:
    tool_digest = _canonical_digest({key: value for key, value in fingerprint.items() if key != "diagnostic"})
    input_digest = _canonical_digest(
        {
            "schema": (pilot.SEMANTIC_SCOPE if producer == pilot.PROFILE
                       else "krr-runtime-script-producer-input-v2"),
            "producer": producer,
            "sourceInputsSha256": source_inputs_digest,
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
        "sourceInputsSha256": source_inputs_digest,
        "sourceInputScope": (pilot.SEMANTIC_SCOPE if producer == pilot.PROFILE
                             else "krr-native-coverage-source-inputs-v1"),
        "recipeSha256": recipe.recipe_sha256,
        "toolEnvironmentSha256": tool_digest,
        "inputSha256": input_digest,
        "identitySha256": identity_digest,
        "moonTaskHash": moon_task_hash,
        "diagnostic": cast(dict[str, object] | None, fingerprint.get("diagnostic")),
    }


def _collect_facts(
    repo: Path, producers: tuple[str, ...]
) -> tuple[
    str,
    str,
    dict[str, recipe_reader.RuntimeScriptRecipe],
    dict[str, object],
    tuple[str, dict[str, object]] | None,
    str | None,
]:
    if not repo.is_absolute() or repo.resolve(strict=True) != repo:
        raise _CollectionFailure("repository_root_not_canonical")
    source_before, inputs_before = _source_state(repo)
    has_broad_profile = any(producer != pilot.PROFILE for producer in producers)
    tool_before = _tool_fingerprint(repo) if has_broad_profile else {}
    pilot_before = _pilot_facts(repo) if pilot.PROFILE in producers else None
    recipes = {
        producer: recipe_reader.read_runtime_script_recipe(repo, producer)
        for producer in producers
    }
    for producer, recipe in recipes.items():
        if recipe.recipe_name != profiles.producer_recipe(producer):
            raise _CollectionFailure("recipe_profile_mismatch")
    tool_after = _tool_fingerprint(repo) if has_broad_profile else {}
    pilot_after = _pilot_facts(repo) if pilot.PROFILE in producers else None
    source_after, inputs_after = _source_state(repo)
    if (
        source_before != source_after
        or inputs_before != inputs_after
        or tool_before != tool_after
        or pilot_before != pilot_after
    ):
        raise _CollectionFailure("source_changed_during_collection")
    _, moon_task_hash = _moon_context()
    return source_before, inputs_before, recipes, tool_before, pilot_before, moon_task_hash


def _read_execution_facts(
    repo: Path, producer: str
) -> tuple[str, str, recipe_reader.RuntimeScriptRecipe, dict[str, object]]:
    if not repo.is_absolute() or repo.resolve(strict=True) != repo:
        raise _CollectionFailure("repository_root_not_canonical")
    source_digest, source_inputs_digest = _source_state(repo)
    recipe = recipe_reader.read_runtime_script_recipe(repo, producer)
    if recipe.recipe_name != profiles.producer_recipe(producer):
        raise _CollectionFailure("recipe_profile_mismatch")
    if producer == pilot.PROFILE:
        source_inputs_digest, tool = _pilot_facts(repo)
    else:
        tool = _tool_fingerprint(repo)
    return source_digest, source_inputs_digest, recipe, tool


def collect_execution_snapshot(repo: Path, producer: str) -> IdentityReport:
    """Read one execution-side identity snapshot; callers must pair it around a run."""
    if producer not in profiles.accepted_producers():
        return _miss(producer, "unknown_profile")
    _, moon_task_hash = _moon_context()
    try:
        source_digest, source_inputs_digest, recipe, fingerprint = _read_execution_facts(
            repo, producer
        )
    except (OSError, _CollectionFailure, recipe_reader.RuntimeScriptRecipeError) as error:
        return _miss(producer, _collection_miss_reason(error), moon_task_hash)
    return _identity_from_facts(
        producer, source_digest, source_inputs_digest, recipe, fingerprint, moon_task_hash
    )


def collect_fixed_leaf_identity_reports(repo: Path) -> dict[str, IdentityReport]:
    """Collect all fixed leaf identities behind one conservative stability window."""
    producers = tuple(profiles.RUNTIME_SCRIPT_PROFILES)
    try:
        (
            source_digest,
            source_inputs_digest,
            recipes,
            fingerprint,
            pilot_facts,
            moon_task_hash,
        ) = _collect_facts(repo, producers)
    except (OSError, _CollectionFailure, recipe_reader.RuntimeScriptRecipeError) as error:
        reason = _collection_miss_reason(error)
        return {producer: _miss(producer, reason) for producer in producers}
    return {
        producer: _identity_from_facts(
            producer,
            source_digest,
            pilot_facts[0] if producer == pilot.PROFILE and pilot_facts is not None else source_inputs_digest,
            recipes[producer],
            pilot_facts[1] if producer == pilot.PROFILE and pilot_facts is not None else fingerprint,
            moon_task_hash,
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
        (
            before,
            source_inputs_digest,
            recipes,
            fingerprint,
            pilot_facts,
            moon_task_hash,
        ) = _collect_facts(repo, (producer,))
    except (OSError, _CollectionFailure, recipe_reader.RuntimeScriptRecipeError) as error:
        return _miss(producer, _collection_miss_reason(error), moon_task_hash)
    return _identity_from_facts(
        producer, before,
        pilot_facts[0] if pilot_facts is not None else source_inputs_digest,
        recipes[producer], pilot_facts[1] if pilot_facts is not None else fingerprint, moon_task_hash
    )


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
