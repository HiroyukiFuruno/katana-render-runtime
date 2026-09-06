#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
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
_NAME_STATUS_PATTERN = re.compile(r"^(?P<kind>[ACDMRTUXB])(?P<score>\d{1,3})?$")
_WORKFLOW_DIRECTORY_PREFIX = ".github/workflows/"
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
    expected_repository = repository.casefold()
    numbers = {
        int(match.group("number"))
        for match in _FULL_ISSUE_PATTERN.finditer(message)
        if f"{match.group('owner')}/{match.group('repo')}".casefold() == expected_repository
    }
    numbers.update(
        int(match.group("number")) for match in _SHORT_ISSUE_PATTERN.finditer(message)
    )
    return numbers


def closing_issue_numbers(body: str, repository: str) -> set[int]:
    """Return same-repository Issues referenced with a GitHub closing keyword."""

    if not isinstance(body, str):
        raise ContractViolation("PR本文の形式が不正です")
    numbers: set[int] = set()
    for match in _CLOSING_ISSUE_REFERENCE_PATTERN.finditer(body):
        numbers.update(issue_numbers(match.group("reference"), repository))
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
    if not raw:
        return []
    if not raw.endswith("\0"):
        raise ContractViolation("git diff --name-status -z outputが途中で切れています")
    fields = raw.split("\0")[:-1]
    paths: list[str] = []
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
        paths.extend(record_paths)
    return paths


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
) -> None:
    if branch == default_branch:
        return

    referenced_numbers: set[int] = set()
    for index, message in enumerate(commit_messages, start=1):
        references = issue_numbers(message, repository)
        if not references:
            raise ContractViolation(
                f"非default branchのcommit {index}に対象repositoryのIssue参照がありません"
            )
        referenced_numbers.update(references)

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


def _require_sha(value: object, label: str) -> str:
    if not isinstance(value, str) or _SHA_PATTERN.fullmatch(value) is None:
        raise ContractViolation(f"{label}は40文字のSHAである必要があります")
    return value.lower()


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


def _pr_commit_messages(
    *,
    repository: str,
    base_sha: str,
    head_sha: str,
) -> list[str]:
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
    messages: list[str] = []
    commit_shas: list[str] = []
    for commit in commits:
        if not isinstance(commit, dict):
            raise ContractViolation("GitHub compare responseのcommitが不正です")
        commit_shas.append(_require_sha(commit.get("sha"), "compare commit SHA"))
        payload = commit.get("commit")
        if not isinstance(payload, dict) or not isinstance(payload.get("message"), str):
            raise ContractViolation("GitHub compare responseのcommit messageが不正です")
        messages.append(payload["message"])
    # GitHub's compare response does not expose head_commit.  The final item
    # is the head tip only after the full base..head range above was verified.
    if commit_shas[-1] != head_sha:
        raise ContractViolation("GitHub compare responseの最終commitがhead SHAと一致しません")
    return messages


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
    # GitHub returns at most 300 paths for a comparison.  A full page is
    # ambiguous, so never let partial dependency or workflow evidence pass.
    if len(files) >= 300:
        raise ContractViolation(
            "GitHub compare filesが300件上限で打ち切られた可能性があります"
        )
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
    return paths


def _trusted_workflow_path_errors(changed_paths: Sequence[str]) -> list[str]:
    """Reject every PR workflow change that could forge an Actions latch."""
    return sorted(
        {
            path
            for path in changed_paths
            if path.startswith(_WORKFLOW_DIRECTORY_PREFIX)
        }
    )


def _default_advance_workflow_errors(*, repository: str, base_sha: str, trusted_default_sha: str) -> list[str]:
    """Reject a default-branch workflow change since this PR's trusted base.

    The compare API exposes at most 300 changed files.  A full page is
    therefore ambiguous and is rejected instead of letting a stale PR base
    inherit a newer default workflow without validation.
    """
    if base_sha == trusted_default_sha:
        return []
    comparison = _gh_json(f"repos/{repository}/compare/{base_sha}...{trusted_default_sha}")
    if not isinstance(comparison, dict):
        raise ContractViolation("trusted default compare responseの形式が不正です")
    base = comparison.get("base_commit")
    if not isinstance(base, dict) or _require_sha(base.get("sha"), "trusted default compare base SHA") != base_sha:
        raise ContractViolation("trusted default compare responseのbase SHAが一致しません")
    files = comparison.get("files")
    if not isinstance(files, list) or len(files) >= 300:
        raise ContractViolation("trusted default compare filesが打ち切られた可能性があります")
    paths: list[str] = []
    for item in files:
        if not isinstance(item, dict):
            raise ContractViolation("trusted default compare fileの形式が不正です")
        for key in ("filename", "previous_filename"):
            value = item.get(key)
            if value is None and key == "previous_filename":
                continue
            if not isinstance(value, str) or not value:
                raise ContractViolation("trusted default compare file pathが不正です")
            paths.append(value)
    return _trusted_workflow_path_errors(paths)


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
    trusted_default_sha: str | None = None,
) -> set[int]:
    """Validate a PR's base..head contract without checking out PR code."""
    repository = _require_repository_name(repository)
    base_sha = _require_sha(base_sha, "PR base SHA")
    head_sha = _require_sha(head_sha, "PR head SHA")
    if trusted_default_sha is not None:
        trusted_default_sha = _require_sha(trusted_default_sha, "trusted default SHA")
    if pr_number < 1:
        raise ContractViolation("PR番号は正の整数である必要があります")
    if not _is_remote_ref(f"refs/heads/{branch}"):
        raise ContractViolation(f"PR head branchの形式が不正です: {branch!r}")

    commit_messages = _pr_commit_messages(
        repository=repository,
        base_sha=base_sha,
        head_sha=head_sha,
    )
    references: set[int] = set()
    for message in commit_messages:
        references.update(issue_numbers(message, repository))
    if len(references) != 1:
        raise ContractViolation(
            "PR rangeの参照Issueは同一repositoryのcanonicalなOPEN Issue 1件である必要があります: "
            f"件数={len(references)}"
        )
    _validate_pr_canonical_issue(
        repository=repository,
        number=next(iter(references)),
        issue_loader=issue_loader,
    )

    changed_paths = _pr_changed_paths(
        repository=repository, base_sha=base_sha, head_sha=head_sha
    )
    protected_workflows = _trusted_workflow_path_errors(changed_paths)
    if protected_workflows:
        raise ContractViolation(
            "GitHub Actions workflowはPRから変更できません: "
            + ", ".join(protected_workflows)
        )
    if trusted_default_sha is not None:
        default_workflows = _default_advance_workflow_errors(
            repository=repository, base_sha=base_sha, trusted_default_sha=trusted_default_sha,
        )
        if default_workflows:
            raise ContractViolation(
                "trusted default branch上でGitHub Actions workflowが変更されています: "
                + ", ".join(default_workflows)
            )
    validate_contract(
        branch=branch,
        default_branch=None,
        repository=repository,
        commit_messages=commit_messages,
        changed_paths=changed_paths,
        issue_loader=issue_loader,
    )
    return references


def _run_git(repository: Path, *arguments: str) -> str:
    result = subprocess.run(
        ["git", *arguments],
        cwd=repository,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip()
        raise ContractViolation(f"git {' '.join(arguments)} failed: {detail}")
    return result.stdout.strip()


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
        parser.add_argument(
            "--trusted-default-sha",
            help="immutable default-branch SHA whose workflow bytes the writer trusts",
        )
        arguments = parser.parse_args()

        pr_values = (
            arguments.pr_number,
            arguments.pr_base_sha,
            arguments.pr_head_sha,
            arguments.pr_branch,
            arguments.repository,
            arguments.trusted_default_sha,
        )
        if any(value is not None for value in pr_values):
            if not all(value is not None for value in pr_values):
                raise ContractViolation(
                    "PR range modeには--pr-number、--pr-base-sha、--pr-head-sha、"
                    "--pr-branch、--repository、--trusted-default-shaがすべて必要です"
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
                trusted_default_sha=arguments.trusted_default_sha,
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
        push_updates = pushed_branch_updates(updates, default_branch=default_branch)
        validation_targets = list(push_updates) if push_updates else []
        if not validation_targets and not push_input.strip() and branch != default_branch:
            if not branch:
                raise ContractViolation("空のpre-push入力ではcheckout branchが必要です")
            validation_targets.append((branch, "HEAD"))

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

        all_references: set[int] = set()
        for target_branch, target_revision in validation_targets:
            commit_output = _run_git(
                repository,
                "log",
                "--reverse",
                "-z",
                "--format=%B",
                f"{default_ref}..{target_revision}",
            )
            commit_messages = parse_commit_messages(commit_output)
            changed_output = _run_git(
                repository,
                "diff",
                "--name-status",
                "-z",
                "--find-renames",
                "--find-copies",
                "--find-copies-harder",
                f"{default_ref}...{target_revision}",
            )
            changed_paths = parse_name_status_paths(changed_output)
            for message in commit_messages:
                all_references.update(issue_numbers(message, repository_name))

            validate_contract(
                branch=target_branch,
                default_branch=default_branch,
                repository=repository_name,
                commit_messages=commit_messages,
                changed_paths=changed_paths,
                issue_loader=load_issue,
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
