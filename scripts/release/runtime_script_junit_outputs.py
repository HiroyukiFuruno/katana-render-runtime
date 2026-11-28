"""Plan and verify fresh JUnit outputs for fixed runtime-script producers."""

from __future__ import annotations

import hashlib
import os
import re
import stat
import time
from dataclasses import dataclass
from pathlib import Path

import runtime_script_junit as junit


@dataclass(frozen=True)
class ReportPlan:
    producer: str
    epoch: str
    paths: tuple[str, ...]
    started_ns: int


def plan_reports(repo: Path, producer: str, epoch: str, group_count: int) -> ReportPlan:
    if producer not in junit.PRODUCER_NAMES or re.fullmatch(r"[0-9a-f]{64}", epoch) is None:
        raise junit.RuntimeScriptJUnitError("report plan identity is invalid")
    if type(group_count) is not int or not 1 <= group_count <= 32:
        raise junit.RuntimeScriptJUnitError("JUnit group count is unsupported")
    root = repo.resolve(strict=True)
    current = root
    for component in ("tmp", "quality", "runtime-script-results"):
        current = current / component
        try:
            current.mkdir(mode=0o700)
        except FileExistsError:
            pass
        metadata = current.lstat()
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
            raise junit.RuntimeScriptJUnitError("JUnit output parent is not a real directory")
    paths = tuple(
        f"tmp/quality/runtime-script-results/{producer}-{epoch}-group-{index:03}.xml"
        for index in range(1, group_count + 1)
    )
    for relative in paths:
        path = root / relative
        junit.require_output_path(relative, ".xml", producer, epoch)
        try:
            path.lstat()
        except FileNotFoundError:
            continue
        raise junit.RuntimeScriptJUnitError("fresh JUnit output already exists")
    return ReportPlan(producer, epoch, paths, time.time_ns())


def collect_reports(
    repo: Path, plan: ReportPlan, process_exit: int
) -> list[tuple[str, bytes]]:
    if process_exit != 0:
        raise junit.RuntimeScriptJUnitError("failed test process cannot produce JUnit results")
    reports: list[tuple[str, bytes]] = []
    for relative in plan.paths:
        try:
            data = read_output(repo, relative, plan.producer, ".xml", plan.epoch)
            metadata = (repo / relative).stat(follow_symlinks=False)
        except OSError as error:
            raise junit.RuntimeScriptJUnitError("fresh JUnit output is missing") from error
        if metadata.st_mtime_ns < plan.started_ns:
            raise junit.RuntimeScriptJUnitError("JUnit output predates this test run")
        junit.parse_junit(data)
        reports.append((relative, data))
    return reports


def read_output(
    repo: Path,
    relative: str,
    producer: str,
    suffix: str,
    epoch: str | None = None,
) -> bytes:
    path = junit.require_output_path(relative, suffix, producer, epoch)
    root = repo.resolve(strict=True)
    current = root
    for component in path.parts[:-1]:
        current = current / component
        metadata = current.lstat()
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
            raise junit.RuntimeScriptJUnitError("result parent is not a real directory")
    descriptor = os.open(current / path.name, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    with os.fdopen(descriptor, "rb") as source:
        metadata = os.fstat(source.fileno())
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
            raise junit.RuntimeScriptJUnitError("result file is not a regular file")
        return source.read()


def validate_records(repo: Path, records: object, producer: str, epoch: str) -> None:
    if not isinstance(records, list) or not records:
        raise junit.RuntimeScriptJUnitError("receipt has no JUnit reports")
    seen_paths: set[str] = set()
    for record in records:
        if not isinstance(record, dict) or set(record) != {
            "path", "sha256", "tests", "failures", "errors", "skipped"
        }:
            raise junit.RuntimeScriptJUnitError("JUnit receipt has an unsupported shape")
        relative = record.get("path")
        if not isinstance(relative, str):
            raise junit.RuntimeScriptJUnitError("JUnit path is invalid")
        path = junit.require_output_path(relative, ".xml", producer, epoch)
        if path.as_posix() in seen_paths:
            raise junit.RuntimeScriptJUnitError("JUnit path is duplicated")
        seen_paths.add(path.as_posix())
        try:
            data = read_output(repo, relative, producer, ".xml", epoch)
        except (OSError, ValueError) as error:
            if isinstance(error, junit.RuntimeScriptJUnitError):
                raise
            raise junit.RuntimeScriptJUnitError("JUnit report is unavailable or escapes repo") from error
        if hashlib.sha256(data).hexdigest() != record.get("sha256"):
            raise junit.RuntimeScriptJUnitError("JUnit bytes differ from receipt digest")
        counts = junit.parse_junit(data)
        if any(type(record.get(key)) is not int or record[key] != counts[key] for key in junit.COUNT_KEYS):
            raise junit.RuntimeScriptJUnitError("JUnit counts differ from receipt")
        if (
            counts["tests"] <= counts["skipped"]
            or counts["failures"]
            or counts["errors"]
        ):
            raise junit.RuntimeScriptJUnitError("JUnit result is not a clean nonempty pass")
