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
from typing import Never

import html_performance_paired_stats as statistics
import html_performance_python_identity as python_identity
import run_html_performance as runner
import verify_html_performance_evidence as gate


class NoImprovement(ValueError):
    """正式な比較で昇格すべき改善が確認されなかった。"""


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
                improvement |= _compare(baseline, candidate)
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
    return improvement, phases


def improvement_established(directory: Path, root: Path) -> bool:
    """正式な84条件の検証後、同じ補正で改善方向も判定する。"""
    root = root.resolve(strict=True)
    runtime_identity = _runtime_identity(root)
    manifest_sha = gate.sha256(directory / "bundle.json")
    result = gate.verify(directory, root)
    gate.require(gate.sha256(directory / "bundle.json") == manifest_sha,
                 "promotion bundle changed during verification")
    improvement, _ = _measure(gate.Bundle(directory), result)
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
    }


def promote(directory: Path, root: Path, evidence_directory: Path, run_id: str) -> str:
    """固定済み候補を再検証し、改善時だけ履歴へ新しい基準を追加する。"""
    root = root.resolve(strict=True)
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
    evidence_directory.mkdir(mode=0o700, parents=False, exist_ok=False)
    evidence_directory.chmod(0o700)
    gate.require(not evidence_directory.is_symlink()
                 and evidence_directory.stat().st_mode & 0o777 == 0o700,
                 "promotion evidence directory is not private")
    stdout_path = evidence_directory / "verifier.stdout"
    stderr_path = evidence_directory / "verifier.stderr"
    argv = [sys.executable, str(root / "scripts/release/verify_html_performance_evidence.py"),
            "--repo", str(root), "--evidence-dir", str(directory)]
    with (evidence_directory / "command.json").open("x", encoding="utf8") as command:
        json.dump(argv, command)
    with stdout_path.open("xb") as stdout, stderr_path.open("xb") as stderr:
        result = subprocess.run(argv, cwd=root, stdout=stdout, stderr=stderr, check=False)
    completed_at = datetime.now(UTC).isoformat().replace("+00:00", "Z")
    with (evidence_directory / "exit.json").open("x", encoding="utf8") as exit_file:
        json.dump({"exitCode": result.returncode, "completedAtUtc": completed_at}, exit_file)
    gate.require(result.returncode == 0, "formal performance verification failed")
    gate.require(gate.sha256(directory / "bundle.json") == manifest_sha,
                 "promotion bundle changed during verification")
    verified = gate.read_json(stdout_path)
    bundle = gate.Bundle(directory)
    improvement, phases = _measure(bundle, verified)
    gate.require(_runtime_identity(root) == runtime_identity,
                 "promotion runtime identity changed during comparison")
    if not improvement:
        raise NoImprovement("significant performance improvement is not established")
    gate.require(gate.sha256(directory / "bundle.json") == manifest_sha,
                 "promotion bundle changed during comparison")
    summary_path = evidence_directory / "summary.json"
    with summary_path.open("xb") as summary:
        summary.write((json.dumps({"schema": "krr-html-performance-promotion-summary-v1",
                                   "bundleManifestSha256": manifest_sha,
                                   "candidatePhaseStatistics": phases},
                                  sort_keys=True, separators=(",", ":")) + "\n").encode())
    stdout_sha, summary_sha = gate.sha256(stdout_path), gate.sha256(summary_path)
    record = _record(bundle, verified, phases, run_id, manifest_sha,
                     stdout_sha, summary_sha, completed_at)
    records = gate._objects(history["records"], "history records")
    gate.require(all(item.get("id") != record["id"] for item in records), "record already exists")
    history["records"] = [*records, record]
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
    _atomic_history_update(root, history_path, before, after, head)
    return gate._text(record["id"], "new reference id")


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
    try:
        args = parser.parse_args()
        selected = promote(args.evidence_dir, args.repo, args.promotion_evidence_dir, args.run_id)
    except NoImprovement:
        print(json.dumps({"status": "unchanged", "reason": "no-significant-improvement"}))
        return 0
    except (ValueError, KeyError, TypeError, OSError, RuntimeError, subprocess.SubprocessError):
        print("HTML performance promotion failed; inspect history and saved evidence", file=sys.stderr)
        return 1
    print(json.dumps({"status": "promoted", "recordId": selected}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
