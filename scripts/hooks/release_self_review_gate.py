from __future__ import annotations

from collections.abc import Mapping
import os
from pathlib import Path
import re
import sys

from local_review_state import ReviewError, command, strict_json


THREAD_ID = re.compile(r"[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}")
REVIEW_TASK_ID = "pre-pr-self-review"


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
    if review.get("reviewed_head") != command(["git", "rev-parse", "HEAD"], root).strip():
        raise ReviewError("現在のHEADがレビュー対象と一致しません")
    if command(["git", "status", "--porcelain=v1", "--untracked-files=normal"], root):
        raise ReviewError("未コミットの変更があります")


def main() -> int:
    try:
        verify_release_review(Path.cwd().resolve(), Path.home(), os.environ.get("CODEX_THREAD_ID", ""))
    except ReviewError as error:
        print(f"品質ゲートを起動できません: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
