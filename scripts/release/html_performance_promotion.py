"""正式なpaired HTML性能証跡から改善の有意性を判定する。"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
from datetime import UTC, datetime
from pathlib import Path
from typing import Never, NamedTuple

import html_performance_paired_stats as statistics
import html_performance_python_identity as python_identity
import html_performance_retained_runs as retention
import html_performance_source_reuse_cli as reuse
import run_html_performance as runner
import verify_html_performance_evidence as gate


class PromotionResult(NamedTuple):
    record_id: str
    status: str


def _runtime_identity(root: Path) -> str:
    gate.require(root == runner.ROOT, "promotion repository differs from bootstrap root")
    runner._require_private_venv_path()
    runner._verify_prepared_python()
    gate.require(Path(sys.executable).absolute() == runner._venv_python().absolute()
                 and Path(sys.prefix).resolve(strict=True) == runner.VENV.resolve(strict=True),
                 "promotion Python is outside the prepared environment")
    uv_path = os.environ.get(runner.UV_PATH_ENV)
    gate.require(bool(uv_path) and runner._file_sha256(Path(uv_path or "")) == os.environ.get(runner.UV_ENV),
                 "promotion bootstrap tool identity differs")
    identity = python_identity.installed_identity(runner.SCIPY_VERSION, runner.NUMPY_VERSION)
    gate.require(identity == os.environ.get(runner.IDENTITY_ENV)
                 and os.environ.get(runner.VERSION_ENV) == runner.SCIPY_VERSION,
                 "promotion scientific runtime identity differs")
    return identity


def _compare(baseline: list[float], candidate: list[float]) -> bool:
    gate.require(not statistics.evaluate_paired_regression(baseline, candidate).detected_bad,
                 "promotion detected a regression")
    return statistics.evaluate_paired_regression(candidate, baseline).detected_bad


def _measure(bundle: gate.Bundle, result: gate.JsonObject) -> tuple[bool, gate.JsonObject]:
    gate.require(result.get("candidateConditions") == 84
                 and result.get("pairedConditions") == 84
                 and result.get("nonComparableConditions") == 0
                 and result.get("performanceNonregression") is True,
                 "promotion requires all 84 accepted paired conditions")
    attempts = gate._objects(bundle.manifest["attempts"], "promotion attempts")
    gate.require(len(attempts) == 84, "promotion attempt inventory differs")
    improvement = False
    rss_nonregression = True
    elapsed: dict[str, list[float]] = {phase: [] for phase in gate.PHASES}
    rss: dict[str, list[float]] = {phase: [] for phase in gate.PHASES}
    for attempt in attempts:
        expected = bundle.json(attempt["expected"])
        pins = gate._objects(expected["reportPins"], "promotion report pins")
        grouped = gate._group_reports([bundle.file(pin) for pin in pins], expected)
        for phase in gate.PHASES:
            for index in range(2):
                samples = {
                    arm: [gate._phase_sample(sample, phase)[index]
                          for pair in range(5)
                          for sample in gate._objects(grouped[arm][pair]["samples"], "samples")]
                    for arm in ("baseline", "candidate")
                }
                baseline, candidate = samples["baseline"], samples["candidate"]
                memory_policy_version = result.get("memoryPolicyVersion")
                if index == 0 or memory_policy_version not in (
                    "html-memory-v2", "html-memory-v3", "html-memory-v4",
                ):
                    improvement |= _compare(baseline, candidate)
                else:
                    rss_nonregression &= not statistics.evaluate_paired_regression(
                        baseline, candidate,
                    ).detected_bad
                    # WHY: V4受入で許容したRSS増加でも、基準昇格では全RSSの非劣化を求める。
                    improvement |= statistics.evaluate_paired_regression(
                        candidate, baseline,
                    ).detected_bad
                (elapsed if index == 0 else rss)[phase].extend(candidate)
    phases: gate.JsonObject = {}
    for phase in gate.PHASES:
        gate.require(len(elapsed[phase]) == len(rss[phase]) == 2100,
                     "promotion candidate sample inventory differs")
        gate.require(all(value.is_integer() for value in rss[phase]), "RSS is not integral")
        phases[phase] = {
            "maxSampleSeconds": max(elapsed[phase]) / 1000,
            "medianSeconds": gate._percentile(elapsed[phase], 0.5) / 1000,
            "p95Seconds": gate._percentile(elapsed[phase], 0.95) / 1000,
            "maxPeakRssBytes": int(max(rss[phase])),
        }
    return improvement and rss_nonregression, phases


def _verified_reuse(request: Path, root: Path, directory: Path, manifest_sha: str,
                    request_sha: str) -> gate.JsonObject:
    result = reuse.verify_reuse_request(
        request, root, expected_measured_dir=directory,
        expected_measured_manifest_sha256=manifest_sha, expected_request_sha256=request_sha,
        memory_policy_version="html-memory-v4",
    )
    gate.require(result.get("reuseEligibility") is True, "performance reuse is not eligible")
    return result


def improvement_established(directory: Path, root: Path, *, reuse_request: Path | None = None) -> bool:
    """正式な84条件の検証後、同じ補正で改善方向も判定する。"""
    root = root.resolve(strict=True)
    runtime_identity = _runtime_identity(root)
    manifest_sha = gate.sha256(directory / "bundle.json")
    directory = directory.resolve(strict=True)
    request_sha = gate.sha256(reuse_request) if reuse_request is not None else None
    eligibility = (_verified_reuse(reuse_request, root, directory, manifest_sha, request_sha)
                   if reuse_request is not None and request_sha is not None else None)
    result = (gate._object(eligibility.get("measuredResult"), "reused measured result")
              if eligibility is not None else
              gate.verify(directory, root, memory_policy_version="html-memory-v4"))
    gate._require_cli_memory_v4(result, reused=False)
    gate.require(gate.sha256(directory / "bundle.json") == manifest_sha,
                 "promotion bundle changed during verification")
    improvement, _ = _measure(gate.Bundle(directory), result)
    if reuse_request is not None and request_sha is not None:
        gate.require(_verified_reuse(reuse_request, root, directory, manifest_sha, request_sha) == eligibility,
                     "performance reuse changed during comparison")
    gate.require(gate.sha256(directory / "bundle.json") == manifest_sha,
                 "promotion bundle changed during comparison")
    gate.require(_runtime_identity(root) == runtime_identity,
                 "promotion runtime identity changed")
    return improvement


def _schema_validate(root: Path, payload: bytes) -> None:
    program = ('import {parsePerformanceHistory} from "./scripts/html/performance-history.ts";'
               'parsePerformanceHistory(JSON.parse(await Bun.stdin.text()));')
    subprocess.run(["bun", "-e", program], cwd=root, input=payload, check=True,
                   stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)


def _candidate_pins(bundle: gate.Bundle) -> gate.JsonObject:
    first = gate._objects(bundle.manifest["attempts"], "promotion attempts")[0]
    expected = bundle.json(first["expected"])
    candidate = gate._object(gate._object(expected["arms"], "arms")["candidate"], "candidate")
    build_ref = gate._object(candidate["build_artifact_ref"], "build artifact")
    proof = bundle.json(build_ref)
    before = gate._object(gate._object(expected["endpointRefs"], "endpoints")["before"], "before")
    snapshot = gate._object(proof["sourceSnapshotRef"], "source snapshot")
    return {
        "schema": "krr-html-performance-reference-build-pins-v1",
        "buildArtifactSchema": "krr-html-performance-build-artifact-v3",
        "buildArtifactSha256": build_ref["sha256"],
        "sourceSnapshotSha256": snapshot["sha256"],
        "binarySha256": candidate["binary_sha256"],
        "lockSha256": candidate["lock_sha256"],
        "cargoExecutableSha256": gate._object(proof["cargoExecutable"], "cargo")["sha256"],
        "rustcExecutableSha256": gate._object(proof["rustcExecutable"], "rustc")["sha256"],
        "contextBeforeSha256": before["sha256"],
    }


def _memory_policy_record(bundle: gate.Bundle, result: gate.JsonObject) -> gate.JsonObject:
    version = result.get("memoryPolicyVersion")
    if version is None:
        return {}
    attempts = gate._objects(bundle.manifest["attempts"], "memory policy attempts")
    gate.require(len(attempts) == 84 and result.get("retainedMemoryConditions") == 84,
                 "promotion requires all retained-memory conditions")
    if version == "html-memory-v2":
        policies = [bundle.json(attempt["result"])["memoryPolicy"] for attempt in attempts]
        references = [bundle.json(attempt["expected"])["memoryRetentionRef"] for attempt in attempts]
        for reference in references:
            bundle.file(reference)
        return {"memoryPolicy": {
            "version": "html-memory-v2", "policySha256": gate._v3_json_digest(policies),
            "retentionEvidenceSha256": gate._v3_json_digest(references),
            "candidateConditions": 84, "acceptedConditions": 84,
            "warmupCycles": 5, "measuredCycles": 25,
        }}
    gate.require(version in ("html-memory-v3", "html-memory-v4"), "promotion memory policy version differs")
    assessments = gate._objects(result["memoryPolicyAssessments"], "memory policy assessments")
    gate.require(len(assessments) == 84, "memory policy assessment count differs")
    fields = {"conditionId", "version", "policySha256", "retentionSha256",
              "originalPolicyVersion", "originalPolicySha256"}
    original_versions = (("html-memory-v2", "html-memory-v3")
                         if version == "html-memory-v3" else
                         ("html-memory-v2", "html-memory-v3", "html-memory-v4"))
    condition_ids: list[str] = []
    for assessment in assessments:
        gate.require(set(assessment) == fields
                     and assessment.get("version") == version
                     and assessment.get("originalPolicyVersion") in original_versions,
                     "memory policy assessment fields differ")
        condition_ids.append(gate._text(assessment.get("conditionId"), "condition id"))
        for key in ("policySha256", "retentionSha256", "originalPolicySha256"):
            gate.digest(assessment.get(key))
    gate.require(len(condition_ids) == len(set(condition_ids)),
                 "memory policy assessment conditions are duplicated")
    retention = sorted(
        ({"conditionId": gate._text(item["conditionId"], "condition id"),
          "sha256": gate._text(item["retentionSha256"], "retention SHA256")}
         for item in assessments),
        key=lambda item: gate._text(item["conditionId"], "condition id"),
    )
    return {"memoryPolicy": {
        "version": version, "policySha256": gate._v3_json_digest(assessments),
        "retentionEvidenceSha256": gate._v3_json_digest(retention),
        "candidateConditions": 84, "acceptedConditions": 84,
        "warmupCycles": 5, "measuredCycles": 25,
    }}


def _record(bundle: gate.Bundle, result: gate.JsonObject, phases: gate.JsonObject,
            run_id: str, manifest_sha: str, stdout_sha: str, summary_sha: str,
            completed_at: str) -> gate.JsonObject:
    stage = gate._object(result["htmlStage90"], "HTML acceptance")
    gate.require(stage.get("acceptance") is True
                 and stage.get("fontIdentityEquivalence") == "established",
                 "HTML acceptance is not established")
    head = gate._text(result["head"], "verified HEAD")
    gate.require(head == bundle.manifest["head"], "verified bundle HEAD differs")
    return {
        "id": f"html84-{head[:8]}-{run_id}", "sourceCommit": head,
        "verificationCompletedAtUtc": completed_at,
        "bundleManifestSha256": manifest_sha, "referenceBuild": _candidate_pins(bundle),
        "scope": {"catalog": "catalog-stage90", "candidateConditions": 84,
                  "pairedConditions": 84, "candidateOnlyConditions": 0,
                  "trialsPerConditionPerPhase": 25, "candidateSamplesPerPhase": 2100,
                  "candidateAndBaselineSamplesPerPhase": 4200, "phaseRecords": 25200,
                  "checks": result["checks"],
                  "aggregation": "candidate samples pooled across all 84 conditions; no-op phases included"},
        "measuredAcceptance": {"verifierRun": run_id, "verifierExitCode": 0,
                               "performanceNonregression": True,
                               "htmlMinimumScore": stage["minimumScore"],
                               "htmlScoreThreshold": stage["threshold"],
                               "outputFontIdentityEquivalence": "established"},
        "candidatePhaseStatistics": phases,
        "evidence": {"summaryRun": run_id, "summarySha256": summary_sha,
                     "verifierStdoutSha256": stdout_sha},
        "reuseStatus": "historical-result; current-source eligibility must be verified separately",
        **_memory_policy_record(bundle, result),
    }


def promote(directory: Path, root: Path, evidence_directory: Path, run_id: str, *,
            reuse_request: Path | None = None) -> PromotionResult:
    """正式な84条件の結果を履歴へ追加し、有意改善時だけ現行基準を更新する。"""
    from produce_html_performance_evidence import candidate_build_locks

    root = root.resolve(strict=True)
    with candidate_build_locks(root):
        return _promote(directory, root, evidence_directory, run_id, reuse_request=reuse_request)


def _promote(directory: Path, root: Path, evidence_directory: Path, run_id: str, *,
             reuse_request: Path | None) -> PromotionResult:
    execution_environment = dict(os.environ)
    runtime_identity = _runtime_identity(root)
    directory = directory.resolve(strict=True)
    gate.require(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]*", run_id) is not None,
                 "promotion run id is invalid")
    gate.require(not any((directory / name).exists() or (directory / name).is_symlink()
                         for name in ("reuse-request.json", "acquisition.json")),
                 "promotion requires a local original bundle")
    history_path = root / "docs/performance/html-performance-history.json"
    gate.require(history_path.is_file() and not history_path.is_symlink()
                 and history_path.resolve() == history_path, "history is not a regular file")
    evidence_parent = evidence_directory.parent.resolve(strict=True)
    gate.require(evidence_parent.is_relative_to((root / "tmp").resolve(strict=True))
                 and not evidence_parent.is_relative_to(directory),
                 "promotion evidence must use a private temporary directory")
    evidence_directory = evidence_parent / evidence_directory.name
    before = history_path.read_bytes()
    head = gate.git(root, "rev-parse", "HEAD").strip()
    committed = gate.git(root, "show", f"{head}:docs/performance/html-performance-history.json").encode()
    gate.require(before == committed, "history differs from the measured commit")
    history = json.loads(before, object_pairs_hook=gate.unique_object)
    gate.require(isinstance(history, dict), "history is invalid")
    _schema_validate(root, before)
    manifest_sha = gate.sha256(directory / "bundle.json")
    request_sha = gate.sha256(reuse_request) if reuse_request is not None else None
    eligibility = (_verified_reuse(reuse_request, root, directory, manifest_sha, request_sha)
                   if reuse_request is not None and request_sha is not None else None)
    gate.require(dict(os.environ) == execution_environment, "promotion environment changed before verification")
    evidence_directory.mkdir(mode=0o700, parents=False, exist_ok=False)
    evidence_directory.chmod(0o700)
    gate.require(not evidence_directory.is_symlink()
                 and evidence_directory.stat().st_mode & 0o777 == 0o700,
                 "promotion evidence directory is not private")
    stdout_path = evidence_directory / "verifier.stdout"
    stderr_path = evidence_directory / "verifier.stderr"
    argv = [sys.executable, str(root / "scripts/release/verify_html_performance_evidence.py"),
            "--repo", str(root), "--evidence-dir", str(directory),
            "--require-memory-policy-v4"]
    if reuse_request is not None:
        argv.extend(["--reuse-request", str(reuse_request)])
    with (evidence_directory / "command.json").open("x", encoding="utf8") as command:
        json.dump(argv, command)
    with stdout_path.open("xb") as stdout, stderr_path.open("xb") as stderr:
        result = subprocess.run(argv, cwd=root, stdout=stdout, stderr=stderr, check=False,
                                env=execution_environment.copy())
    completed_at = datetime.now(UTC).isoformat().replace("+00:00", "Z")
    with (evidence_directory / "exit.json").open("x", encoding="utf8") as exit_file:
        json.dump({"exitCode": result.returncode, "completedAtUtc": completed_at}, exit_file)
    gate.require(result.returncode == 0, "formal performance verification failed")
    gate.require(dict(os.environ) == execution_environment, "promotion environment changed during verification")
    gate.require(gate.sha256(directory / "bundle.json") == manifest_sha,
                 "promotion bundle changed during verification")
    verified = gate.read_json(stdout_path)
    if eligibility is not None:
        gate.require(verified == eligibility, "performance reuse verification differs")
        verified = gate._object(verified.get("measuredResult"), "reused measured result")
    gate._require_cli_memory_v4(verified, reused=False)
    bundle = gate.Bundle(directory)
    improvement, phases = _measure(bundle, verified)
    gate.require(dict(os.environ) == execution_environment, "promotion environment changed during comparison")
    gate.require(_runtime_identity(root) == runtime_identity,
                 "promotion runtime identity changed during comparison")
    gate.require(gate.sha256(directory / "bundle.json") == manifest_sha,
                 "promotion bundle changed during comparison")
    summary_path = evidence_directory / "summary.json"
    with summary_path.open("xb") as summary:
        summary.write((json.dumps({"schema": "krr-html-performance-promotion-summary-v1",
                                   "bundleManifestSha256": manifest_sha,
                                   "candidatePhaseStatistics": phases,
                                   "significantImprovement": improvement},
                                  sort_keys=True, separators=(",", ":")) + "\n").encode())
    stdout_sha, summary_sha = gate.sha256(stdout_path), gate.sha256(summary_path)
    record = _record(bundle, verified, phases, run_id, manifest_sha,
                     stdout_sha, summary_sha, completed_at)
    records = gate._objects(history["records"], "history records")
    gate.require(all(item.get("id") != record["id"] for item in records), "record already exists")
    history["records"] = [*records, record]
    if "memoryPolicy" in record:
        history["schema"] = "krr-html-performance-history-v2"
    if improvement:
        history["activeReferenceRecordId"] = record["id"]
        history["activeReferenceSelectionNote"] = "正式な同一条件の比較で有意な改善と非劣化を確認。"
    after = (json.dumps(history, ensure_ascii=False, indent=2) + "\n").encode()
    _schema_validate(root, after)
    gate.require(gate.sha256(directory / "bundle.json") == manifest_sha
                 and gate.sha256(stdout_path) == stdout_sha
                 and gate.sha256(summary_path) == summary_sha,
                 "promotion evidence changed before update")
    gate.require(_runtime_identity(root) == runtime_identity,
                 "promotion runtime identity changed")
    if reuse_request is not None and request_sha is not None:
        gate.require(_verified_reuse(reuse_request, root, directory, manifest_sha, request_sha) == eligibility,
                     "performance reuse changed before update")
    gate.require(dict(os.environ) == execution_environment, "promotion environment changed before update")
    if improvement:
        retention.stage_reference_change(root, directory / "bundle.json", manifest_sha)
    _atomic_history_update(root, history_path, before, after, head)
    if improvement:
        retention.reconcile_reference(root)
    return PromotionResult(gate._text(record["id"], "new reference id"),
                           "promoted" if improvement else "recorded")


def _atomic_history_update(root: Path, history_path: Path, before: bytes, after: bytes, head: str) -> None:
    lock = history_path.with_name(history_path.name + ".promotion.lock")
    descriptor = os.open(lock, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    temporary: Path | None = None
    try:
        gate.require(not history_path.is_symlink() and history_path.read_bytes() == before
                     and gate.git(root, "rev-parse", "HEAD").strip() == head,
                     "history or HEAD changed during promotion")
        mode = history_path.stat().st_mode & 0o777
        with tempfile.NamedTemporaryFile(dir=history_path.parent, prefix=".history-promotion-",
                                         delete=False) as file:
            temporary = Path(file.name)
            os.fchmod(file.fileno(), mode)
            file.write(after)
            file.flush()
            os.fsync(file.fileno())
        gate.require(not history_path.is_symlink() and history_path.read_bytes() == before
                     and gate.git(root, "rev-parse", "HEAD").strip() == head,
                     "history or HEAD changed before replacement")
        os.replace(temporary, history_path)
        temporary = None
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)
        os.close(descriptor)
        lock.unlink()


class _ArgumentParser(argparse.ArgumentParser):
    def error(self, _message: str) -> Never:
        raise ValueError("invalid promotion arguments")


def main() -> int:
    parser = _ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--evidence-dir", type=Path, required=True)
    parser.add_argument("--promotion-evidence-dir", type=Path, required=True)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--reuse-request", type=Path)
    try:
        args = parser.parse_args()
        result = promote(args.evidence_dir, args.repo, args.promotion_evidence_dir, args.run_id,
                         reuse_request=args.reuse_request)
    except (ValueError, KeyError, TypeError, OSError, RuntimeError, subprocess.SubprocessError):
        print("HTML performance promotion failed; inspect history and saved evidence", file=sys.stderr)
        return 1
    print(json.dumps({"status": result.status, "recordId": result.record_id}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
