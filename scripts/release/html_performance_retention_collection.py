"""Attach separate retained-memory observations to a catalog measurement."""

from __future__ import annotations

from collections.abc import Mapping
from pathlib import Path
from statistics import median

import html_performance_retention_capture as capture
import html_performance_retention_evidence as evidence
import verify_html_performance_evidence as gate
from produce_html_performance_evidence import FilePin, _receipt_file_pin


def collect(
    folder: Path, expected: gate.JsonObject, *, binary: Path, fixture: Path,
    plan: Path, root: Path, environment: Mapping[str, str],
) -> tuple[FilePin, list[FilePin]]:
    """Keep the original policy and save additional observations for final validation."""
    observed = _capture(
        folder, expected, binary=binary, fixture=fixture, plan=plan, root=root,
        environment=environment, diagnostic=False,
    )
    reference, transport = evidence.write_retention_evidence(
        folder / "memory-retention.json", capture=observed, expected=expected,
    )
    return (_receipt_file_pin(reference, "retention observation"),
            [_receipt_file_pin(item, "retention transport") for item in transport])


def needs_diagnostic(result: gate.JsonObject) -> bool:
    policy = result.get("memoryPolicy")
    retention = policy.get("retention") if isinstance(policy, dict) else None
    return isinstance(retention, dict) and retention.get("retention_allowed") is False


def diagnose(
    folder: Path, expected: gate.JsonObject, *, binary: Path, fixture: Path,
    plan: Path, root: Path, environment: Mapping[str, str],
) -> gate.JsonObject:
    condition = gate._object(expected["catalogCondition"], "diagnostic retention condition")
    label = gate._text(condition.get("condition_id"), "diagnostic retention condition ID")
    try:
        observed = _capture(
            folder, expected, binary=binary, fixture=fixture, plan=plan, root=root,
            environment=environment, diagnostic=True,
        )
        rss = evidence.read_diagnostic_rss(observed)
    except Exception as error:
        return {"conditionId": label, "status": "error", "errorType": type(error).__name__}
    medians = {str(count): int(median(rss[count - 5:count])) for count in (25, 50, 100)}
    return {"conditionId": label, "status": "completed", "rssBytes": rss,
            "rssMedianBytes": medians, "rssGrowthAfter50Bytes": medians["100"] - medians["50"]}


def _capture(
    folder: Path, expected: gate.JsonObject, *, binary: Path, fixture: Path,
    plan: Path, root: Path, environment: Mapping[str, str], diagnostic: bool,
) -> capture.RetentionCapture:
    viewport = gate._object(expected["viewport"], "retention viewport")
    mode = "retention-diagnostic" if diagnostic else "retention"
    argv = [str(binary), f"--{mode}", str(fixture),
            str(folder / "unused-retention-output.json"), str(plan),
            str(gate._integer(viewport["width"], "retention width")),
            str(gate._integer(viewport["height"], "retention height")),
            str(viewport["scale"])]
    return capture.capture(
        argv, cwd=root, env=environment, output=folder / mode,
        validate_sample=lambda cycle, raw: evidence.validate_sample(expected, cycle, raw),
    )
