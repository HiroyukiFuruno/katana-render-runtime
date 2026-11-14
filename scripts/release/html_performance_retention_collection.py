"""Attach separate retained-memory observations to a catalog measurement."""

from __future__ import annotations

from collections.abc import Mapping
from pathlib import Path

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
