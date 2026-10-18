"""Create fail-closed local receipts for the existing KRR coverage producers."""

from __future__ import annotations

import hashlib
import importlib
import json
import os
import re
import secrets
import subprocess
import uuid
from pathlib import Path
from typing import cast

import native_coverage_result as coverage_result
import native_coverage_local_anchor as local_anchor


KEY_SCHEMA = "krr-native-local-coverage-key-v1"
RUN_SCHEMA = "krr-native-local-coverage-run-v1"
RECEIPT_SCHEMA = "krr-native-local-coverage-receipt-v1"
LOCAL_CONTEXT_SCHEMA = "krr-native-local-coverage-context-v1"
LOCAL_HOST_KEYS = {"os", "productVersion", "buildVersion", "architecture"}
LOCAL_TOOL_KEYS = {
    "rustc",
    "cargo",
    "cargoLlvmCov",
    "llvm",
    "java",
    "bun",
    "node",
    "python",
    "graphviz",
    "plantuml",
}
AUTHENTICITY = "unsigned-local-moon-cache"
SHA256_RE = re.compile(r"[0-9a-f]{64}\Z")
SAFE_REASON_RE = re.compile(r"[a-z0-9_]{1,80}\Z")
PRODUCERS = {
    "root": ("root",),
    "performance": ("performance-baseline", "performance-candidate"),
}
REPORT_NAMES = {
    "root": "root.json",
    "performance-baseline": "baseline.json",
    "performance-candidate": "candidate.json",
}

JsonObject = dict[str, object]


class LocalCoverageError(ValueError):
    """Raised when a local coverage producer receipt cannot be trusted."""


def _json_bytes(value: object) -> bytes:
    return (
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
        + "\n"
    ).encode()


def _digest(value: object) -> str:
    canonical = json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=True
    ).encode()
    return hashlib.sha256(canonical).hexdigest()


def _producer_domains(producer: object) -> tuple[str, ...]:
    if not isinstance(producer, str) or producer not in PRODUCERS:
        raise LocalCoverageError("producer must be root or performance")
    return PRODUCERS[producer]


def _source_digest(repo: Path) -> str:
    script = Path(__file__).resolve().parents[1] / "mermaid/native-source-manifest.ts"
    try:
        result = subprocess.run(
            ["bun", "run", str(script), "--root", str(repo)],
            cwd=repo,
            text=True,
            capture_output=True,
            check=False,
        )
    except OSError as error:
        raise LocalCoverageError(
            "canonical native source manifest command is unavailable"
        ) from error
    if result.returncode != 0:
        diagnostic = result.stderr.strip()
        truncated = len(diagnostic) > 2048
        if truncated:
            diagnostic = diagnostic[-2048:]
        suffix = " [stderr tail truncated]" if truncated else ""
        raise LocalCoverageError(
            f"canonical native source manifest command failed with exit {result.returncode}: "
            f"{diagnostic}{suffix}"
        )
    try:
        manifest = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise LocalCoverageError(
            "canonical native source manifest is invalid JSON"
        ) from error
    if (
        not isinstance(manifest, dict)
        or set(manifest) != {"schema", "digest", "files"}
        or manifest.get("schema") != "krr-native-build-source-manifest-v1"
        or not isinstance(manifest.get("files"), list)
        or not manifest["files"]
        or not isinstance(manifest.get("digest"), str)
        or SHA256_RE.fullmatch(manifest["digest"]) is None
    ):
        raise LocalCoverageError(
            "canonical native source manifest has an invalid shape"
        )
    return cast(str, manifest["digest"])


def _local_context(repo: Path) -> JsonObject:
    try:
        module = importlib.import_module("native_coverage_local_context")
        context = module.collect_local_context(repo)
    except (ImportError, OSError, ValueError) as error:
        raise LocalCoverageError(
            "live local coverage context is unavailable"
        ) from error
    if (
        not isinstance(context, dict)
        or set(context)
        != {
            "schema",
            "cacheable",
            "missReasons",
            "host",
            "tools",
            "environmentSha256",
            "resourcesSha256",
            "hostContextSha256",
        }
        or context.get("schema") != LOCAL_CONTEXT_SCHEMA
        or type(context.get("cacheable")) is not bool
        or not isinstance(context.get("missReasons"), list)
        or any(
            not isinstance(item, str) or SAFE_REASON_RE.fullmatch(item) is None
            for item in context["missReasons"]
        )
    ):
        raise LocalCoverageError("live local coverage context has an invalid schema")
    host = context.get("host")
    tools = context.get("tools")
    if (
        not isinstance(host, dict)
        or set(host) != LOCAL_HOST_KEYS
        or any(not isinstance(value, str) or not value for value in host.values())
        or not isinstance(tools, dict)
        or set(tools) != LOCAL_TOOL_KEYS
        or any(not isinstance(value, str) or not value for value in tools.values())
    ):
        raise LocalCoverageError(
            "live local coverage context has invalid host or tool identity"
        )
    for key in ("environmentSha256", "resourcesSha256", "hostContextSha256"):
        if (
            not isinstance(context.get(key), str)
            or SHA256_RE.fullmatch(context[key]) is None
        ):
            raise LocalCoverageError(
                f"live local coverage context has an invalid {key}"
            )
    if context["cacheable"] is True and context["missReasons"]:
        raise LocalCoverageError("cacheable local context contains miss reasons")
    if context["cacheable"] is False and not context["missReasons"]:
        raise LocalCoverageError(
            "non-cacheable local context must explain the cache miss"
        )
    digest_input = {
        key: value for key, value in context.items() if key != "hostContextSha256"
    }
    if context["hostContextSha256"] != _digest(digest_input):
        raise LocalCoverageError(
            "live local coverage context digest does not match its fields"
        )
    return cast(JsonObject, context)


def _threads() -> str:
    value = os.environ.get("TEST_THREADS", "1")
    if value == "":
        return "default"
    if value != "default" and (not value.isdecimal() or int(value) < 1):
        raise LocalCoverageError("TEST_THREADS must be positive or empty")
    return value


def _fixed_coverage_policy() -> None:
    if os.environ.get("COVERAGE_MIN_LINES", "100") != "100":
        raise LocalCoverageError("COVERAGE_MIN_LINES must remain 100")
    if os.environ.get("COVERAGE_MAX_UNCOVERED_LINES", "0") != "0":
        raise LocalCoverageError("COVERAGE_MAX_UNCOVERED_LINES must remain 0")


def _commands(repo: Path, producer: str) -> list[JsonObject]:
    threads = _threads()
    return [
        {
            "domain": domain,
            **coverage_result._domain_command(
                repo,
                domain,
                REPORT_NAMES[domain],
                threads,
            ),
        }
        for domain in _producer_domains(producer)
    ]


def _execution_environment_digest() -> str:
    keys = (
        "CARGO",
        "RTK",
        "RUSTFLAGS",
        "TEST_THREADS",
        "CARGO_BUILD_JOBS",
        "CARGO_HOME",
        "CARGO_TARGET_DIR",
        "CARGO_LLVM_COV_TARGET_DIR",
    )
    values = {key: os.environ.get(key) for key in keys}
    return _digest(values)


def _identity(repo: Path, producer: str) -> JsonObject:
    _producer_domains(producer)
    _fixed_coverage_policy()
    source_digest = _source_digest(repo)
    context = _local_context(repo)
    command_rows = _commands(repo, producer)
    context_digest = cast(str, context["hostContextSha256"])
    payload: JsonObject = {
        "schema": KEY_SCHEMA,
        "producer": producer,
        "sourceDigest": source_digest,
        "contextDigest": context_digest,
        "environmentDigest": _execution_environment_digest(),
        "commands": command_rows,
        "coveragePolicy": {"linePercent": 100, "unmeasuredLines": 0},
    }
    return {
        **payload,
        "identitySha256": _digest(payload),
        "cacheable": context["cacheable"],
    }


def _bound_identity(repo: Path, producer: str) -> JsonObject:
    base = _identity(repo, producer)
    base_digest = cast(str, base["identitySha256"])
    epoch = (
        local_anchor.prepare(repo, producer, base_digest)
        if base["cacheable"] is True
        else None
    )
    payload = {
        key: value
        for key, value in base.items()
        if key not in {"identitySha256", "cacheable"}
    }
    payload.update(
        {
            "baseIdentitySha256": base_digest,
            "anchorEpoch": epoch,
            "cacheable": base["cacheable"] is True and epoch is not None,
        }
    )
    return {**payload, "identitySha256": _digest(payload)}


def _paths(repo: Path, producer: str) -> dict[str, Path]:
    stem = producer
    directory = repo / coverage_result.REPORT_DIRECTORY
    return {
        "probe": directory / f"{stem}-local-probe.json",
        "run": directory / f"{stem}-local-run.json",
        "receipt": directory / f"{stem}-local-result.json",
    }


def _ensure_report_directory(repo: Path) -> None:
    cursor = repo
    for part in Path(coverage_result.REPORT_DIRECTORY).parts:
        cursor /= part
        try:
            cursor.mkdir(mode=0o700)
        except FileExistsError:
            pass
        except OSError as error:
            raise LocalCoverageError(
                "local coverage output directory is unavailable"
            ) from error
        try:
            cursor.lstat()
        except OSError as error:
            raise LocalCoverageError(
                "local coverage output directory is unavailable"
            ) from error
        if cursor.is_symlink() or not cursor.is_dir():
            raise LocalCoverageError(
                "local coverage output path must contain only real directories"
            )


def _read_json(path: Path, label: str) -> object:
    try:
        if path.is_symlink() or not path.is_file():
            raise LocalCoverageError(f"{label} must be a regular non-symlink file")
        return json.loads(path.read_bytes())
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise LocalCoverageError(f"{label} is unavailable or invalid JSON") from error


def _write_atomic(path: Path, value: object) -> None:
    directory = path.parent
    if directory.is_symlink() or not directory.is_dir():
        raise LocalCoverageError(
            "local coverage output directory must be a real directory"
        )
    if path.is_symlink():
        raise LocalCoverageError("local coverage output must not be a symlink")
    temporary = path.with_name(f".{path.name}.{uuid.uuid4().hex}.tmp")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(temporary, flags, 0o600)
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(_json_bytes(value))
        os.replace(temporary, path)
    finally:
        if temporary.exists():
            temporary.unlink()


def local_key(repo: Path, producer: str) -> JsonObject:
    identity = _bound_identity(repo, producer)
    fingerprint = cast(str, identity["identitySha256"])
    if identity["cacheable"] is False:
        _ensure_report_directory(repo)
        token = secrets.token_hex(32)
        fingerprint = token
    else:
        _ensure_report_directory(repo)
        token = None
    _write_atomic(
        _paths(repo, producer)["probe"],
        {
            "schema": "krr-native-local-coverage-probe-v1",
            "producer": producer,
            "identitySha256": identity["identitySha256"],
            "cacheable": identity["cacheable"],
            "token": token,
        },
    )
    return {
        "schema": KEY_SCHEMA,
        "producer": producer,
        "cacheable": identity["cacheable"],
        "fingerprint": fingerprint,
        "sourceDigest": identity["sourceDigest"],
        "contextDigest": identity["contextDigest"],
        "commandDigest": _digest(identity["commands"]),
    }


def _read_key_probe(repo: Path, producer: str, identity: JsonObject) -> str | None:
    probe = _read_json(_paths(repo, producer)["probe"], "local probe token")
    if (
        not isinstance(probe, dict)
        or set(probe) != {"schema", "producer", "identitySha256", "cacheable", "token"}
        or probe.get("schema") != "krr-native-local-coverage-probe-v1"
        or probe.get("producer") != producer
        or probe.get("identitySha256") != identity["identitySha256"]
        or probe.get("cacheable") is not identity["cacheable"]
    ):
        raise LocalCoverageError(
            "current non-cacheable probe token does not match this run"
        )
    token = probe.get("token")
    if identity["cacheable"] is True:
        if token is not None:
            raise LocalCoverageError(
                "cacheable key probe unexpectedly contains a token"
            )
        return None
    if not isinstance(token, str) or SHA256_RE.fullmatch(token) is None:
        raise LocalCoverageError("non-cacheable key probe token is invalid")
    return token


def _probe_token(repo: Path, producer: str, identity: JsonObject) -> str | None:
    if identity["cacheable"] is True:
        return None
    return _read_key_probe(repo, producer, identity)


def local_begin(repo: Path, producer: str) -> JsonObject:
    _ensure_report_directory(repo)
    identity = _bound_identity(repo, producer)
    moon_managed = "MOON_TASK_ID" in os.environ or "MOON_TASK_HASH" in os.environ
    probe_path = _paths(repo, producer)["probe"]
    if not moon_managed:
        token = secrets.token_hex(32) if identity["cacheable"] is False else None
        _write_atomic(
            probe_path,
            {
                "schema": "krr-native-local-coverage-probe-v1",
                "producer": producer,
                "identitySha256": identity["identitySha256"],
                "cacheable": identity["cacheable"],
                "token": token,
            },
        )
    else:
        token = _read_key_probe(repo, producer, identity)
    run: JsonObject = {
        "schema": RUN_SCHEMA,
        "producer": producer,
        "runId": uuid.uuid4().hex,
        "probeToken": token,
        "identity": identity,
    }
    _write_atomic(_paths(repo, producer)["run"], run)
    return {"schema": RUN_SCHEMA, "producer": producer, "started": True}


def _report_records(repo: Path, producer: str) -> list[JsonObject]:
    rows = []
    threads = _threads()
    for domain in _producer_domains(producer):
        rows.append(
            {"domain": domain, **coverage_result._report_record(repo, domain, threads)}
        )
    return rows


def _validate_identity(
    receipt: object, identity: JsonObject, producer: str, token: str | None
) -> JsonObject:
    row = coverage_result._object(receipt, "local coverage receipt")
    required = {
        "schema",
        "authenticity",
        "producer",
        "producerExit",
        "cacheable",
        "probeToken",
        "identity",
        "domains",
    }
    if set(row) != required or row.get("schema") != RECEIPT_SCHEMA:
        raise LocalCoverageError("local coverage receipt fields or schema differ")
    if (
        row.get("authenticity") != AUTHENTICITY
        or row.get("producer") != producer
        or type(row.get("producerExit")) is not int
        or row["producerExit"] != 0
        or row.get("cacheable") is not identity["cacheable"]
        or row.get("probeToken") != token
        or row.get("identity") != identity
    ):
        raise LocalCoverageError(
            "local coverage receipt identity or producer result differs"
        )
    return row


def local_finish(repo: Path, producer: str, producer_exit: int) -> JsonObject:
    """Trust the fixed Just recipe's exit argument; this is not an adversarial-caller attestation."""
    _ensure_report_directory(repo)
    if type(producer_exit) is not int or producer_exit != 0:
        raise LocalCoverageError("coverage producer did not exit successfully")
    run = coverage_result._object(
        _read_json(_paths(repo, producer)["run"], "local run"), "local run"
    )
    identity = _bound_identity(repo, producer)
    token = _probe_token(repo, producer, identity)
    if (
        set(run) != {"schema", "producer", "runId", "probeToken", "identity"}
        or run.get("schema") != RUN_SCHEMA
        or run.get("producer") != producer
        or run.get("identity") != identity
        or run.get("probeToken") != token
    ):
        raise LocalCoverageError("producer inputs changed between begin and finish")
    receipt: JsonObject = {
        "schema": RECEIPT_SCHEMA,
        "authenticity": AUTHENTICITY,
        "producer": producer,
        "producerExit": 0,
        "cacheable": identity["cacheable"],
        "probeToken": token,
        "identity": identity,
        "domains": _report_records(repo, producer),
    }
    _write_atomic(_paths(repo, producer)["receipt"], receipt)
    if identity["anchorEpoch"] is not None:
        receipt_bytes = _paths(repo, producer)["receipt"].read_bytes()
        if not local_anchor.commit(
            repo,
            producer,
            cast(str, identity["baseIdentitySha256"]),
            cast(str, identity["anchorEpoch"]),
            receipt_bytes,
        ):
            raise LocalCoverageError(
                "trusted local coverage anchor could not be committed"
            )
    return {
        "schema": RECEIPT_SCHEMA,
        "created": True,
        "cacheable": identity["cacheable"],
    }


def local_validate(repo: Path, producer: str) -> JsonObject:
    _ensure_report_directory(repo)
    identity = _bound_identity(repo, producer)
    token = _probe_token(repo, producer, identity)
    receipt_path = _paths(repo, producer)["receipt"]
    if identity["anchorEpoch"] is not None:
        try:
            receipt_bytes = receipt_path.read_bytes()
        except OSError:
            receipt_bytes = b""
        if not local_anchor.verify(
            repo,
            producer,
            cast(str, identity["baseIdentitySha256"]),
            cast(str, identity["anchorEpoch"]),
            receipt_bytes,
        ):
            local_anchor.invalidate(
                repo,
                producer,
                cast(str, identity["baseIdentitySha256"]),
                cast(str, identity["anchorEpoch"]),
            )
            raise LocalCoverageError(
                "trusted local coverage anchor does not match receipt bytes"
            )
    try:
        _validate_receipt_reports(repo, producer, identity, token, receipt_path)
    except (LocalCoverageError, coverage_result.CoverageResultError):
        if identity["anchorEpoch"] is not None:
            local_anchor.invalidate(
                repo,
                producer,
                cast(str, identity["baseIdentitySha256"]),
                cast(str, identity["anchorEpoch"]),
            )
        raise
    return {
        "schema": RECEIPT_SCHEMA,
        "valid": True,
        "cacheable": identity["cacheable"],
        "producer": producer,
    }


def _validate_receipt_reports(
    repo: Path,
    producer: str,
    identity: JsonObject,
    token: str | None,
    receipt_path: Path,
) -> JsonObject:
    receipt = _validate_identity(
        _read_json(receipt_path, "local receipt"), identity, producer, token
    )
    if receipt.get("domains") != _report_records(repo, producer):
        raise LocalCoverageError(
            "coverage reports changed or no longer satisfy strict 100/0 totals"
        )
    return receipt
