from __future__ import annotations

from collections.abc import Mapping
import hashlib
import os
from pathlib import Path
import re
import sys

from local_review_state import ReviewError, command, strict_json


THREAD_ID = re.compile(r"[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}")
REVIEW_TASK_ID = "pre-pr-self-review"
SHA256 = re.compile(r"[0-9a-f]{64}")


def reviewed_task(ledger: Path, root: Path, thread_id: str) -> Mapping[str, object]:
    try:
        document = strict_json(ledger.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, ReviewError) as error:
        raise ReviewError("作業台帳を読み込めません") from error

    if not isinstance(document, dict) or document.get("mode") != "work":
        raise ReviewError("リリース作業の台帳が必要です")
    cwd = document.get("cwd")
    if (
        not isinstance(cwd, str)
        or Path(cwd).resolve() != root
        or document.get("session_id") != thread_id
    ):
        raise ReviewError("台帳の作業場所またはセッションが一致しません")

    tasks = document.get("tasks")
    if not isinstance(tasks, list):
        raise ReviewError("作業台帳の tasks が不正です")
    reviews = [task for task in tasks if isinstance(task, dict) and task.get("id") == REVIEW_TASK_ID]
    if len(reviews) != 1:
        raise ReviewError("自己レビュー項目が一意ではありません")
    return reviews[0]


def evidence_path(root: Path, value: object, label: str) -> Path:
    if not isinstance(value, str) or not value:
        raise ReviewError(f"最小再現の {label} が不正です")
    relative = Path(value)
    if relative.is_absolute() or ".." in relative.parts:
        raise ReviewError(f"最小再現の {label} はrepository内でなければなりません")
    path = root / relative
    try:
        resolved = path.resolve(strict=True)
    except (OSError, RuntimeError, ValueError) as error:
        raise ReviewError(f"最小再現の {label} を参照できません") from error
    if resolved != path or not path.is_file():
        raise ReviewError(f"最小再現の {label} が通常ファイルではありません")
    return path


def verify_minimum_reproduction(review: Mapping[str, object], root: Path, head: str) -> None:
    failure = review.get("heavy_failure")
    if "heavy_failure" not in review:
        return
    if not isinstance(failure, Mapping) or set(failure) != {"input_digest", "minimum_reproduction"}:
        raise ReviewError("重い失敗の記録が不正です")
    input_digest = failure.get("input_digest")
    reproduction = failure.get("minimum_reproduction")
    if not isinstance(input_digest, str) or SHA256.fullmatch(input_digest) is None:
        raise ReviewError("重い失敗の入力digestが不正です")
    if not isinstance(reproduction, Mapping) or set(reproduction) != {"record", "log", "log_sha256"}:
        raise ReviewError("最小再現の記録が不正です")
    log_sha256 = reproduction.get("log_sha256")
    if not isinstance(log_sha256, str) or SHA256.fullmatch(log_sha256) is None:
        raise ReviewError("最小再現のログhashが不正です")
    log = evidence_path(root, reproduction.get("log"), "ログ")
    record_path = evidence_path(root, reproduction.get("record"), "exit記録")
    try:
        if hashlib.sha256(log.read_bytes()).hexdigest() != log_sha256:
            raise ReviewError("最小再現のログhashが一致しません")
        record = strict_json(record_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError) as error:
        raise ReviewError("最小再現の証跡を読み込めません") from error
    if not isinstance(record, Mapping):
        raise ReviewError("最小再現のexit記録が不正です")
    exit_code = record.get("exit_code")
    argv = record.get("command")
    if (
        type(exit_code) is not int
        or exit_code != 0
        or not isinstance(argv, list)
        or not argv
        or not all(isinstance(part, str) for part in argv)
        or record.get("cwd") != str(root)
        or record.get("input_digest") != input_digest
        or record.get("head_at_start") != head
        or record.get("head_at_end") != head
        or record.get("dirty_at_start") is not False
    ):
        raise ReviewError("最小再現が現在のclean HEADと同一入力に束縛されていません")


def verify_release_review(root: Path, home: Path, thread_id: str) -> None:
    if not thread_id or not command(["git", "branch", "--show-current"], root).startswith("release/"):
        return
    if THREAD_ID.fullmatch(thread_id) is None:
        raise ReviewError("CodexセッションIDが不正です")
    ledger = home / ".codex" / "hooks" / "task-state" / f"{thread_id}.json"
    review = reviewed_task(ledger, root, thread_id)
    if review.get("state") != "done":
        raise ReviewError("自己レビューが完了していません")
    if review.get("pending_fixes") != []:
        raise ReviewError("修正作業が残っています。未対応が0件になってから自己レビューを確定してください")
    head = command(["git", "rev-parse", "HEAD"], root).strip()
    if review.get("reviewed_head") != head:
        raise ReviewError("現在のHEADがレビュー対象と一致しません")
    if command(["git", "status", "--porcelain=v1", "--untracked-files=normal"], root):
        raise ReviewError("未コミットの変更があります")
    verify_minimum_reproduction(review, root, head)


def main() -> int:
    try:
        verify_release_review(Path.cwd().resolve(), Path.home(), os.environ.get("CODEX_THREAD_ID", ""))
    except ReviewError as error:
        print(f"品質ゲートを起動できません: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
