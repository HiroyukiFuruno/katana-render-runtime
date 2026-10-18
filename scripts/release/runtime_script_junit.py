"""Parse and validate the fixed runtime-script Bun JUnit outputs."""

from __future__ import annotations

import hashlib
import re
import xml.etree.ElementTree as ET
from pathlib import Path

import runtime_script_profiles as profiles

COUNT_KEYS = ("tests", "failures", "errors", "skipped")
PRODUCER_NAMES = (
    "runtime-asset-script-other",
    "runtime-unused-script-regression",
    *sorted(profiles.RUNTIME_SCRIPT_PROFILES),
)


class RuntimeScriptJUnitError(ValueError):
    """Raised for an unsupported or failing Bun JUnit report."""


def require_output_path(
    relative: str, suffix: str, producer: str, epoch: str | None = None
) -> Path:
    path = Path(relative)
    expected_name = f"{producer}.json" if suffix == ".json" else None
    if (
        producer not in PRODUCER_NAMES
        or path.is_absolute()
        or ".." in path.parts
        or len(path.parts) != 4
        or path.suffix != suffix
        or path.parts[:3]
        != ("tmp", "quality", "runtime-script-results")
        or (expected_name is not None and path.name != expected_name)
        or (
            suffix == ".xml"
            and re.fullmatch(
                re.escape(producer)
                + "-"
                + (re.escape(epoch) if epoch is not None else r"[0-9a-f]{64}")
                + r"-group-[0-9]{3}\.xml",
                path.name,
            )
            is None
        )
    ):
        raise RuntimeScriptJUnitError("result path is outside the fixed output directory")
    return path


def require_current_output_path(relative: str, suffix: str, producer: str) -> Path:
    path = Path(relative)
    if (
        producer not in PRODUCER_NAMES
        or suffix != ".xml"
        or path.is_absolute()
        or ".." in path.parts
        or len(path.parts) != 4
        or path.suffix != suffix
        or path.parts[:3] != ("tmp", "quality", "runtime-script-results")
        or re.fullmatch(
            re.escape(producer) + r"-current-group-[0-9]{3}\.xml", path.name
        )
        is None
    ):
        raise RuntimeScriptJUnitError("current JUnit path is outside the fixed output set")
    return path


def parse_junit(
    data: bytes, expected_files: tuple[str, ...] | None = None
) -> dict[str, int]:
    """Count actual testcase/failure nodes from one Bun JUnit XML document."""
    if re.search(br"<!\s*(?:DOCTYPE|ENTITY)\b", data, re.IGNORECASE):
        raise RuntimeScriptJUnitError("JUnit declarations and entities are unsupported")
    try:
        root = ET.fromstring(data)
    except ET.ParseError as error:
        raise RuntimeScriptJUnitError("JUnit XML is malformed") from error
    if root.tag not in {"testsuites", "testsuite"}:
        raise RuntimeScriptJUnitError("JUnit root element is unsupported")
    if root.tag == "testsuites" and any(child.tag != "testsuite" for child in root):
        raise RuntimeScriptJUnitError("JUnit suite hierarchy is unsupported")
    suites = list(root.iter("testsuite"))
    if not suites:
        raise RuntimeScriptJUnitError("JUnit contains no test suites")
    suite_files: set[str] = set()
    totals = {key: 0 for key in COUNT_KEYS}
    all_cases = list(root.iter("testcase"))
    if not all_cases:
        raise RuntimeScriptJUnitError("JUnit suite contains no test cases")
    totals["tests"] = len(all_cases)
    for case in all_cases:
        if any(
            child.tag not in {"failure", "error", "skipped", "system-out", "system-err"}
            for child in case
        ):
            raise RuntimeScriptJUnitError("JUnit test case shape is unsupported")
        totals["failures"] += len(case.findall("failure"))
        totals["errors"] += len(case.findall("error"))
        totals["skipped"] += len(case.findall("skipped"))
    for suite in suites:
        if any(
            child.tag not in {"testsuite", "testcase", "system-out", "system-err"}
            for child in suite
        ):
            raise RuntimeScriptJUnitError("JUnit suite child shape is unsupported")
        cases = list(suite.iter("testcase"))
        if not cases:
            raise RuntimeScriptJUnitError("JUnit suite contains no test cases")
        suite_file = suite.get("file")
        if suite_file is not None:
            suite_files.add(_normalize_suite_file(suite_file))
    if expected_files is not None:
        expected = {_normalize_suite_file(path) for path in expected_files}
        if not expected or suite_files != expected:
            raise RuntimeScriptJUnitError("JUnit suites differ from declared test files")
    for element in [root, *suites]:
        for key in COUNT_KEYS:
            declared = element.get(key)
            if declared is None:
                continue
            if not declared.isdecimal():
                raise RuntimeScriptJUnitError("JUnit declared count is invalid")
            actual = totals[key] if element is root else _suite_count(element, key)
            if int(declared) != actual:
                raise RuntimeScriptJUnitError("JUnit declared count disagrees with cases")
    return totals


def _normalize_suite_file(path: str) -> str:
    normalized = path.removeprefix("./")
    if (
        not normalized.startswith("scripts/runtime-bundles/")
        or not normalized.endswith("_test.ts")
        or ".." in Path(normalized).parts
        or Path(normalized).is_absolute()
    ):
        raise RuntimeScriptJUnitError("JUnit suite file path is unsupported")
    return normalized


def _suite_count(suite: ET.Element, key: str) -> int:
    cases = list(suite.iter("testcase"))
    if key == "tests":
        return len(cases)
    child = {"failures": "failure", "errors": "error", "skipped": "skipped"}[key]
    return sum(len(case.findall(child)) for case in cases)


def build_records(
    reports: list[tuple[str, bytes]], producer: str, epoch: str
) -> list[dict[str, object]]:
    return _build_records(reports, producer, epoch=epoch)


def build_current_records(
    reports: list[tuple[str, bytes]], producer: str
) -> list[dict[str, object]]:
    if not reports:
        raise RuntimeScriptJUnitError("at least one JUnit report is required")
    records: list[dict[str, object]] = []
    seen_paths: set[str] = set()
    for relative, data in reports:
        path = require_current_output_path(relative, ".xml", producer)
        if path.as_posix() in seen_paths:
            raise RuntimeScriptJUnitError("JUnit report path is duplicated")
        seen_paths.add(path.as_posix())
        records.append(_record(data, path))
    return records


def _build_records(
    reports: list[tuple[str, bytes]], producer: str, epoch: str
) -> list[dict[str, object]]:
    if not reports:
        raise RuntimeScriptJUnitError("at least one JUnit report is required")
    records: list[dict[str, object]] = []
    seen_paths: set[str] = set()
    for relative, data in reports:
        path = require_output_path(relative, ".xml", producer, epoch)
        if path.as_posix() in seen_paths:
            raise RuntimeScriptJUnitError("JUnit report path is duplicated")
        seen_paths.add(path.as_posix())
        records.append(_record(data, path))
    return records


def _record(data: bytes, path: Path) -> dict[str, object]:
    counts = parse_junit(data)
    if (
        counts["tests"] <= counts["skipped"]
        or counts["failures"]
        or counts["errors"]
    ):
        raise RuntimeScriptJUnitError("JUnit result is not a clean nonempty pass")
    return {"path": path.as_posix(), "sha256": hashlib.sha256(data).hexdigest(), **counts}
