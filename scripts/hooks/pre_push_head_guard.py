#!/usr/bin/env python3
from __future__ import annotations

import os
import stat
import subprocess
import sys
from pathlib import Path

from verify_push_issue import ContractViolation, parse_push_updates


def _git_head(repository: Path) -> str:
    result = subprocess.run(
        ["git", "--no-replace-objects", "rev-parse", "HEAD"],
        cwd=repository,
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip()


def _git_bytes(repository: Path, *arguments: str) -> bytes:
    result = subprocess.run(
        ["git", "--no-replace-objects", *arguments],
        cwd=repository,
        check=True,
        capture_output=True,
    )
    return result.stdout


def _supports_worktree_executable_mode(repository: Path) -> bool:
    if os.name != "posix":
        return False
    try:
        configured = _git_bytes(repository, "config", "--bool", "--get", "core.filemode")
    except subprocess.CalledProcessError as error:
        if error.returncode != 1:
            raise
        return True
    return configured.strip() == b"true"


def _checkout_configuration(repository: Path) -> dict[str, str]:
    arguments = [
        "git",
        "--no-replace-objects",
        "config",
        "--type=bool-or-str",
        "--null",
        "--get-regexp",
        r"^core\.(autocrlf|eol)$",
    ]
    result = subprocess.run(
        arguments,
        cwd=repository,
        capture_output=True,
        check=False,
    )
    if result.returncode == 1:
        return {}
    if result.returncode != 0:
        raise subprocess.CalledProcessError(
            result.returncode, arguments, output=result.stdout, stderr=result.stderr
        )
    configuration: dict[str, str] = {}
    for record in result.stdout.split(b"\0"):
        if not record:
            continue
        key, separator, value = record.partition(b"\n")
        if not separator:
            raise ContractViolation("Git checkout設定を確認できません")
        try:
            configuration[key.decode("ascii").lower()] = value.decode("ascii").lower()
        except UnicodeDecodeError as error:
            raise ContractViolation("Git checkout設定を確認できません") from error
    return configuration


def _cached_checkout_attributes(
    repository: Path, names: list[str]
) -> dict[str, dict[str, str]]:
    attributes = ("text", "eol", "filter", "working-tree-encoding")
    name_set = set(names)
    arguments = [
        "git",
        "--no-replace-objects",
        "check-attr",
        "--cached",
        "-z",
        "--stdin",
        *attributes,
    ]
    input_bytes = b"".join(os.fsencode(name) + b"\0" for name in names)
    result = subprocess.run(
        arguments,
        cwd=repository,
        check=True,
        capture_output=True,
        input=input_bytes,
    )
    fields = result.stdout.split(b"\0")
    if fields and fields[-1] == b"":
        fields.pop()
    expected_fields = len(names) * len(attributes) * 3
    if len(fields) != expected_fields:
        raise ContractViolation("Git checkout属性を確認できません")
    values: dict[str, dict[str, str]] = {}
    for offset in range(0, len(fields), 3):
        name, attribute, value = fields[offset:offset + 3]
        try:
            decoded_name = os.fsdecode(name)
            decoded_attribute = attribute.decode("ascii")
            decoded_value = value.decode("ascii").lower()
        except UnicodeDecodeError as error:
            raise ContractViolation("Git checkout属性を確認できません") from error
        if decoded_attribute not in attributes or decoded_name not in name_set:
            raise ContractViolation("Git checkout属性を確認できません")
        values.setdefault(decoded_name, {})[decoded_attribute] = decoded_value
    if set(values) != set(names) or any(
        set(values[name]) != set(attributes) for name in names
    ):
        raise ContractViolation("Git checkout属性を確認できません")
    return values


def _index_eol_classifications(repository: Path) -> dict[str, str]:
    result = subprocess.run(
        ["git", "--no-replace-objects", "ls-files", "--eol", "-z"],
        cwd=repository,
        check=True,
        capture_output=True,
    )
    classifications: dict[str, str] = {}
    for record in result.stdout.split(b"\0"):
        if not record:
            continue
        metadata, separator, raw_name = record.partition(b"\t")
        fields = metadata.split()
        if not separator or not fields:
            raise ContractViolation("Git index EOL属性を確認できません")
        try:
            name = os.fsdecode(raw_name)
            index_class = fields[0].decode("ascii")
        except UnicodeDecodeError as error:
            raise ContractViolation("Git index EOL属性を確認できません") from error
        classifications[name] = index_class
    return classifications


def _is_builtin_crlf_materialization(
    committed: bytes,
    actual: bytes,
    attributes: dict[str, str],
    configuration: dict[str, str],
    index_eol: str | None,
) -> bool:
    if not committed or b"\0" in committed or b"\r" in committed or b"\n" not in committed:
        return False
    if actual != committed.replace(b"\n", b"\r\n"):
        return False
    if attributes.get("filter") not in ("unspecified", "unset"):
        return False
    if attributes.get("working-tree-encoding") not in ("unspecified", "unset"):
        return False

    text = attributes.get("text")
    eol = attributes.get("eol")
    autocrlf = configuration.get("core.autocrlf")
    core_eol = configuration.get("core.eol")
    if text not in ("set", "auto", "unspecified"):
        return False
    if eol not in ("crlf", "lf", "input", "unspecified"):
        return False
    if autocrlf not in (
        None,
        "true",
        "yes",
        "on",
        "1",
        "false",
        "no",
        "off",
        "0",
        "input",
    ):
        return False
    if core_eol not in (None, "crlf", "lf", "native"):
        return False
    if eol in ("lf", "input") or text == "unset":
        return False

    if text == "unspecified":
        if eol == "crlf":
            auto_text = False
        elif autocrlf == "true":
            auto_text = True
        else:
            return False
    else:
        auto_text = text == "auto"

    if auto_text and index_eol != "i/lf":
        return False
    if eol == "crlf":
        return True
    if autocrlf == "input":
        return False
    if autocrlf == "true":
        return True
    effective_core_eol = core_eol or "native"
    if effective_core_eol == "crlf":
        return True
    if effective_core_eol == "native":
        return os.linesep == "\r\n"
    return False


def _assert_checkout_matches_head(repository: Path, reviewed_head: str) -> None:
    tree_records = _git_bytes(
        repository, "ls-tree", "-r", "-z", "--full-tree", reviewed_head
    ).split(b"\0")
    tree: dict[str, tuple[str, str]] = {}
    for record in tree_records:
        if not record:
            continue
        metadata, raw_name = record.split(b"\t", 1)
        mode, object_type, object_id = metadata.decode("ascii").split()
        name = os.fsdecode(raw_name)
        if object_type == "commit":
            raise ContractViolation(f"submoduleは品質確認できません: {name}")
        tree[name] = (mode, object_id)

    index_records = _git_bytes(repository, "ls-files", "--stage", "-z").split(b"\0")
    index: dict[str, tuple[str, str]] = {}
    for record in index_records:
        if not record:
            continue
        metadata, raw_name = record.split(b"\t", 1)
        mode, object_id, stage = metadata.decode("ascii").split()
        name = os.fsdecode(raw_name)
        if stage != "0":
            raise ContractViolation("unmerged index cannot be pushed")
        index[name] = (mode, object_id)

    for option in ("-v", "-f"):
        flags = _git_bytes(repository, "ls-files", option, "-z").split(b"\0")
        if any(record and not record.startswith(b"H ") for record in flags):
            raise ContractViolation("git index flagがHEADと一致しません")

    if index != tree:
        raise ContractViolation("git indexがHEADと一致しません")

    untracked = _git_bytes(
        repository, "ls-files", "--others", "--exclude-standard", "-z"
    ).split(b"\0")
    if any(untracked):
        raise ContractViolation("未追跡ファイルが品質確認対象に含まれます")

    compare_executable_mode = _supports_worktree_executable_mode(repository)
    tracked_contents: dict[str, bytes] = {}
    for name, (expected_mode, object_id) in tree.items():
        path = repository / name
        try:
            actual_mode = path.lstat().st_mode
        except (FileNotFoundError, NotADirectoryError) as error:
            raise ContractViolation(f"HEADと一致しません: {name}") from error

        if expected_mode == "120000":
            if not stat.S_ISLNK(actual_mode):
                raise ContractViolation(f"HEADと一致しません: {name}")
            actual_bytes = os.fsencode(os.readlink(path))
        elif expected_mode in ("100644", "100755"):
            if not stat.S_ISREG(actual_mode):
                raise ContractViolation(f"HEADと一致しません: {name}")
            executable = bool(actual_mode & 0o111)
            if compare_executable_mode and executable != (expected_mode == "100755"):
                raise ContractViolation(f"HEADと一致しません: {name}")
            actual_bytes = path.read_bytes()
        else:
            raise ContractViolation(f"HEADのentry modeを確認できません: {name}")

        tracked_contents[name] = actual_bytes

    object_ids = sorted({object_id for _mode, object_id in tree.values()})
    if object_ids:
        batch = subprocess.run(
            ["git", "--no-replace-objects", "cat-file", "--batch"],
            cwd=repository,
            check=True,
            capture_output=True,
            input=("\n".join(object_ids) + "\n").encode("ascii"),
        ).stdout
        offset = 0
        committed_blobs: dict[str, bytes] = {}
        for expected_id in object_ids:
            newline = batch.find(b"\n", offset)
            if newline < 0:
                raise ContractViolation("HEAD blobを読み取れません")
            object_id, object_type, size_text = batch[offset:newline].decode("ascii").split()
            if object_id != expected_id or object_type != "blob":
                raise ContractViolation("HEAD blobを読み取れません")
            size = int(size_text)
            start = newline + 1
            end = start + size
            if end >= len(batch) or batch[end:end + 1] != b"\n":
                raise ContractViolation("HEAD blobを読み取れません")
            committed_blobs[object_id] = batch[start:end]
            offset = end + 1

        mismatched = {
            name: (mode, committed_blobs[object_id], tracked_contents[name])
            for name, (mode, object_id) in tree.items()
            if tracked_contents[name] != committed_blobs[object_id]
        }
        if mismatched:
            names = sorted(mismatched)
            configuration = _checkout_configuration(repository)
            attributes = _cached_checkout_attributes(repository, names)
            index_eol = _index_eol_classifications(repository)
            for name, (mode, committed, actual) in mismatched.items():
                if not _is_builtin_crlf_materialization(
                    committed,
                    actual,
                    attributes[name] if mode in ("100644", "100755") else {},
                    configuration,
                    index_eol.get(name),
                ):
                    raise ContractViolation(f"HEADと一致しません: {name}")

    current_head = _git_head(repository)
    if current_head != reviewed_head:
        raise ContractViolation(
            "品質確認中にcheckout HEADが変わりました: "
            f"{reviewed_head} -> {current_head}"
        )


def validate_push_head(raw_updates: str, reviewed_head: str, repository: Path) -> None:
    if len(reviewed_head) != 40 or any(
        character not in "0123456789abcdef" for character in reviewed_head
    ):
        raise ContractViolation("review対象HEADの形式が不正です")

    updates = parse_push_updates(raw_updates)
    branch_updates = tuple(
        (local_sha, remote_ref)
        for _local_ref, local_sha, remote_ref, _remote_sha in updates
        if remote_ref.startswith("refs/heads/") and local_sha != "0" * 40
    )
    for local_sha, remote_ref in branch_updates:
        if local_sha.lower() != reviewed_head:
            raise ContractViolation(
                f"push対象 {remote_ref} がreview対象HEADと異なります: "
                f"{local_sha} != {reviewed_head}"
            )
    current_head = _git_head(repository)
    if branch_updates and current_head != reviewed_head:
        raise ContractViolation(
            "品質確認中にcheckout HEADが変わりました: "
            f"{reviewed_head} -> {current_head}"
        )
    if branch_updates:
        _assert_checkout_matches_head(repository, reviewed_head)


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: pre_push_head_guard.py REVIEWED_HEAD", file=sys.stderr)
        return 2
    try:
        validate_push_head(sys.stdin.read(), sys.argv[1], Path.cwd())
    except (ContractViolation, subprocess.CalledProcessError) as error:
        print(f"pre-push HEAD check failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
