"""Derive v2 memory observations from existing validated report shapes.

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


def _samples(reports: Mapping[int, gate.JsonObject], phase: str) -> tuple[list[float], list[float]]:
    elapsed: list[float] = []
    rss: list[float] = []
    for pair in range(5):
        report = reports[pair]
        gate.require(report.get("measurement_executed") is True
                     and type(report.get("pair_index")) is int
                     and type(report.get("sample_count")) is int,
                     "report execution or pair metadata is invalid")
        for sample in gate._objects(report["samples"], "report samples"):
            duration, peak = gate._phase_sample(sample, phase)
            elapsed.append(duration)
            rss.append(peak)
    return elapsed, rss


def evaluate_paired(
    report_paths: list[Path], expected: Mapping[str, object], retained_rss: Sequence[float]
) -> gate.JsonObject:
    """Compute a paired policy; the caller authenticates reports and retention evidence."""
    expected_object = cast(gate.JsonObject, expected)
    grouped = gate._group_reports(report_paths, expected_object)
    aggregate = gate._aggregate_grouped_reports(grouped, expected_object)
    gate.require(aggregate["measurement_executed"] is True, "actual measurements absent")
    phases: dict[str, object] = {}
    accepted = True
    for phase in gate.PHASES:
        base_time, base_rss = _samples(grouped["baseline"], phase)
        candidate_time, candidate_rss = _samples(grouped["candidate"], phase)
        result = memory.evaluate_peak_phase(base_rss, candidate_rss, base_time, candidate_time)
        phases[phase] = asdict(result)
        accepted = accepted and result.peak_allowed
    retention = memory.evaluate_retention(retained_rss)
    return {
        "version": memory.POLICY_VERSION,
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
    report_paths: list[Path], expected: Mapping[str, object], retained_rss: Sequence[float]
) -> gate.JsonObject:
    """Evaluate only known non-comparable candidates, without passing comparison policy."""
    expected_object = cast(gate.JsonObject, expected)
    catalog = gate._object(expected_object.get("catalogCondition"), "catalog condition")
    condition_id = gate._text(catalog.get("condition_id"), "condition id")
    gate.require(condition_id in gate.NON_COMPARABLE_BASELINE_FRAGMENT_FAILURES,
                 "candidate-only condition is not an authorized baseline refusal")
    reports = _candidate_reports(report_paths, expected_object)
    aggregate = gate.aggregate_reports(reports, expected_object)
    retention = memory.evaluate_retention(retained_rss)
    return {
        "version": memory.POLICY_VERSION,
        "comparisonStatus": "nonComparable",
        "conditionId": condition_id,
        "candidateSamples": 25,
        "candidateAggregate": aggregate,
        "retention": asdict(retention),
        "candidateRetentionPass": retention.retention_allowed,
        "policyPass": False,
        "formalAccepted": False,
    }
