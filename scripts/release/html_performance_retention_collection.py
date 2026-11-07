"""Attach separate retained-memory observations to a catalog measurement."""

from __future__ import annotations

from collections.abc import Mapping
from pathlib import Path

import html_performance_memory_evidence as memory
import html_performance_retention_capture as capture
import html_performance_retention_evidence as evidence
import verify_html_performance_evidence as gate
from produce_html_performance_evidence import FilePin, _receipt_file_pin


def collect(
    folder: Path, expected: gate.JsonObject, *, binary: Path, fixture: Path,
    plan: Path, root: Path, environment: Mapping[str, str],
) -> tuple[FilePin, list[FilePin]]:
    """Keep the original policy and save additional observations for final validation."""
    viewport = gate._object(expected["viewport"], "retention viewport")
    argv = [str(binary), "--retention", str(fixture),
            str(folder / "unused-retention-output.json"), str(plan),
            str(gate._integer(viewport["width"], "retention width")),
            str(gate._integer(viewport["height"], "retention height")),
            str(viewport["scale"])]
    observed = capture.capture(
        argv, cwd=root, env=environment, output=folder / "retention",
        validate_sample=lambda cycle, raw: evidence.validate_sample(expected, cycle, raw),
    )
    reference, transport = evidence.write_retention_evidence(
        folder / "memory-retention.json", capture=observed, expected=expected,
    )
    return (_receipt_file_pin(reference, "retention observation"),
            [_receipt_file_pin(item, "retention transport") for item in transport])


def policy(
    reports: list[Path], expected: gate.JsonObject, result: gate.JsonObject,
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
        return memory.evaluate_candidate_only(candidates, expected, retained)
    return memory.evaluate_paired(reports, expected, retained)
