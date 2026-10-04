#!/usr/bin/env python3
from __future__ import annotations

import argparse
import base64
import binascii
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import IO, Optional


class ContractViolation(RuntimeError):
    pass


@dataclass(frozen=True)
class Issue:
    number: int
    state: str
    body: str
    url: str
    updated_at: str = ""


IssueLoader = Callable[[int], Optional[Issue]]
TreeEntries = dict[str, tuple[str, str, str]]
_MAX_PROVENANCE_BLOB_BYTES = 100 * 1024 * 1024


def _read_push_input(stream: IO[str]) -> str:
    if stream.isatty():
        return ""
    return stream.read()


_ISSUE_URL_TERMINATOR = r"(?=$|[\s)\]}>.,!?;:'\"])"
_FULL_ISSUE_PATTERN = re.compile(
    r"https://github\.com/(?P<owner>[^/\s]+)/(?P<repo>[^/\s]+)/issues/(?P<number>[1-9]\d*)"
    + _ISSUE_URL_TERMINATOR,
    re.IGNORECASE,
)
_SHORT_ISSUE_PATTERN = re.compile(r"(?<![\w/])#(?P<number>[1-9]\d*)\b")
_REFS_ISSUE_PATTERN = re.compile(
    r"\brefs\b(?:[ \t]*:[ \t]*|[ \t]+)[\[(\'\"]?(?P<reference>#[1-9]\d*\b|https://github\.com/[^/\s]+/[^/\s]+/issues/[1-9]\d*"
    + _ISSUE_URL_TERMINATOR
    + r")",
    re.IGNORECASE,
)
_CLOSING_ISSUE_REFERENCE_PATTERN = re.compile(
    r"\b(?:close(?:s|d)?|fix(?:es|ed)?|resolve(?:s|d)?)\b"
    r"(?:[ \t]*:[ \t]*|[ \t]+)"
    r"(?P<reference>#[1-9]\d*\b|https://github\.com/[^/\s]+/[^/\s]+/issues/[1-9]\d*"
    + _ISSUE_URL_TERMINATOR
    + r")",
    re.IGNORECASE,
)
_ZERO_SHA = "0" * 40
_SHA_PATTERN = re.compile(r"^[0-9a-fA-F]{40}$")
_REPOSITORY_PATTERN = re.compile(r"^[^/\s]+/[^/\s]+$")
_RELEASE_BRANCH_PATTERN = re.compile(r"release/v(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\Z")
_NAME_STATUS_PATTERN = re.compile(r"^(?P<kind>[ACDMRTUXB])(?P<score>\d{1,3})?$")
_MANIFEST_NAMES = {
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "go.mod",
    "Gemfile",
}
_LOCKFILE_NAMES = {
    "Cargo.lock",
    "bun.lock",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "poetry.lock",
    "go.sum",
    "Gemfile.lock",
}
_LOCKFILE_ORIGIN_MANIFESTS = {
    "Cargo.lock": "Cargo.toml",
    "bun.lock": "package.json",
    "package-lock.json": "package.json",
    "pnpm-lock.yaml": "package.json",
    "yarn.lock": "package.json",
    "poetry.lock": "pyproject.toml",
    "go.sum": "go.mod",
    "Gemfile.lock": "Gemfile",
}
_MANIFEST_TOKEN_DELIMITERS = frozenset({","})
_EVIDENCE_FIELDS: tuple[tuple[str, tuple[str, ...]], ...] = (
    ("上流公開版", ("上流公開版", "Upstream release")),
    ("API移行", ("API移行", "API migration")),
    (
        "依存manifest",
        ("依存manifest", "Dependency manifest", "Dependency manifests"),
    ),
    ("lockfile", ("lockfile", "Lockfiles")),
    ("検証証跡", ("検証証跡", "Verification")),
)
_NON_PATH_EVIDENCE_FIELDS = frozenset({"上流公開版", "API移行", "検証証跡"})
_REQUIRED_EVIDENCE_PLACEHOLDER_VALUES = frozenset({"", "-", "todo", "tbd"})
_NON_PATH_PLACEHOLDER_EVIDENCE_VALUES = frozenset(
    {
        "n/a",
        "na",
        "n.a.",
        "none",
        "null",
        "nil",
        "not applicable",
        "not available",
        "未定",
        "該当なし",
    }
)


def _path_tokens(value: str) -> tuple[str, ...]:
    """Parse exact path tokens with optional backtick quoting."""
    tokens: list[str] = []
    index = 0
    value_length = len(value)
    while index < value_length:
        while index < value_length and (
            value[index].isspace() or value[index] in _MANIFEST_TOKEN_DELIMITERS
        ):
            index += 1
        if index == value_length:
            break
        if value[index] == "`":
            closing = value.find("`", index + 1)
            if closing < 0:
                raise ContractViolation("依存manifest欄のquoted tokenが閉じていません")
            token = value[index + 1 : closing]
            if not token:
                raise ContractViolation("依存manifest欄のquoted tokenが空です")
            index = closing + 1
            if index < value_length and not (
                value[index].isspace() or value[index] in _MANIFEST_TOKEN_DELIMITERS
            ):
                raise ContractViolation("依存manifest欄のquoted token隣接が不正です")
        else:
            start = index
            while index < value_length and not (
                value[index].isspace()
                or value[index] in _MANIFEST_TOKEN_DELIMITERS
                or value[index] == "`"
            ):
                index += 1
            token = value[start:index]
            if index < value_length and value[index] == "`":
                raise ContractViolation("依存manifest欄のtoken隣接が不正です")
        tokens.append(token)
    return tuple(tokens)


def parse_push_updates(raw: str) -> tuple[tuple[str, str, str, str], ...]:
    updates: list[tuple[str, str, str, str]] = []
    for line in raw.splitlines():
        if not line.strip():
            continue
        fields = line.split()
        if len(fields) != 4:
            raise ContractViolation(f"pre-push updateの形式が不正です: {line!r}")
        local_ref, local_sha, remote_ref, remote_sha = fields
        if not _is_local_ref(local_ref):
            raise ContractViolation(f"pre-push local refの形式が不正です: {local_ref!r}")
        if not _is_remote_ref(remote_ref):
            raise ContractViolation(
                f"pre-push refの形式が不正です: {local_ref!r} -> {remote_ref!r}"
            )
        if not _SHA_PATTERN.fullmatch(local_sha) or not _SHA_PATTERN.fullmatch(
            remote_sha
        ):
            raise ContractViolation(
                f"pre-push SHAの形式が不正です: {local_sha!r} -> {remote_sha!r}"
            )
        updates.append((local_ref, local_sha, remote_ref, remote_sha))
    return tuple(updates)


def _is_remote_ref(reference: str) -> bool:
    """Accept a branch or tag ref using Git's refname safety constraints."""
    if not (reference.startswith("refs/heads/") or reference.startswith("refs/tags/")):
        return False
    suffix = reference.removeprefix("refs/heads/")
    if suffix == reference:
        suffix = reference.removeprefix("refs/tags/")
    if not suffix or suffix.startswith("/") or suffix.endswith("/"):
        return False
    if "//" in suffix or ".." in suffix or "@{" in suffix or suffix.endswith("."):
        return False
    forbidden = set(" ~^:?*[")
    if any(character.isspace() or ord(character) < 32 or character in forbidden for character in suffix):
        return False
    if "\\" in suffix:
        return False
    return all(
        component not in {"", ".", ".."}
        and not component.startswith(".")
        and not component.endswith(".lock")
        for component in suffix.split("/")
    )


def _is_local_ref(reference: str) -> bool:
    if reference == "(delete)":
        return True
    if any(character.isspace() for character in reference):
        return False
    return (
        _SHA_PATTERN.fullmatch(reference) is not None
        or _is_remote_ref(reference)
        or reference == "HEAD"
        or "~" in reference
        or "^" in reference
    )


def pushed_branch_updates(
    updates: Sequence[tuple[str, str, str, str]],
    *,
    default_branch: str,
) -> tuple[tuple[str, str], ...]:
    targets: list[tuple[str, str]] = []
    seen: set[tuple[str, str]] = set()
    for _local_ref, local_sha, remote_ref, _remote_sha in updates:
        if local_sha == _ZERO_SHA:
            continue
        if not remote_ref.startswith("refs/heads/"):
            continue
        branch = remote_ref.removeprefix("refs/heads/")
        if not branch or branch == default_branch:
            continue
        key = (branch, local_sha)
        if key not in seen:
            targets.append(key)
            seen.add(key)
    return tuple(targets)


def issue_numbers(message: str, repository: str) -> set[int]:
    """Return same-repository Issues named by explicit ``Refs`` clauses only."""

    expected_repository = repository.casefold()
    numbers: set[int] = set()
    for line in message.splitlines():
        clause = re.search(r"\brefs\b(?:[ \t]*:[ \t]*|[ \t]+)(?P<references>.+)$", line, re.IGNORECASE)
        if clause is None:
            continue
        references = clause.group("references")
        numbers.update(int(match.group("number")) for match in _SHORT_ISSUE_PATTERN.finditer(references))
        for match in _FULL_ISSUE_PATTERN.finditer(references):
            if f"{match.group('owner')}/{match.group('repo')}".casefold() == expected_repository:
                numbers.add(int(match.group("number")))
    return numbers


def closing_issue_numbers(body: str, repository: str) -> set[int]:
    """Return same-repository Issues referenced with a GitHub closing keyword."""

    if not isinstance(body, str):
        raise ContractViolation("PR本文の形式が不正です")
    expected_repository = repository.casefold()
    numbers: set[int] = set()
    for clause in _CLOSING_ISSUE_REFERENCE_PATTERN.finditer(body):
        reference = clause.group("reference")
        if reference.startswith("#"):
            numbers.add(int(reference[1:]))
            continue
        match = _FULL_ISSUE_PATTERN.fullmatch(reference)
        if match is not None and f"{match.group('owner')}/{match.group('repo')}".casefold() == expected_repository:
            numbers.add(int(match.group("number")))
    return numbers


def dependency_contract_paths(paths: Sequence[str]) -> tuple[list[str], list[str]]:
    manifests: set[str] = set()
    lockfiles: set[str] = set()
    for raw_path in paths:
        path = PurePosixPath(raw_path)
        if path.name in _MANIFEST_NAMES:
            manifests.add(raw_path)
        if path.name in _LOCKFILE_NAMES:
            lockfiles.add(raw_path)
    return sorted(manifests), sorted(lockfiles)


def parse_name_status_paths(raw: str) -> list[str]:
    """Collect every changed path from `git diff --name-status -z` output."""
    return [
        path
        for _status, record_paths in _parse_name_status_records(raw)
        for path in record_paths
    ]


def _parse_name_status_records(raw: str) -> list[tuple[str, tuple[str, ...]]]:
    """Parse complete name-status records while retaining rename endpoints."""
    if not raw:
        return []
    if not raw.endswith("\0"):
        raise ContractViolation("git diff --name-status -z outputが途中で切れています")
    fields = raw.split("\0")[:-1]
    records: list[tuple[str, tuple[str, ...]]] = []
    index = 0
    while index < len(fields):
        status = fields[index]
        index += 1
        match = _NAME_STATUS_PATTERN.fullmatch(status)
        if match is None:
            raise ContractViolation(f"git diff statusが不正です: {status!r}")
        kind = match.group("kind")
        score = match.group("score")
        if kind in {"R", "C"}:
            if score is None or int(score) > 100:
                raise ContractViolation(
                    f"git diff {kind} statusのrename/copy scoreが不正です"
                )
            required_paths = 2
        else:
            if score is not None:
                raise ContractViolation(f"git diff {kind} statusに不要なscoreがあります")
            required_paths = 1
        if len(fields) - index < required_paths:
            raise ContractViolation("git diff --name-status -z recordが途中で切れています")
        record_paths = fields[index : index + required_paths]
        index += required_paths
        if any(not path for path in record_paths):
            raise ContractViolation("git diff --name-status -z pathが空です")
        records.append((kind, tuple(record_paths)))
    return records


def parse_commit_messages(raw: str) -> list[str]:
    """Keep one `git log -z` record per commit, including empty messages."""
    if not raw:
        return []
    if not raw.endswith("\0"):
        raise ContractViolation("git log -z outputが途中で切れています")
    return raw.split("\0")[:-1]


def dependency_evidence_errors(
    body: str,
    manifests: Sequence[str],
    lockfiles: Sequence[str],
) -> list[str]:
    errors: list[str] = []
    if not re.search(
        r"(?im)^##+\s*(?:依存更新証跡|Dependency Update Evidence)\s*$",
        body,
    ):
        errors.append("依存更新証跡の見出し")
    evidence_values: dict[str, str] = {}
    for display_name, labels in _EVIDENCE_FIELDS:
        label_pattern = "|".join(re.escape(label) for label in labels)
        match = re.search(
            rf"(?im)^\s*[-*]\s*(?:{label_pattern})\s*[:：]\s*(?P<value>.+?)\s*$",
            body,
        )
        value = match.group("value").strip() if match is not None else ""
        placeholder = value.casefold()
        if match is None or placeholder in _REQUIRED_EVIDENCE_PLACEHOLDER_VALUES or (
            display_name in _NON_PATH_EVIDENCE_FIELDS
            and placeholder in _NON_PATH_PLACEHOLDER_EVIDENCE_VALUES
        ):
            errors.append(display_name)
        elif display_name not in evidence_values:
            evidence_values[display_name] = value

    manifest_tokens = _path_tokens(evidence_values.get("依存manifest", ""))
    if manifests:
        expected_manifest_paths = set(manifests)
        actual_manifest_paths = set(manifest_tokens)
        for path in manifests:
            if path not in actual_manifest_paths:
                errors.append(path)
        if (
            len(manifest_tokens) != len(actual_manifest_paths)
            or actual_manifest_paths - expected_manifest_paths
        ):
            errors.append("依存manifest")

    lockfile_tokens = _path_tokens(evidence_values.get("lockfile", ""))
    if lockfiles:
        expected_lockfile_paths = set(lockfiles)
        actual_lockfile_paths = set(lockfile_tokens)
        if (
            actual_lockfile_paths != expected_lockfile_paths
            or len(lockfile_tokens) != len(actual_lockfile_paths)
        ):
            errors.append("lockfile")

    if lockfiles and not manifests:
        origin_paths = sorted(
            {
                str(PurePosixPath(lockfile).with_name(_LOCKFILE_ORIGIN_MANIFESTS[PurePosixPath(lockfile).name]))
                for lockfile in lockfiles
            }
        )
        if (
            set(manifest_tokens) != set(origin_paths)
            or len(manifest_tokens) != len(origin_paths)
        ):
            errors.append("依存manifest")

    return errors


def validate_contract(
    *,
    branch: str,
    default_branch: Optional[str],
    repository: str,
    commit_messages: Sequence[str],
    changed_paths: Sequence[str],
    issue_loader: IssueLoader,
    release_issue_numbers: set[int] | None = None,
) -> None:
    if branch == default_branch:
        return

    referenced_numbers: set[int] = set()
    release_branch = is_release_branch(branch)
    if release_branch and release_issue_numbers is not None:
        if any(type(number) is not int or number < 1 for number in release_issue_numbers):
            raise ContractViolation("release Issue集合の形式が不正です")
        referenced_numbers.update(release_issue_numbers)
    else:
        for index, message in enumerate(commit_messages, start=1):
            references = issue_numbers(message, repository)
            if not references and not release_branch:
                raise ContractViolation(
                    f"非default branchのcommit {index}に対象repositoryのIssue参照がありません"
                )
            referenced_numbers.update(references)

    if not referenced_numbers:
        raise ContractViolation(
            "非default branchのcommit範囲に対象repositoryのIssue参照がありません"
        )

    loaded_issues: list[Issue] = []
    for number in sorted(referenced_numbers):
        issue = issue_loader(number)
        if issue is None:
            raise ContractViolation(f"Issue #{number}を対象repositoryで確認できません")
        if issue.state != "OPEN":
            raise ContractViolation(f"Issue #{number}はOPENではありません: {issue.state}")
        loaded_issues.append(issue)

    manifests, lockfiles = dependency_contract_paths(changed_paths)
    if not manifests and not lockfiles:
        return
    issue_errors = [
        dependency_evidence_errors(issue.body, manifests, lockfiles)
        for issue in loaded_issues
    ]
    if any(not errors for errors in issue_errors):
        return
    missing = sorted({error for errors in issue_errors for error in errors})
    raise ContractViolation(
        "参照Issueの依存更新証跡が不足しています: " + ", ".join(missing)
    )


def _effective_release_issue_numbers(
    *,
    repository: str,
    references_by_commit: Sequence[tuple[str, set[int]]],
    net_paths: Sequence[str],
    issue_loader: IssueLoader,
    commit_paths: Callable[[str], Sequence[str]],
    commit_surviving_paths: Callable[[str, Sequence[str]], Sequence[str]],
) -> set[int]:
    """Allow closed refs only when every referencing commit is obsolete in this diff."""
    net_path_set = set(net_paths)
    by_issue: dict[int, list[str]] = {}
    for commit, references in references_by_commit:
        for number in references:
            by_issue.setdefault(number, []).append(commit)
    effective: set[int] = set()
    for number, commits in by_issue.items():
        issue = issue_loader(number)
        if issue is None:
            raise ContractViolation(f"Issue #{number}を対象repositoryで確認できません")
        canonical_url = f"https://github.com/{repository}/issues/{number}"
        if (
            type(issue.number) is not int
            or issue.number != number
            or not isinstance(issue.url, str)
            or issue.url.casefold() != canonical_url.casefold()
        ):
            raise ContractViolation(f"Issue #{number}は対象repositoryのcanonical Issueではありません")
        if issue.state == "OPEN":
            effective.add(number)
            continue
        if issue.state != "CLOSED" or not commits:
            raise ContractViolation(f"Issue #{number}はOPENではありません: {issue.state}")
        for commit in commits:
            paths = tuple(commit_paths(commit))
            if not paths or net_path_set.intersection(paths):
                raise ContractViolation(f"closed Issue #{number}の参照commitが現行差分と重複するか証跡不足です")
            surviving_paths = set(commit_surviving_paths(commit, paths))
            if net_path_set.intersection(surviving_paths):
                raise ContractViolation(
                    f"closed Issue #{number}の参照commit由来の内容がrename先で現行差分に残っています"
                )
    return effective


def _require_sha(value: object, label: str) -> str:
    if not isinstance(value, str) or _SHA_PATTERN.fullmatch(value) is None:
        raise ContractViolation(f"{label}は40文字のSHAである必要があります")
    return value.lower()


def is_release_branch(branch: str) -> bool:
    """完全なリリースIssue集合を許可するbranchか判定する。"""
    return _RELEASE_BRANCH_PATTERN.fullmatch(branch) is not None


def _require_repository_name(value: str) -> str:
    if _REPOSITORY_PATTERN.fullmatch(value) is None:
        raise ContractViolation(f"repositoryの形式が不正です: {value!r}")
    return value


def _gh_json(*arguments: str) -> object:
    result = subprocess.run(
        ["gh", "api", *arguments],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip()
        raise ContractViolation(f"GitHub API request failed: {detail}")
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise ContractViolation("GitHub API responseがJSONではありません") from error


def _pr_commit_records(
    *,
    repository: str,
    base_sha: str,
    head_sha: str,
) -> list[tuple[str, str]]:
    comparison = _gh_json(
        f"repos/{repository}/compare/{base_sha}...{head_sha}"
    )
    if not isinstance(comparison, dict):
        raise ContractViolation("GitHub compare responseの形式が不正です")
    base_commit = comparison.get("base_commit")
    merge_base_commit = comparison.get("merge_base_commit")
    if not isinstance(base_commit, dict) or not isinstance(merge_base_commit, dict):
        raise ContractViolation("GitHub compare responseにbase/merge-base commitがありません")
    if _require_sha(base_commit.get("sha"), "compare base SHA") != base_sha:
        raise ContractViolation("GitHub compare responseのbase SHAが一致しません")
    _require_sha(merge_base_commit.get("sha"), "compare merge-base SHA")
    status = comparison.get("status")
    ahead_by = comparison.get("ahead_by")
    behind_by = comparison.get("behind_by")
    total_commits = comparison.get("total_commits")
    commits = comparison.get("commits")
    if status not in {"ahead", "behind", "diverged", "identical"}:
        raise ContractViolation("GitHub compare responseのstatusが不正です")
    if not isinstance(ahead_by, int) or ahead_by < 0:
        raise ContractViolation("GitHub compare responseのahead_byが不正です")
    if not isinstance(behind_by, int) or behind_by < 0:
        raise ContractViolation("GitHub compare responseのbehind_byが不正です")
    if not isinstance(total_commits, int) or total_commits < 0:
        raise ContractViolation("GitHub compare responseのtotal_commitsが不正です")
    if not isinstance(commits, list):
        raise ContractViolation("GitHub compare responseのcommitsが不正です")
    # Compare API truncates large commit ranges.  Partial history must never
    # make a trusted success possible.
    if len(commits) != ahead_by or total_commits != ahead_by:
        raise ContractViolation(
            "GitHub compare responseがbase..headの全commitを返していません"
        )
    if ahead_by == 0:
        raise ContractViolation("PR base..headに検証対象commitがありません")
    records: list[tuple[str, str]] = []
    for commit in commits:
        if not isinstance(commit, dict):
            raise ContractViolation("GitHub compare responseのcommitが不正です")
        commit_sha = _require_sha(commit.get("sha"), "compare commit SHA")
        payload = commit.get("commit")
        if not isinstance(payload, dict) or not isinstance(payload.get("message"), str):
            raise ContractViolation("GitHub compare responseのcommit messageが不正です")
        records.append((commit_sha, payload["message"]))
    # GitHub's compare response does not expose head_commit.  The final item
    # is the head tip only after the full base..head range above was verified.
    if records[-1][0] != head_sha:
        raise ContractViolation("GitHub compare responseの最終commitがhead SHAと一致しません")
    return records


def _pr_commit_messages(*, repository: str, base_sha: str, head_sha: str) -> list[str]:
    return [message for _, message in _pr_commit_records(repository=repository, base_sha=base_sha, head_sha=head_sha)]


def referenced_issue_snapshot(
    *,
    repository: str,
    base_sha: str,
    head_sha: str,
    issue_loader: IssueLoader | None = None,
) -> tuple[Issue, ...]:
    """Read the complete base..head Issue reference set without PR checkout.

    This deliberately uses the same compare response and reference parser as
    the push contract.  Callers must not substitute GitHub's
    ``closingIssuesReferences``: it omits references that live only in commit
    messages.
    """

    repository = _require_repository_name(repository)
    base_sha = _require_sha(base_sha, "PR base SHA")
    head_sha = _require_sha(head_sha, "PR head SHA")
    if issue_loader is None:
        issue_loader = lambda number: _load_issue(repository, number)
    references: set[int] = set()
    for message in _pr_commit_messages(
        repository=repository,
        base_sha=base_sha,
        head_sha=head_sha,
    ):
        references.update(issue_numbers(message, repository))

    snapshot: list[Issue] = []
    for number in sorted(references):
        issue = issue_loader(number)
        if issue is None:
            raise ContractViolation(f"Issue #{number}を対象repositoryで確認できません")
        if type(issue.number) is not int or issue.number != number:
            raise ContractViolation(f"Issue #{number}のsnapshot番号が一致しません")
        if (
            not isinstance(issue.state, str)
            or not issue.state
            or not isinstance(issue.body, str)
            or not isinstance(issue.url, str)
        ):
            raise ContractViolation(f"Issue #{number}のsnapshot形式が不正です")
        if not isinstance(issue.updated_at, str) or not issue.updated_at:
            raise ContractViolation(f"Issue #{number}のupdated_atが不正です")
        canonical_url = f"https://github.com/{repository}/issues/{number}"
        if issue.url.casefold() != canonical_url.casefold():
            raise ContractViolation(
                f"Issue #{number}は対象repositoryのcanonical Issue URLではありません"
            )
        snapshot.append(issue)
    return tuple(snapshot)


def _pr_changed_paths(
    *, repository: str, base_sha: str, head_sha: str
) -> list[str]:
    """Read paths from immutable base..head objects, never mutable PR metadata."""

    comparison = _gh_json(f"repos/{repository}/compare/{base_sha}...{head_sha}")
    if not isinstance(comparison, dict):
        raise ContractViolation("GitHub compare responseの形式が不正です")
    base_commit = comparison.get("base_commit")
    if not isinstance(base_commit, dict) or (
        _require_sha(base_commit.get("sha"), "compare base SHA") != base_sha
    ):
        raise ContractViolation("GitHub compare responseのbase SHAが一致しません")
    files = comparison.get("files")
    if not isinstance(files, list):
        raise ContractViolation("GitHub compare filesの形式が不正です")
    paths: list[str] = []
    for changed_file in files:
        if not isinstance(changed_file, dict):
            raise ContractViolation("GitHub compare files entryの形式が不正です")
        filename = changed_file.get("filename")
        if not isinstance(filename, str) or not filename:
            raise ContractViolation("GitHub compare files entryのfilenameが不正です")
        paths.append(filename)
        previous_filename = changed_file.get("previous_filename")
        if previous_filename is not None:
            if not isinstance(previous_filename, str) or not previous_filename:
                raise ContractViolation(
                    "GitHub compare files entryのprevious_filenameが不正です"
                )
            paths.append(previous_filename)
    # GitHub returns at most 300 paths for a comparison.  When the response
    # reaches that limit, compare the immutable merge-base and head commit
    # trees instead of trusting a partial file list.  Comparing the current
    # base tip would incorrectly classify changes made only on the default
    # branch after the PR diverged as PR changes.  The tree endpoint is also
    # fail-closed on truncation and malformed entries.
    if len(files) >= 300:
        merge_base_commit = comparison.get("merge_base_commit")
        if not isinstance(merge_base_commit, dict):
            raise ContractViolation("GitHub compare responseにmerge-base commitがありません")
        merge_base_sha = _require_sha(
            merge_base_commit.get("sha"), "compare merge-base SHA"
        )
        return _pr_changed_paths_from_trees(
            repository=repository,
            baseline_sha=merge_base_sha,
            head_sha=head_sha,
        )
    return paths


def _git_tree_entries(
    *, repository: str, commit_sha: str, label: str, commit_payload: object = None
) -> TreeEntries:
    commit = commit_payload if commit_payload is not None else _gh_json(
        f"repos/{repository}/git/commits/{commit_sha}"
    )
    if not isinstance(commit, dict):
        raise ContractViolation(f"{label} commit responseの形式が不正です")
    if _require_sha(commit.get("sha"), f"{label} commit SHA") != commit_sha:
        raise ContractViolation(f"{label} commit SHAが一致しません")
    tree = commit.get("tree")
    if not isinstance(tree, dict):
        raise ContractViolation(f"{label} commit responseにtreeがありません")
    tree_sha = _require_sha(tree.get("sha"), f"{label} tree SHA")
    payload = _gh_json(f"repos/{repository}/git/trees/{tree_sha}?recursive=1")
    if not isinstance(payload, dict) or payload.get("truncated") is not False:
        raise ContractViolation(f"{label} tree responseが打ち切られたか形式不正です")
    if _require_sha(payload.get("sha"), f"{label} tree response SHA") != tree_sha:
        raise ContractViolation(f"{label} tree response SHAが一致しません")
    entries = payload.get("tree")
    if not isinstance(entries, list):
        raise ContractViolation(f"{label} tree entriesの形式が不正です")
    result: dict[str, tuple[str, str, str]] = {}
    for entry in entries:
        if not isinstance(entry, dict):
            raise ContractViolation(f"{label} tree entryの形式が不正です")
        path = entry.get("path")
        entry_type = entry.get("type")
        mode = entry.get("mode")
        entry_sha = entry.get("sha")
        if (
            not isinstance(path, str)
            or not path
            or "\x00" in path
            or path.startswith("/")
            or any(part in {"", ".", ".."} for part in path.split("/"))
        ):
            raise ContractViolation(f"{label} tree entry pathの形式が不正です")
        valid_modes = {"blob": {"100644", "100755", "120000"}, "tree": {"040000"}, "commit": {"160000"}}
        if not isinstance(entry_type, str) or entry_type not in valid_modes:
            raise ContractViolation(f"{label} tree entry typeの形式が不正です")
        if not isinstance(mode, str) or mode not in valid_modes[entry_type]:
            raise ContractViolation(f"{label} tree entry modeの形式が不正です")
        normalized_sha = _require_sha(entry_sha, f"{label} tree entry SHA")
        if path in result:
            raise ContractViolation(f"{label} tree entry pathが重複しています")
        result[path] = (entry_type, mode, normalized_sha)
    return result


def _pr_changed_paths_from_trees(
    *, repository: str, baseline_sha: str, head_sha: str
) -> list[str]:
    base_entries = _git_tree_entries(
        repository=repository, commit_sha=baseline_sha, label="compare merge-base"
    )
    head_entries = _git_tree_entries(
        repository=repository, commit_sha=head_sha, label="compare head"
    )
    paths = sorted(set(base_entries) | set(head_entries))
    return [
        path for path in paths if base_entries.get(path) != head_entries.get(path)
    ]


def _validate_pr_canonical_issue(
    *,
    repository: str,
    number: int,
    issue_loader: IssueLoader,
) -> None:
    issue = issue_loader(number)
    if issue is None:
        raise ContractViolation(f"Issue #{number}を対象repositoryで確認できません")
    if type(issue.number) is not int or issue.number != number:
        raise ContractViolation(f"Issue #{number}のsnapshot番号が一致しません")
    if issue.state != "OPEN":
        raise ContractViolation(f"Issue #{number}はOPENではありません: {issue.state}")
    canonical_url = f"https://github.com/{repository}/issues/{number}"
    if not isinstance(issue.url, str) or issue.url.casefold() != canonical_url.casefold():
        raise ContractViolation(
            f"Issue #{number}は対象repositoryのcanonical Issue URLではありません"
        )


def validate_pr_range(
    *,
    repository: str,
    pr_number: int,
    base_sha: str,
    head_sha: str,
    branch: str,
    issue_loader: IssueLoader,
) -> set[int]:
    """Validate a PR's base..head contract without checking out PR code."""
    repository = _require_repository_name(repository)
    base_sha = _require_sha(base_sha, "PR base SHA")
    head_sha = _require_sha(head_sha, "PR head SHA")
    if pr_number < 1:
        raise ContractViolation("PR番号は正の整数である必要があります")
    if not _is_remote_ref(f"refs/heads/{branch}"):
        raise ContractViolation(f"PR head branchの形式が不正です: {branch!r}")

    commit_records = _pr_commit_records(
        repository=repository,
        base_sha=base_sha,
        head_sha=head_sha,
    )
    commit_messages = [message for _, message in commit_records]
    references_by_commit: list[tuple[str, set[int]]] = []
    for sha, message in commit_records:
        references_by_commit.append((sha, issue_numbers(message, repository)))
    references = {number for _, numbers in references_by_commit for number in numbers}
    if not references or (not is_release_branch(branch) and len(references) != 1):
        raise ContractViolation(
            "PR rangeの参照Issueは同一repositoryのcanonicalなOPEN Issue "
            + ("1件" if not is_release_branch(branch) else "1件以上")
            + "である必要があります: "
            f"件数={len(references)}"
        )
    if not is_release_branch(branch):
        for number in sorted(references):
            _validate_pr_canonical_issue(repository=repository, number=number, issue_loader=issue_loader)
    changed_paths = _pr_changed_paths(repository=repository, base_sha=base_sha, head_sha=head_sha)
    if is_release_branch(branch):
        edge_cache: dict[tuple[str, str], tuple[tuple[str, tuple[str, ...]], ...]] = {}
        tree_cache: dict[str, TreeEntries] = {}
        commit_cache: dict[str, dict[str, object]] = {}
        blob_cache: dict[str, bytes] = {}
        effective = _effective_release_issue_numbers(
            repository=repository,
            references_by_commit=references_by_commit,
            net_paths=changed_paths,
            issue_loader=issue_loader,
            commit_paths=lambda sha: _pr_commit_paths(repository, sha, commit_cache),
            commit_surviving_paths=lambda sha, paths: _pr_surviving_paths_at_head(
                repository, sha, head_sha, paths, edge_cache, tree_cache, commit_cache, blob_cache
            ),
        )
        if not effective:
            raise ContractViolation("release PRにOPEN Issue参照がありません")
    else:
        effective = references
    validate_contract(
        branch=branch,
        default_branch=None,
        repository=repository,
        commit_messages=commit_messages,
        changed_paths=changed_paths,
        issue_loader=issue_loader,
        release_issue_numbers=effective if is_release_branch(branch) else None,
    )
    return effective


def _pr_commit_paths(
    repository: str, sha: str,
    commit_cache: Optional[dict[str, dict[str, object]]] = None,
) -> list[str]:
    """Fetch complete immutable paths for one non-merge commit; truncation fails closed."""
    payload = None if commit_cache is None else commit_cache.get(sha)
    if payload is None:
        payload = _gh_json(f"repos/{repository}/git/commits/{sha}")
    if not isinstance(payload, dict) or _require_sha(payload.get("sha"), "provenance commit SHA") != sha:
        raise ContractViolation("closed Issue参照commitのprovenanceが不正です")
    if commit_cache is not None:
        commit_cache[sha] = payload
    parents = payload.get("parents")
    if not isinstance(parents, list) or len(parents) != 1 or not isinstance(parents[0], dict):
        raise ContractViolation("merge commitのclosed Issue provenanceは判定できません")
    parent_sha = _require_sha(parents[0].get("sha"), "provenance parent SHA")
    return _pr_changed_paths(repository=repository, base_sha=parent_sha, head_sha=sha)


def _advance_path_state(
    paths: Sequence[str], records: Sequence[tuple[str, tuple[str, ...]]]
) -> set[str]:
    """親commitのpath集合を、1つのimmutable parent-child edgeに沿って進める。"""
    current = set(paths)
    tracked_deletions = {
        record_paths[0]
        for kind, record_paths in records
        if kind == "D" and record_paths[0] in current
    }
    additions = {
        record_paths[0]
        for kind, record_paths in records
        if kind == "A"
    }
    if tracked_deletions and additions:
        raise ContractViolation(
            "closed Issue provenanceでtracked pathの削除と同一edgeの追加がありrename判定できません"
        )
    removed: set[str] = set()
    added: set[str] = set()
    for kind, record_paths in records:
        if kind == "R" and record_paths[0] in current:
            removed.add(record_paths[0])
            added.add(record_paths[1])
        elif kind == "C" and record_paths[0] in current:
            added.add(record_paths[1])
        elif kind == "D" and record_paths[0] in current:
            removed.add(record_paths[0])
    return (current - removed) | added


def _prove_path_additions(
    paths: Sequence[str],
    records: Sequence[tuple[str, tuple[str, ...]]],
    parent_sha: str,
    commit_sha: str,
    parents: Sequence[str],
    states: dict[str, set[str]],
    tree_loader: Callable[[str], TreeEntries],
    copy_detector: Optional[Callable[[TreeEntries, TreeEntries], dict[str, str]]] = None,
) -> tuple[tuple[str, tuple[str, ...]], ...]:
    """追加の同一blobコピーと、別parentから来た独立追加だけを証明する。"""
    additions = [record_paths[0] for kind, record_paths in records if kind == "A"]
    if not additions:
        return tuple(records)
    if len(parents) == 1 and any(
        kind == "D" and record_paths[0] in paths for kind, record_paths in records
    ):
        # 大幅編集を伴う移動はblob一致では証明できないため、従来どおり拒否する。
        return tuple(records)
    parent_tree = tree_loader(parent_sha)
    child_tree = tree_loader(commit_sha)
    tracked_entries: dict[tuple[str, str], str] = {}
    for path in paths:
        entry = parent_tree.get(path)
        if entry is None:
            raise ContractViolation("closed Issue provenanceのtracked pathがparent treeにありません")
        tracked_entries[(entry[0], entry[2])] = path
    all_tracked_entries = set(tracked_entries)
    other_trees: list[tuple[str, TreeEntries]] = []
    if len(parents) > 1:
        for other_sha in parents:
            if other_sha == parent_sha:
                continue
            other_tree = tree_loader(other_sha)
            other_trees.append((other_sha, other_tree))
            for path in states.get(other_sha, set()):
                entry = other_tree.get(path)
                if entry is None:
                    raise ContractViolation("closed Issue provenanceのtracked pathがparent treeにありません")
                all_tracked_entries.add((entry[0], entry[2]))
    proven: list[tuple[str, tuple[str, ...]]] = []
    for kind, record_paths in records:
        if kind != "A":
            proven.append((kind, record_paths))
            continue
        path = record_paths[0]
        entry = child_tree.get(path)
        if entry is None or path in parent_tree:
            raise ContractViolation("closed Issue provenanceの追加pathがimmutable treeと一致しません")
        content_identity = (entry[0], entry[2])
        if content_identity in tracked_entries:
            proven.append(("C", (tracked_entries[content_identity], path)))
        elif content_identity not in all_tracked_entries and any(
            other_tree.get(path) == entry and path not in states.get(other_sha, set())
            for other_sha, other_tree in other_trees
        ):
            # mergeの同一path・同一blobが別parentですでに存在する場合だけ除外する。
            continue
        else:
            proven.append((kind, record_paths))
    if copy_detector is not None:
        candidates = {
            record_paths[0]: child_tree[record_paths[0]]
            for kind, record_paths in proven if kind == "A"
            and child_tree[record_paths[0]][0] == "blob"
        }
        sources = {path: parent_tree[path] for path in paths if parent_tree[path][0] == "blob"}
        if candidates and sources:
            copies = copy_detector(sources, candidates)
            proven = [
                ("C", (copies[record_paths[0]], record_paths[0]))
                if kind == "A" and record_paths[0] in copies else (kind, record_paths)
                for kind, record_paths in proven
            ]
    return tuple(proven)


def _pr_blob_bytes(repository: str, sha: str, cache: dict[str, bytes]) -> bytes:
    if sha in cache:
        return cache[sha]
    payload = _gh_json(f"repos/{repository}/git/blobs/{sha}")
    if not isinstance(payload, dict) or _require_sha(payload.get("sha"), "provenance blob SHA") != sha:
        raise ContractViolation("closed Issue provenanceのblob SHAが一致しません")
    size, content = payload.get("size"), payload.get("content")
    if (
        payload.get("encoding") != "base64" or type(size) is not int
        or not 0 <= size <= _MAX_PROVENANCE_BLOB_BYTES or not isinstance(content, str)
        or len(content) > (_MAX_PROVENANCE_BLOB_BYTES * 4 // 3 + 4) * 2
    ):
        raise ContractViolation("closed Issue provenanceのblob responseが不正か上限超過です")
    try:
        decoded = base64.b64decode(content.replace("\n", ""), validate=True)
    except (binascii.Error, ValueError) as error:
        raise ContractViolation("closed Issue provenanceのblob encodingが不正です") from error
    if len(decoded) != size or hashlib.sha1(f"blob {size}\0".encode() + decoded).hexdigest() != sha:
        raise ContractViolation("closed Issue provenanceのblob contentがimmutable SHAと一致しません")
    if sum(len(data) for data in cache.values()) + size > _MAX_PROVENANCE_BLOB_BYTES:
        raise ContractViolation("closed Issue provenanceのblob cacheが上限超過です")
    cache[sha] = decoded
    return decoded


def _native_copy_sources(
    sources: TreeEntries, candidates: TreeEntries, blob_loader: Callable[[str], bytes]
) -> dict[str, str]:
    """未証明の追加だけに既存Gitのcopy判定を適用し、object DBを本repoから隔離する。"""
    environment = {
        name: value for name, value in os.environ.items() if not name.startswith("GIT_")
    }
    environment.update({"GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": os.devnull})
    with tempfile.TemporaryDirectory(prefix="krr-provenance-") as directory:
        object_db = Path(directory) / "objects.git"

        def git(*arguments: str, data: bytes = b"") -> bytes:
            result = subprocess.run(
                ["git", "--git-dir", str(object_db), "-c", f"core.attributesFile={os.devnull}", *arguments], input=data,
                cwd=directory, env=environment, capture_output=True, check=False,
            )
            if result.returncode != 0:
                raise ContractViolation("closed Issue provenanceの隔離Git copy判定に失敗しました")
            return result.stdout

        git("init", "--bare", "--template=", str(object_db))
        for sha in sorted({entry[2] for entry in [*sources.values(), *candidates.values()]}):
            actual_sha = git("hash-object", "-w", "--stdin", data=blob_loader(sha)).decode().strip()
            if actual_sha != sha:
                raise ContractViolation("closed Issue provenanceの隔離blob SHAが一致しません")

        def tree(entries: TreeEntries) -> str:
            directories: dict[str, dict[str, tuple[str, str, str]]] = {"": {}}
            for path, entry in entries.items():
                parts = path.split("/")
                parent = ""
                for component in parts[:-1]:
                    child = f"{parent}/{component}" if parent else component
                    directories.setdefault(child, {})
                    parent = child
                directories[parent][parts[-1]] = entry
            for directory_name in sorted(directories, key=lambda name: name.count("/") + bool(name), reverse=True):
                records = b"".join(
                    f"{mode} {entry_type} {sha}\t{name}\0".encode()
                    for name, (entry_type, mode, sha) in sorted(directories[directory_name].items())
                )
                tree_sha = git("mktree", "-z", data=records).decode().strip()
                if not directory_name:
                    return tree_sha
                parent, _, name = directory_name.rpartition("/")
                if name in directories[parent]:
                    raise ContractViolation("closed Issue provenanceの隔離tree pathが衝突しています")
                directories[parent][name] = ("tree", "040000", tree_sha)
            raise ContractViolation("closed Issue provenanceの隔離treeを作成できません")

        before = tree(sources)
        after = tree({**sources, **candidates})
        records = _parse_name_status_records(git(
            "diff", "--no-ext-diff", "--no-textconv", "--name-status", "-z",
            "--find-renames", "--find-copies", "--find-copies-harder", before, after,
        ).decode())
        return {
            paths[1]: paths[0] for kind, paths in records
            if kind == "C" and paths[0] in sources and paths[1] in candidates
        }


def _pr_surviving_paths_at_head(
    repository: str,
    sha: str,
    head_sha: str,
    source_paths: Sequence[str],
    edge_cache: dict[tuple[str, str], tuple[tuple[str, tuple[str, ...]], ...]],
    tree_cache: Optional[dict[str, TreeEntries]] = None,
    commit_cache: Optional[dict[str, dict[str, object]]] = None,
    blob_cache: Optional[dict[str, bytes]] = None,
) -> list[str]:
    """Trace source paths through every source-descendant parent edge to the PR head."""
    sha = _require_sha(sha, "provenance commit SHA")
    head_sha = _require_sha(head_sha, "provenance head SHA")
    if sha == head_sha:
        return list(source_paths)
    comparison = _gh_json(f"repos/{repository}/compare/{sha}...{head_sha}")
    if not isinstance(comparison, dict):
        raise ContractViolation("closed Issue rename provenanceのcompare responseが不正です")
    base_commit = comparison.get("base_commit")
    if not isinstance(base_commit, dict) or _require_sha(
        base_commit.get("sha"), "rename provenance base SHA"
    ) != sha:
        raise ContractViolation("closed Issue rename provenanceのbase SHAが一致しません")
    commits = comparison.get("commits")
    total_commits = comparison.get("total_commits")
    ahead_by = comparison.get("ahead_by")
    if (
        not isinstance(commits, list)
        or not commits
        or type(total_commits) is not int
        or total_commits != len(commits)
        or type(ahead_by) is not int
        or ahead_by != total_commits
    ):
        raise ContractViolation("closed Issue rename provenanceのcommit一覧が不完全です")
    parents_by_commit: dict[str, tuple[str, ...]] = {}
    tree_cache = {} if tree_cache is None else tree_cache
    commit_cache = {} if commit_cache is None else commit_cache
    blob_cache = {} if blob_cache is None else blob_cache

    def tree_loader(commit_sha: str) -> TreeEntries:
        if commit_sha not in tree_cache:
            tree_cache[commit_sha] = _git_tree_entries(
                repository=repository, commit_sha=commit_sha,
                label="closed Issue provenance", commit_payload=commit_cache.get(commit_sha),
            )
        return tree_cache[commit_sha]

    for commit_record in commits:
        if not isinstance(commit_record, dict):
            raise ContractViolation("closed Issue rename provenanceのcommit entryが不正です")
        commit_sha = _require_sha(commit_record.get("sha"), "rename provenance commit SHA")
        if commit_sha in parents_by_commit:
            raise ContractViolation("closed Issue rename provenanceのcommit一覧に重複があります")
        commit_payload = commit_cache.get(commit_sha)
        if commit_payload is None:
            commit_payload = _gh_json(f"repos/{repository}/git/commits/{commit_sha}")
        if not isinstance(commit_payload, dict) or _require_sha(
            commit_payload.get("sha"), "rename provenance commit SHA"
        ) != commit_sha:
            raise ContractViolation("closed Issue rename provenance commitが不正です")
        commit_cache[commit_sha] = commit_payload
        parents = commit_payload.get("parents")
        if not isinstance(parents, list) or any(not isinstance(parent, dict) for parent in parents):
            raise ContractViolation("closed Issue rename provenanceのparent一覧が不正です")
        parents_by_commit[commit_sha] = tuple(
            _require_sha(parent.get("sha"), "rename provenance parent SHA")
            for parent in parents
        )
    states: dict[str, set[str]] = {sha: set(source_paths)}
    pending = set(parents_by_commit)
    while pending:
        ready = [
            commit_sha
            for commit_sha in pending
            if all(parent_sha not in pending for parent_sha in parents_by_commit[commit_sha])
        ]
        if not ready:
            raise ContractViolation("closed Issue rename provenanceのcommit DAGにcycleがあります")
        for commit_sha in ready:
            current: set[str] = set()
            for parent_sha in parents_by_commit[commit_sha]:
                if parent_sha not in states or not states[parent_sha]:
                    continue
                edge = _pr_path_edge_records(
                    repository, parent_sha, commit_sha, edge_cache, tree_loader
                )
                proven_edge = _prove_path_additions(
                    states[parent_sha], edge, parent_sha, commit_sha,
                    parents_by_commit[commit_sha], states, tree_loader,
                    lambda sources, candidates: _native_copy_sources(
                        sources, candidates, lambda blob_sha: _pr_blob_bytes(repository, blob_sha, blob_cache)
                    ),
                )
                current.update(_advance_path_state(states[parent_sha], proven_edge))
            if any(parent_sha in states for parent_sha in parents_by_commit[commit_sha]):
                states[commit_sha] = current
            pending.remove(commit_sha)
    if head_sha not in states:
        raise ContractViolation("closed Issue rename provenanceが指定headに到達していません")
    return sorted(states[head_sha])


def _pr_path_edge_records(
    repository: str,
    parent_sha: str,
    commit_sha: str,
    edge_cache: dict[tuple[str, str], tuple[tuple[str, tuple[str, ...]], ...]],
    tree_loader: Optional[Callable[[str], TreeEntries]] = None,
) -> tuple[tuple[str, tuple[str, ...]], ...]:
    key = (parent_sha, commit_sha)
    if key in edge_cache:
        return edge_cache[key]
    comparison = _gh_json(
        f"repos/{repository}/compare/{parent_sha}...{commit_sha}"
    )
    if not isinstance(comparison, dict):
        raise ContractViolation("closed Issue rename provenanceのcommit compareが不正です")
    base_commit = comparison.get("base_commit")
    if not isinstance(base_commit, dict) or _require_sha(
        base_commit.get("sha"), "rename provenance parent SHA"
    ) != parent_sha:
        raise ContractViolation("closed Issue rename provenanceのparent SHAが一致しません")
    files = comparison.get("files")
    if not isinstance(files, list):
        raise ContractViolation(
            f"closed Issue rename provenanceのfile一覧が不完全です: {parent_sha}...{commit_sha}"
        )
    if len(files) >= 300:
        if tree_loader is None:
            raise ContractViolation(
                f"closed Issue rename provenanceのfile一覧が不完全です: {parent_sha}...{commit_sha}"
            )
        # compareの上限に達したedgeは、完全性を検証済みのimmutable treeで置き換える。
        parent_tree = tree_loader(parent_sha)
        child_tree = tree_loader(commit_sha)
        paths = sorted(set(parent_tree) | set(child_tree))
        tree_records: list[tuple[str, tuple[str, ...]]] = []
        for path in paths:
            before = parent_tree.get(path)
            after = child_tree.get(path)
            if before == after:
                continue
            before = before if before is not None and before[0] != "tree" else None
            after = after if after is not None and after[0] != "tree" else None
            if before is None and after is None:
                continue
            kind = "A" if before is None else "D" if after is None else "M"
            tree_records.append((kind, (path,)))
        edge_cache[key] = tuple(tree_records)
        return edge_cache[key]
    records: list[tuple[str, tuple[str, ...]]] = []
    for changed_file in files:
        if not isinstance(changed_file, dict):
            raise ContractViolation("closed Issue rename provenanceのfile entryが不正です")
        filename = changed_file.get("filename")
        status = changed_file.get("status")
        if (
            not isinstance(filename, str)
            or not filename
            or status not in {"added", "removed", "modified", "renamed"}
        ):
            raise ContractViolation("closed Issue rename provenanceのfile情報が不正です")
        previous_filename = changed_file.get("previous_filename")
        if status == "renamed":
            if not isinstance(previous_filename, str) or not previous_filename:
                raise ContractViolation("closed Issue rename provenanceのprevious_filenameが不正です")
            records.append(("R", (previous_filename, filename)))
        elif previous_filename is not None:
            raise ContractViolation("closed Issue rename provenanceに予期しないprevious_filenameがあります")
        else:
            kind = {"added": "A", "removed": "D", "modified": "M"}[status]
            records.append((kind, (filename,)))
    edge_cache[key] = tuple(records)
    return edge_cache[key]


def _run_git(repository: Path, *arguments: str) -> str:
    result = subprocess.run(
        ["git", "--no-replace-objects", *arguments],
        cwd=repository,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip()
        raise ContractViolation(f"git {' '.join(arguments)} failed: {detail}")
    return result.stdout.strip()


def _is_ancestor(repository: Path, ancestor: str, descendant: str) -> bool:
    """fast-forward関係を判定し、Git実行失敗は拒否する。"""
    result = subprocess.run(
        ["git", "--no-replace-objects", "merge-base", "--is-ancestor", ancestor, descendant],
        cwd=repository,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode == 0:
        return True
    if result.returncode == 1:
        return False
    raise ContractViolation("push rangeのancestor判定に失敗しました")


def push_validation_range_base(
    repository: Path,
    *,
    remote_sha: str,
    local_sha: str,
    default_range_base: str,
) -> str:
    """通常のfast-forward pushではremote先端を起点にする。

    feature branchがremote先端より後のlive defaultをmergeした場合は、
    live defaultから到達可能なcommitをIssue検査へ含めない。non-fast-forward
    更新は信頼できる増分範囲を持たないため、live default branchから検証する。
    """
    remote_sha = _require_sha(remote_sha, "push remote SHA")
    local_sha = _require_sha(local_sha, "push local SHA")
    if remote_sha == _ZERO_SHA:
        return default_range_base
    if _is_ancestor(repository, remote_sha, local_sha):
        if _is_ancestor(repository, default_range_base, local_sha) and not _is_ancestor(
            repository, default_range_base, remote_sha
        ):
            return default_range_base
        return remote_sha
    return default_range_base


def _repository_name(remote_url: str) -> str:
    patterns = (
        r"^git@github\.com:(?P<repository>[^/]+/[^/]+?)(?:\.git)?$",
        r"^https://github\.com/(?P<repository>[^/]+/[^/]+?)(?:\.git)?/?$",
        r"^ssh://git@github\.com/(?P<repository>[^/]+/[^/]+?)(?:\.git)?/?$",
    )
    for pattern in patterns:
        match = re.match(pattern, remote_url)
        if match is not None:
            return match.group("repository")
    raise ContractViolation("GitHub repositoryをremote URLから判定できません")


def branch_remote(repository: Path, branch: str) -> str:
    configured = subprocess.run(
        ["git", "config", f"branch.{branch}.remote"],
        cwd=repository,
        capture_output=True,
        text=True,
        check=False,
    )
    if configured.returncode == 0 and configured.stdout.strip():
        return configured.stdout.strip()
    remotes = _run_git(repository, "remote").splitlines()
    if "origin" in remotes:
        return "origin"
    if len(remotes) == 1:
        return remotes[0]
    raise ContractViolation(f"branch {branch}のpush remoteを判定できません")


def _configured_remote_for_url(repository: Path, remote_url: str) -> str:
    requested = _normalize_remote_url(remote_url)
    matches = [
        remote
        for remote in _run_git(repository, "remote").splitlines()
        if any(
            _normalize_remote_url(configured_url) == requested
            for configured_url in _effective_remote_urls(repository, remote)
        )
    ]
    if len(matches) != 1:
        raise ContractViolation(
            "push URLに対応する設定済remoteを一意に判定できません"
        )
    return matches[0]


def _normalize_remote_url(remote_url: str) -> str:
    return remote_url.strip().removesuffix("/").removesuffix(".git")


def _effective_remote_urls(repository: Path, remote: str) -> tuple[str, ...]:
    """Return every configured push URL, falling back to the fetch URL."""
    try:
        configured = _run_git(repository, "remote", "get-url", "--push", "--all", remote)
    except ContractViolation:
        try:
            configured = _run_git(repository, "remote", "get-url", "--push", remote)
        except ContractViolation:
            configured = ""
    urls = tuple(line.strip() for line in configured.splitlines() if line.strip())
    if urls:
        return urls
    return (_run_git(repository, "remote", "get-url", remote),)


def _matching_push_url(
    repository: Path,
    remote: str,
    requested_url: str | None,
) -> str:
    configured_urls = _effective_remote_urls(repository, remote)
    if requested_url is not None:
        requested = _normalize_remote_url(requested_url)
        matches = [
            configured_url
            for configured_url in configured_urls
            if _normalize_remote_url(configured_url) == requested
        ]
        if not matches:
            raise ContractViolation("--remoteと--remote-urlが同じpush先を示していません")
        selected_url = matches[0]
    else:
        selected_url = configured_urls[0]

    # A remote with push URLs for different repositories is ambiguous.  Keep
    # the hook fail-closed even when the supplied URL happens to match one of
    # those URLs.
    selected_repository = _repository_name(selected_url).casefold()
    if any(
        _repository_name(configured_url).casefold() != selected_repository
        for configured_url in configured_urls
    ):
        raise ContractViolation("remoteに異なるGitHub repositoryのpush URLが混在しています")
    return selected_url


def _remote_for_push(
    repository: Path,
    *,
    remote_name: str | None,
    remote_url: str | None,
    fallback_branch: str | None,
) -> tuple[str, str]:
    """Return the configured remote used for default-ref comparison and its URL."""
    if remote_name:
        if _is_remote_url(remote_name):
            # Keep compatibility with direct invocations that historically passed a URL
            # to --remote, while the hook itself passes the remote name separately.
            if remote_url is not None and (
                _normalize_remote_url(remote_name) != _normalize_remote_url(remote_url)
            ):
                raise ContractViolation("--remoteと--remote-urlが同じpush先を示していません")
            remote_url = remote_url or remote_name
        else:
            configured_url = _matching_push_url(repository, remote_name, remote_url)
            return remote_name, configured_url

    if remote_url:
        remote = _configured_remote_for_url(repository, remote_url)
        return remote, _matching_push_url(repository, remote, remote_url)

    if fallback_branch:
        remote = branch_remote(repository, fallback_branch)
        return remote, _matching_push_url(repository, remote, None)

    remotes = _run_git(repository, "remote").splitlines()
    if "origin" in remotes:
        return "origin", _matching_push_url(repository, "origin", None)
    if len(remotes) == 1:
        remote = remotes[0]
        return remote, _matching_push_url(repository, remote, None)
    raise ContractViolation("push remoteを設定済remoteから一意に判定できません")


def _is_remote_url(value: str) -> bool:
    return value.startswith(("git@", "ssh://", "https://", "http://"))


def _run_remote_git(
    repository: Path,
    failure_message: str,
    *arguments: str,
) -> str:
    """Run a network Git operation without exposing remote diagnostics."""
    try:
        result = subprocess.run(
            ["git", *arguments],
            cwd=repository,
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError:
        raise ContractViolation(failure_message) from None
    if result.returncode != 0:
        raise ContractViolation(failure_message)
    return result.stdout.strip()


def _parse_remote_default_head(raw: str) -> tuple[str, str]:
    """Parse a live `git ls-remote --symref <url> HEAD` response strictly."""
    lines = raw.splitlines()
    if len(lines) != 2:
        raise ContractViolation("push remoteのdefault branch応答が不正です")
    symbolic, head = lines
    symbolic_parts = symbolic.split("\t")
    head_parts = head.split("\t")
    if (
        len(symbolic_parts) != 2
        or symbolic_parts[1] != "HEAD"
        or not symbolic_parts[0].startswith("ref: refs/heads/")
        or len(head_parts) != 2
        or head_parts[1] != "HEAD"
    ):
        raise ContractViolation("push remoteのdefault branch応答が不正です")
    branch = symbolic_parts[0].removeprefix("ref: refs/heads/")
    if not _is_remote_ref(f"refs/heads/{branch}"):
        raise ContractViolation("push remoteのdefault branch名が不正です")
    return branch, _require_sha(head_parts[0], "push remoteのdefault branch SHA")


def _live_remote_default_head(repository: Path, pushed_remote_url: str) -> tuple[str, str]:
    return _parse_remote_default_head(
        _run_remote_git(
            repository,
            "push remoteのdefault branch取得に失敗しました",
            "ls-remote",
            "--symref",
            pushed_remote_url,
            "HEAD",
        )
    )


def _bind_remote_default_ref(
    repository: Path,
    *,
    remote: str,
    pushed_remote_url: str,
    repository_name: str,
) -> tuple[str, str]:
    """Fetch and bind the validation range to the remote's current default tip.

    The local tracking ref may be stale when pre-push starts.  Read the remote
    default branch, fetch that exact branch into its tracking ref, then read it
    again so a concurrent default-branch advance fails closed rather than
    validating against an obsolete range.
    """
    try:
        pushed_repository = _repository_name(pushed_remote_url)
    except ContractViolation:
        raise ContractViolation("push remoteのGitHub repositoryを判定できません") from None
    if pushed_repository.casefold() != repository_name.casefold():
        raise ContractViolation("push remoteと対象GitHub repositoryが一致しません")
    branch, expected_sha = _live_remote_default_head(repository, pushed_remote_url)
    default_ref = f"refs/remotes/{remote}/{branch}"
    _run_remote_git(
        repository,
        "push remoteのdefault branch fetchに失敗しました",
        "fetch",
        "--no-tags",
        "--no-write-fetch-head",
        pushed_remote_url,
        f"+refs/heads/{branch}:{default_ref}",
    )
    fetched_sha = _require_sha(
        _run_git(repository, "rev-parse", default_ref),
        "push remoteのfetch済default branch SHA",
    )
    current_branch, current_sha = _live_remote_default_head(repository, pushed_remote_url)
    if (
        current_branch != branch
        or current_sha != expected_sha
        or fetched_sha != current_sha
    ):
        raise ContractViolation(
            "push remoteのdefault branchが検証中に更新されたためpushを中止します"
        )
    return branch, f"{remote}/{branch}"


def _load_issue(repository_name: str, number: int) -> Issue | None:
    result = subprocess.run(
        [
            "gh",
            "issue",
            "view",
            str(number),
            "--repo",
            repository_name,
            "--json",
            "number,state,body,url,updatedAt",
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        return None
    payload = json.loads(result.stdout)
    if not isinstance(payload, dict):
        raise ContractViolation("GitHub Issue responseの形式が不正です")
    updated_at = payload.get("updatedAt")
    return Issue(
        number=int(payload["number"]),
        state=str(payload["state"]),
        body=str(payload.get("body") or ""),
        url=str(payload["url"]),
        # Keep the push contract's historical empty/null body normalization.
        # Freshness consumers reject a missing updatedAt in their snapshot.
        updated_at=updated_at if isinstance(updated_at, str) else "",
    )


def _local_commit_paths(repository: Path, sha: str) -> list[str]:
    parents = _run_git(repository, "rev-list", "--parents", "-n", "1", sha).split()
    if len(parents) != 2 or parents[0] != sha:
        raise ContractViolation("merge commitまたは不完全なclosed Issue provenanceです")
    return parse_name_status_paths(
        _run_git(repository, "diff", "--name-status", "-z", "--find-renames", parents[1], sha)
    )


def _local_surviving_paths_at_head(
    repository: Path,
    sha: str,
    head_sha: str,
    source_paths: Sequence[str],
    edge_cache: dict[tuple[str, str], tuple[tuple[str, tuple[str, ...]], ...]],
    tree_cache: Optional[dict[str, TreeEntries]] = None,
) -> list[str]:
    """全parent edgeを辿り、mergeとrename後の編集・削除を反映する。"""
    sha = _require_sha(sha, "provenance commit SHA")
    head_sha = _require_sha(head_sha, "provenance head SHA")
    if sha == head_sha:
        return list(source_paths)
    if not _is_ancestor(repository, sha, head_sha):
        raise ContractViolation("closed Issue rename provenance commitが検証対象headのancestorではありません")
    commits = _run_git(
        repository,
        "rev-list",
        "--reverse",
        "--topo-order",
        "--ancestry-path",
        f"{sha}..{head_sha}",
    ).splitlines()
    if not commits or head_sha not in commits:
        raise ContractViolation("closed Issue rename provenanceのcommit一覧が不完全です")
    parents_by_commit: dict[str, tuple[str, ...]] = {}
    tree_cache = {} if tree_cache is None else tree_cache

    def tree_loader(commit_sha: str) -> TreeEntries:
        if commit_sha not in tree_cache:
            raw = _run_git(repository, "ls-tree", "-r", "-z", "--full-tree", commit_sha)
            entries: TreeEntries = {}
            for record in raw.split("\x00"):
                if not record:
                    continue
                metadata, separator, path = record.partition("\t")
                fields = metadata.split()
                if not separator or len(fields) != 3 or not path or path in entries:
                    raise ContractViolation("closed Issue provenanceのlocal tree entryが不正です")
                mode, entry_type, entry_sha = fields
                entries[path] = (entry_type, mode, _require_sha(entry_sha, "local tree entry SHA"))
            tree_cache[commit_sha] = entries
        return tree_cache[commit_sha]

    for commit_sha in commits:
        fields = _run_git(repository, "rev-list", "--parents", "-n", "1", commit_sha).split()
        if not fields or fields[0] != commit_sha:
            raise ContractViolation("closed Issue rename provenanceのcommit情報が不正です")
        parents_by_commit[commit_sha] = tuple(fields[1:])
    states: dict[str, set[str]] = {sha: set(source_paths)}
    pending = set(parents_by_commit)
    while pending:
        ready = [
            commit_sha
            for commit_sha in pending
            if all(parent_sha not in pending for parent_sha in parents_by_commit[commit_sha])
        ]
        if not ready:
            raise ContractViolation("closed Issue rename provenanceのcommit DAGにcycleがあります")
        for commit_sha in ready:
            current: set[str] = set()
            for parent_sha in parents_by_commit[commit_sha]:
                if parent_sha not in states or not states[parent_sha]:
                    continue
                edge = _local_path_edge_records(repository, parent_sha, commit_sha, edge_cache)
                proven_edge = _prove_path_additions(
                    states[parent_sha], edge, parent_sha, commit_sha,
                    parents_by_commit[commit_sha], states, tree_loader,
                )
                current.update(
                    _advance_path_state(states[parent_sha], proven_edge)
                )
            if any(parent_sha in states for parent_sha in parents_by_commit[commit_sha]):
                states[commit_sha] = current
            pending.remove(commit_sha)
    if head_sha not in states:
        raise ContractViolation("closed Issue rename provenanceが指定headに到達していません")
    return sorted(states[head_sha])


def _local_path_edge_records(
    repository: Path,
    parent_sha: str,
    commit_sha: str,
    edge_cache: dict[tuple[str, str], tuple[tuple[str, tuple[str, ...]], ...]],
) -> tuple[tuple[str, tuple[str, ...]], ...]:
    key = (parent_sha, commit_sha)
    if key not in edge_cache:
        raw = _run_git(
            repository,
            "diff",
            "--name-status",
            "-z",
            "--find-renames",
            "--find-copies",
            "--find-copies-harder",
            parent_sha,
            commit_sha,
        )
        edge_cache[key] = tuple(_parse_name_status_records(raw))
    return edge_cache[key]


def main() -> int:
    try:
        parser = argparse.ArgumentParser(description="Validate Issue references on push")
        parser.add_argument(
            "--remote",
            help="the remote used by the current git push (pre-push hook $1)",
        )
        parser.add_argument(
            "--remote-url",
            help="the remote URL used by the current git push (pre-push hook $2)",
        )
        parser.add_argument(
            "--pr-number",
            type=int,
            help="validate this pull request's base..head range through GitHub metadata",
        )
        parser.add_argument(
            "--pr-base-sha",
            help="trusted pull request base SHA for --pr-number mode",
        )
        parser.add_argument(
            "--pr-head-sha",
            help="trusted pull request head SHA for --pr-number mode",
        )
        parser.add_argument(
            "--pr-branch",
            help="trusted pull request head branch for --pr-number mode",
        )
        parser.add_argument(
            "--repository",
            help="GitHub owner/repository for --pr-number mode",
        )
        arguments = parser.parse_args()

        pr_values = (
            arguments.pr_number,
            arguments.pr_base_sha,
            arguments.pr_head_sha,
            arguments.pr_branch,
            arguments.repository,
        )
        if any(value is not None for value in pr_values):
            if not all(value is not None for value in pr_values):
                raise ContractViolation(
                    "PR range modeには--pr-number、--pr-base-sha、--pr-head-sha、"
                    "--pr-branch、--repositoryがすべて必要です"
                )
            if arguments.remote is not None or arguments.remote_url is not None:
                raise ContractViolation("PR range modeで--remoteと--remote-urlは使用できません")
            cache: dict[int, Issue | None] = {}

            def load_issue(number: int) -> Issue | None:
                if number not in cache:
                    cache[number] = _load_issue(arguments.repository, number)
                return cache[number]

            references = validate_pr_range(
                repository=arguments.repository,
                pr_number=arguments.pr_number,
                base_sha=arguments.pr_base_sha,
                head_sha=arguments.pr_head_sha,
                branch=arguments.pr_branch,
                issue_loader=load_issue,
            )
            print(
                "Issue contract passed: "
                f"targets={arguments.pr_branch}, "
                f"issues={','.join(f'#{number}' for number in sorted(references))}"
            )
            return 0

        repository = Path(_run_git(Path.cwd(), "rev-parse", "--show-toplevel"))
        push_input = _read_push_input(sys.stdin)
        updates = parse_push_updates(push_input)
        branch = _run_git(repository, "branch", "--show-current")
        if not updates and not branch:
            raise ContractViolation("空のpre-push入力ではcheckout branchが必要です")

        remote, pushed_remote_url = _remote_for_push(
            repository,
            remote_name=arguments.remote,
            remote_url=arguments.remote_url,
            fallback_branch=branch if not updates else None,
        )
        repository_name = _repository_name(pushed_remote_url)
        default_branch, default_ref = _bind_remote_default_ref(
            repository,
            remote=remote,
            pushed_remote_url=pushed_remote_url,
            repository_name=repository_name,
        )
        # A non-atomic multi-ref push may update the feature branch while the
        # default-branch update is rejected.  Always validate from the live
        # remote tip fetched above; a local default tip is not an accepted
        # substitute for the base of a feature range.
        default_range_base = default_ref
        push_updates = pushed_branch_updates(updates, default_branch=default_branch)
        pushed_remote_heads = {
            (remote_ref.removeprefix("refs/heads/"), local_sha): remote_sha
            for _local_ref, local_sha, remote_ref, remote_sha in updates
            if local_sha != _ZERO_SHA
            and remote_ref.startswith("refs/heads/")
            and remote_ref.removeprefix("refs/heads/") != default_branch
        }
        validation_targets: list[tuple[str, str, str]] = []
        for target_branch, target_revision in push_updates:
            remote_sha = pushed_remote_heads[(target_branch, target_revision)]
            range_base = default_range_base
            if not is_release_branch(target_branch):
                range_base = push_validation_range_base(
                    repository,
                    remote_sha=remote_sha,
                    local_sha=target_revision,
                    default_range_base=default_range_base,
                )
            validation_targets.append((target_branch, target_revision, range_base))
        if not validation_targets and not push_input.strip() and branch != default_branch:
            if not branch:
                raise ContractViolation("空のpre-push入力ではcheckout branchが必要です")
            validation_targets.append((branch, "HEAD", default_range_base))

        if not validation_targets:
            if branch == default_branch:
                print(f"Issue contract skipped on default branch: {branch}")
            else:
                print("Issue contract skipped: no branch push updates")
            return 0

        cache: dict[int, Issue | None] = {}

        def load_issue(number: int) -> Issue | None:
            if number not in cache:
                cache[number] = _load_issue(repository_name, number)
            return cache[number]

        local_edge_cache: dict[tuple[str, str], tuple[tuple[str, tuple[str, ...]], ...]] = {}
        local_tree_cache: dict[str, TreeEntries] = {}
        all_references: set[int] = set()
        for target_branch, target_revision, range_base in validation_targets:
            commit_output = _run_git(
                repository,
                "log",
                "--reverse",
                "-z",
                "--format=%B",
                f"{range_base}..{target_revision}",
            )
            commit_messages = parse_commit_messages(commit_output)
            if is_release_branch(target_branch):
                commit_shas = _run_git(
                    repository,
                    "log",
                    "--reverse",
                    "--format=%H",
                    f"{range_base}..{target_revision}",
                ).splitlines()
                if len(commit_shas) != len(commit_messages):
                    raise ContractViolation("release commit provenanceが不完全です")
            else:
                commit_shas = []
            changed_output = _run_git(
                repository,
                "diff",
                "--name-status",
                "-z",
                "--find-renames",
                "--find-copies",
                "--find-copies-harder",
                f"{range_base}...{target_revision}",
            )
            changed_paths = parse_name_status_paths(changed_output)
            if is_release_branch(target_branch):
                refs_by_commit = [
                    (sha, issue_numbers(message, repository_name))
                    for sha, message in zip(commit_shas, commit_messages)
                ]
                effective = _effective_release_issue_numbers(
                    repository=repository_name,
                    references_by_commit=refs_by_commit,
                    net_paths=changed_paths,
                    issue_loader=load_issue,
                    commit_paths=lambda sha: _local_commit_paths(repository, sha),
                    commit_surviving_paths=lambda sha, paths: _local_surviving_paths_at_head(
                        repository, sha, target_revision, paths, local_edge_cache, local_tree_cache
                    ),
                )
                if not effective:
                    raise ContractViolation("release範囲にOPEN Issue参照がありません")
                all_references.update(effective)
            else:
                for message in commit_messages:
                    all_references.update(issue_numbers(message, repository_name))

            validate_contract(
                branch=target_branch,
                default_branch=default_branch,
                repository=repository_name,
                commit_messages=commit_messages,
                changed_paths=changed_paths,
                issue_loader=load_issue,
                release_issue_numbers=effective if is_release_branch(target_branch) else None,
            )

        references = sorted(all_references)
        print(
            "Issue contract passed: "
            f"targets={','.join(target[0] for target in validation_targets)}, "
            f"issues={','.join(f'#{number}' for number in references)}"
        )
        return 0
    except (ContractViolation, json.JSONDecodeError) as error:
        print(f"Issue contract failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
