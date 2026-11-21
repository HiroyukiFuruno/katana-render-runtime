"""Derive versioned memory observations from existing validated report shapes.

The caller must authenticate retained RSS source, binary and raw evidence, and
must run the complete attempt/endpoint validator. This module never grants
formal acceptance or modifies a v1 result or receipt.
"""

from __future__ import annotations

import re
from collections.abc import Mapping, Sequence
from dataclasses import asdict
from pathlib import Path
from typing import cast

import html_performance_memory_policy as memory
import verify_html_performance_evidence as gate


def evaluate_retention_record(
    reports: list[Path], expected: gate.JsonObject, result: gate.JsonObject,
    *, policy_version: str = memory.POLICY_VERSION,
) -> gate.JsonObject:
    """Compute the provisional value that the final validator must reproduce."""
    reference = gate._object(expected["memoryRetentionRef"], "retention reference")
    path = Path(gate._text(reference["path"], "retention path"))
    gate.require(gate.sha256(path) == reference["sha256"], "retention observation changed")
    record = gate.read_json(path)
    retained = [gate._number(value, "retained RSS")
                for value in gate._array(record["retainedRssBytes"], "retained RSS samples")]
    if result.get("comparisonStatus") == "nonComparable":
        candidates = [path for path in reports if path.name.startswith("candidate-")]
        return evaluate_candidate_only(candidates, expected, retained, policy_version=policy_version)
    return evaluate_paired(reports, expected, retained, policy_version=policy_version)


def _validated_samples(reports: Mapping[int, gate.JsonObject]) -> list[gate.JsonObject]:
    samples: list[gate.JsonObject] = []
    for pair in range(5):
        report = reports[pair]
        gate.require(report.get("measurement_executed") is True
                     and type(report.get("pair_index")) is int
                     and type(report.get("sample_count")) is int,
                     "report execution or pair metadata is invalid")
        samples.extend(gate._objects(report["samples"], "report samples"))
    return samples


def _proven_no_scroll(samples: Sequence[gate.JsonObject]) -> bool:
    return all(gate._is_proven_zero_scroll_full_y(
        sample, gate._object(gate._object(sample["phases"], "phases")["full_y"], "full-Y phase"),
    ) for sample in samples)


def _phase_values(samples: Sequence[gate.JsonObject], phase: str, *,
                  no_scroll: bool) -> tuple[list[float], list[float]]:
    measured = [gate._phase_sample(sample, phase) for sample in samples]
    times, peaks = zip(*measured, strict=True)
    return [0.0] * len(times) if no_scroll else list(times), list(peaks)


def evaluate_paired(
    report_paths: list[Path], expected: Mapping[str, object], retained_rss: Sequence[float], *,
    policy_version: str = memory.POLICY_VERSION,
) -> gate.JsonObject:
    """Compute a paired policy; the caller authenticates reports and retention evidence."""
    expected_object = cast(gate.JsonObject, expected)
    grouped = gate._group_reports(report_paths, expected_object)
    aggregate = gate._aggregate_grouped_reports(grouped, expected_object)
    gate.require(aggregate["measurement_executed"] is True, "actual measurements absent")
    baseline = _validated_samples(grouped["baseline"])
    candidate = _validated_samples(grouped["candidate"])
    no_scroll = (policy_version == memory.ZERO_SCROLL_POLICY_VERSION
                 and _proven_no_scroll(baseline) and _proven_no_scroll(candidate))
    phases: dict[str, object] = {}
    accepted = True
    for phase in gate.PHASES:
        no_scroll_phase = no_scroll and phase == "full_y"
        base_time, base_rss = _phase_values(baseline, phase, no_scroll=no_scroll_phase)
        candidate_time, candidate_rss = _phase_values(candidate, phase, no_scroll=no_scroll_phase)
        result = memory.evaluate_peak_phase(base_rss, candidate_rss, base_time, candidate_time,
                                            policy_version=policy_version)
        phases[phase] = asdict(result)
        accepted = accepted and result.peak_allowed
    retention = memory.evaluate_retention(retained_rss, policy_version=policy_version)
    return {
        "version": policy_version,
        "comparisonStatus": "paired",
        "v1Policy": gate.evaluate_measurement_policy(report_paths, expected_object),
        "aggregate": aggregate,
        "phases": phases,
        "retention": asdict(retention),
        "policyPass": accepted and retention.retention_allowed,
        "formalAccepted": False,
    }


def _candidate_reports(
    report_paths: list[Path], expected: gate.JsonObject
) -> list[gate.JsonObject]:
    gate.require(len(report_paths) == 5 and len({path.resolve() for path in report_paths}) == 5,
                 "five distinct candidate reports required")
    by_pair: dict[int, gate.JsonObject] = {}
    for path in report_paths:
        match = re.fullmatch(r"candidate-(\d+)\.json", path.name)
        if match is None:
            raise ValueError("candidate report filename differs")
        pair = int(match.group(1))
        report = gate.read_json(path)
        arm, samples = gate._validate_report(report, expected)
        gate.require(pair in range(5) and pair not in by_pair and arm == "candidate"
                     and type(report.get("pair_index")) is int and report["pair_index"] == pair
                     and report.get("measurement_executed") is True
                     and type(report.get("sample_count")) is int and len(samples) == 5
                     and [sample.get("sample") for sample in samples] == list(range(5))
                     and all(type(sample.get("sample")) is int for sample in samples),
                     "candidate report identity or samples differ")
        by_pair[pair] = report
    gate.require(set(by_pair) == set(range(5)), "candidate pair inventory differs")
    return [by_pair[pair] for pair in range(5)]


def evaluate_candidate_only(
    report_paths: list[Path], expected: Mapping[str, object], retained_rss: Sequence[float], *,
    policy_version: str = memory.POLICY_VERSION,
) -> gate.JsonObject:
    """Evaluate only known non-comparable candidates, without passing comparison policy."""
    expected_object = cast(gate.JsonObject, expected)
    catalog = gate._object(expected_object.get("catalogCondition"), "catalog condition")
    condition_id = gate._text(catalog.get("condition_id"), "condition id")
    gate.require(condition_id in gate.NON_COMPARABLE_BASELINE_FRAGMENT_FAILURES,
                 "candidate-only condition is not an authorized baseline refusal")
    reports = _candidate_reports(report_paths, expected_object)
    aggregate = gate.aggregate_reports(reports, expected_object)
    retention = memory.evaluate_retention(retained_rss, policy_version=policy_version)
    return {
        "version": policy_version,
        "comparisonStatus": "nonComparable",
        "conditionId": condition_id,
        "candidateSamples": 25,
        "candidateAggregate": aggregate,
        "retention": asdict(retention),
        "candidateRetentionPass": retention.retention_allowed,
        "policyPass": False,
        "formalAccepted": False,
    }
