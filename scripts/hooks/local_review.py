#!/usr/bin/env python3
from __future__ import annotations

import argparse
from datetime import datetime, timedelta, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import tempfile
import threading

from local_review_contract import MODEL, PROMPT, REASONING, receipt_payload, validate_receipt, validate_review
from local_review_lock import review_lock
from local_review_state import ReviewError, cache_path, canonical, command, digest, gate_configuration, issue_context
from local_review_state import repository_root, requirements_context, resolve_remote_name, source_snapshot, strict_json
from verify_push_issue import issue_numbers as referenced_issue_numbers


SCHEMA = Path(__file__).with_suffix(".schema.json")
REVIEW_TIMEOUT_SECONDS = 1800
REVIEW_ANALYSIS_SECONDS = 1500
REVIEW_POLICY = """
レビュー時間はruntimeの性能基準とは別です。runtime 60秒/close 5秒等の既存基準を変更しません。
最初にchangesと要求を6観点へ対応付け、変更ファイルを関心事ごとに一度読み、必要な直接依存だけ調べてください。
同じ内容のworking/staged diffや以前に読んだソースの全文を重ねて表示せず、相違部分・引用行に絞ってください。
過去レビューの再利用は同じ内容と要求の対応を照合してから行い、旧PASSを今回のinputへ転記しないでください。
原本測定のwhole-source hashが違う場合は同一と扱わず、対象実装のhash一致と変更範囲を調べ、
提案修正の効果の根拠と正式公開版の後工程を区別してください。要求に必要な正式再測定はpendingへ残します。
analysis_deadline_utcまでに必要な観点を調査し、その後は新たな探索を開始せず構造化結論を完成してください。
未確認・不一致を発見したら具体的証拠を保存し、解決に必要な直接箇所だけ調べてください。
期限までに全観点を確認できなければblocked/FAILです。探索を延々続けたり、未確認をPASSにしてはいけません。
残りの時間は引用・input_sha256・全6観点・各Issue・findings・pendingのschema確認に使ってください。
"""


def review_prompt() -> str:
    return (PROMPT + REVIEW_POLICY + f"\nreview process budget: {REVIEW_TIMEOUT_SECONDS}s; "
            f"analysis budget: {REVIEW_ANALYSIS_SECONDS}s.\n")


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Review exact local changes before the full quality gate.")
    parser.add_argument("--issue", type=int, action="append", default=[])
    parser.add_argument("--base")
    parser.add_argument("--remote", default=os.environ.get("REVIEW_REMOTE"))
    parser.add_argument("--remote-url", default=os.environ.get("REVIEW_REMOTE_URL"))
    parser.add_argument("--requirements")
    parser.add_argument("--receipt", default="tmp/local-review/receipt.json")
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check-receipt", action="store_true")
    mode.add_argument("--print-input", action="store_true")
    return parser.parse_args()


def issue_numbers(root: Path, args: argparse.Namespace, receipt: Path) -> list[int]:
    numbers = args.issue
    configured = os.environ.get("REVIEW_ISSUE", "")
    if not numbers and configured:
        if not re.fullmatch(r"[1-9][0-9]*(?:[ ,]+[1-9][0-9]*)*", configured):
            raise ReviewError("REVIEW_ISSUE must contain positive Issue numbers")
        numbers = [int(number) for number in re.split(r"[ ,]+", configured)]
    if not numbers:
        messages = command(["git", "log", "--format=%B", f"{args.base}..HEAD", "--"], root)
        references = referenced_issue_numbers(messages, "HiroyukiFuruno/katana-render-runtime")
        if len(references) > 1:
            receipt_numbers: set[int] | None = None
            if receipt.exists():
                try:
                    stored = strict_json(receipt.read_text())
                    read_receipt(receipt, stored["inputs"], root)
                    stored_issues = stored["inputs"]["issues"]
                    if isinstance(stored_issues, list):
                        receipt_numbers = {issue["number"] for issue in stored_issues}
                except (OSError, ReviewError, KeyError, TypeError):
                    receipt_numbers = None
            if receipt_numbers != references:
                raise ReviewError("multiple branch Issues; set REVIEW_ISSUE explicitly")
        numbers = sorted(references)
    if not numbers:
        retained = retained_inputs(receipt, root=root)
        if retained is not None:
            numbers = [issue["number"] for issue in retained["issues"]]
    if not numbers and receipt.exists():
        stored = strict_json(receipt.read_text())
        try:
            read_receipt(receipt, stored["inputs"], root)
            if retained_context_matches(receipt, stored["inputs"], root):
                numbers = [issue["number"] for issue in stored["inputs"]["issues"]]
        except (KeyError, TypeError) as error:
            raise ReviewError("receipt Issue identity is invalid") from error
    if not numbers or any(type(number) is not int or number < 1 for number in numbers):
        raise ReviewError("set REVIEW_ISSUE or pass --issue for the current requirements")
    return sorted(set(numbers))


def build_inputs(root: Path, args: argparse.Namespace, numbers: list[int]) -> dict:
    remote = getattr(args, "remote", None) or os.environ.get("REVIEW_REMOTE") or "origin"
    remote_name = resolve_remote_name(root, remote)
    base = getattr(args, "base", None) or os.environ.get("REVIEW_BASE") or f"{remote_name}/master"
    remote_url = getattr(args, "remote_url", None) or os.environ.get("REVIEW_REMOTE_URL")
    selected_url = remote_url or (remote if remote.startswith(("git@", "ssh://", "https://", "http://")) else None)
    return {"schema": 1, "repository": "HiroyukiFuruno/katana-render-runtime",
            "review_remote": remote_name,
            "review_remote_url_sha256": hashlib.sha256(selected_url.encode()).hexdigest() if selected_url else None,
            "source": source_snapshot(root, base),
            "issues": issue_context(root, numbers, remote, selected_url),
            "requirements": requirements_context(root, args.requirements),
            "gate_configuration": gate_configuration(root),
            "model": MODEL, "reasoning": REASONING,
            "schema_sha256": hashlib.sha256(SCHEMA.read_bytes()).hexdigest(),
            "prompt_sha256": hashlib.sha256(review_prompt().encode()).hexdigest()}


def branch_identity(root: Path) -> str | None:
    try:
        branch = command(["git", "symbolic-ref", "--quiet", "--short", "HEAD"], root).strip()
    except ReviewError:
        return None
    return branch or None


def retained_context_payload(root: Path, inputs: dict, reviewed_head_sha: str) -> dict:
    context = {
        "schema": 1,
        "branch": branch_identity(root) or "",
        "input_sha256": digest(inputs),
        "reviewed_head_sha": reviewed_head_sha,
    }
    return {**context, "context_sha256": digest(context)}


def retained_context_matches(receipt: Path, stored: dict, root: Path | None) -> bool:
    context_path = receipt.parent / "last-input-context.json"
    if root is None or not context_path.is_file():
        return False
    context = strict_json(context_path.read_text())
    current_branch = branch_identity(root)
    if not isinstance(context, dict) or set(context) != {
        "schema", "branch", "input_sha256", "reviewed_head_sha", "context_sha256"
    }:
        return False
    reviewed_head_sha = context.get("reviewed_head_sha")
    context_body = {key: context[key] for key in (
        "schema", "branch", "input_sha256", "reviewed_head_sha"
    )}
    if (
        context.get("schema") != 1
        or not isinstance(context.get("branch"), str)
        or not context["branch"]
        or context.get("input_sha256") != digest(stored)
        or current_branch != context["branch"]
        or not isinstance(reviewed_head_sha, str)
        or re.fullmatch(r"[0-9a-f]{40}", reviewed_head_sha) is None
        or context.get("context_sha256") != digest(context_body)
    ):
        return False
    try:
        command(["git", "merge-base", "--is-ancestor", reviewed_head_sha, "HEAD"], root)
    except ReviewError:
        return False
    return True


def retained_inputs(receipt: Path, numbers: list[int] | None = None, *, root: Path | None = None) -> dict | None:
    path = receipt.parent / "last-input.json"
    if not path.exists():
        return None
    stored = strict_json(path.read_text())
    issues = stored.get("issues") if isinstance(stored, dict) else None
    if not isinstance(issues, list) or not issues:
        raise ReviewError("retained input has no Issue identity")
    if any(not isinstance(issue, dict) or type(issue.get("number")) is not int
           or issue["number"] < 1 for issue in issues):
        raise ReviewError("retained input has invalid Issue identities")
    if numbers is not None and sorted(issue["number"] for issue in issues) != sorted(numbers):
        return None
    report_path = receipt.parent / "last-review.json"
    if not report_path.is_file():
        # 初回失敗ではCodex起動前にlast-inputだけが残るため、診断原本として保持し、
        # レビュー結果とは扱わず、現在のIssue/requirements指定による再試行を妨げない。
        return None
    report = strict_json(report_path.read_text())
    if not isinstance(report, dict):
        raise ReviewError("retained review is not a structured object")
    if report.get("input_sha256") != digest(stored):
        # 診断原本は保持するが、別入力の結果を現在のレビュー文脈へ流用しない。
        return None
    if not retained_context_matches(receipt, stored, root):
        return None
    if stored.get("model") != MODEL or stored.get("reasoning") != REASONING:
        raise ReviewError("retained input has an unexpected review configuration")
    return stored


def requirements_path(root: Path, args: argparse.Namespace, receipt: Path, numbers: list[int]) -> str | None:
    if args.requirements is not None:
        return args.requirements
    if "REVIEW_REQUIREMENTS" in os.environ:
        configured = os.environ["REVIEW_REQUIREMENTS"]
        if not configured.strip():
            raise ReviewError("REVIEW_REQUIREMENTS must identify a requirements file")
        return configured
    retained = retained_inputs(receipt, numbers, root=root)
    if retained is not None and sorted(issue["number"] for issue in retained["issues"]) == sorted(numbers):
        stored = retained
    elif receipt.exists():
        payload = strict_json(receipt.read_text())
        if not isinstance(payload, dict) or not isinstance(payload.get("inputs"), dict):
            raise ReviewError("stored requirements cannot be recovered from an invalid receipt")
        read_receipt(receipt, payload["inputs"], root)
        if sorted(issue["number"] for issue in payload["inputs"]["issues"]) != sorted(numbers):
            return None
        if not retained_context_matches(receipt, payload["inputs"], root):
            return None
        stored = payload["inputs"]
    else:
        return None
    if sorted(issue["number"] for issue in stored["issues"]) != sorted(numbers):
        return None
    requirements = stored.get("requirements")
    if requirements is None:
        return None
    if not isinstance(requirements, dict) or not isinstance(requirements.get("path"), str):
        raise ReviewError("stored requirements path is invalid")
    path = requirements["path"]
    requirements_context(root, path)
    return path


def read_receipt(path: Path, inputs: dict, root: Path) -> dict:
    receipt = strict_json(path.read_text())
    result = validate_receipt(receipt, inputs)
    reviewed_head = receipt["provenance"]["reviewed_head_sha"]
    try:
        command(["git", "merge-base", "--is-ancestor", reviewed_head, "HEAD"], root)
    except ReviewError as error:
        raise ReviewError(
            "receipt provenance rejected: reviewed HEAD is not verifiably in current HEAD ancestry"
        ) from error
    original = strict_json(path.with_suffix(".review.json").read_text())
    if original != result:
        raise ReviewError("receipt differs from the original structured review")
    return result


def review_command(root: Path, result: Path) -> list[str]:
    return ["rtk", "proxy", "codex", "exec", "--ignore-user-config", "--ephemeral",
            "--sandbox", "read-only", "--model", MODEL,
            "-c", f'model_reasoning_effort="{REASONING}"', "--color", "never",
            "--cd", str(root), "--output-schema", str(SCHEMA),
            "--output-last-message", str(result), "-"]


def compact_input(root: Path, inputs: dict) -> dict:
    base = inputs["source"]["base_sha"]
    changes = {}
    selections = {"head": ["diff", "--no-renames", "--name-only", "-z", base, "HEAD", "--"],
                  "working": ["diff", "--no-renames", "--name-only", "-z", base, "--"],
                  "staged": ["diff", "--cached", "--no-renames", "--name-only", "-z", base, "--"],
                  "untracked": ["ls-files", "--others", "--exclude-standard", "-z"]}
    for key, selection in selections.items():
        changes[key] = sorted(set(command(["git", *selection], root).split("\0")) - {""})
    return {"input_sha256": digest(inputs), "base_sha": base,
            "source_snapshot_sha256": digest(inputs["source"]), "changes": changes,
            "issues": inputs["issues"], "requirements": inputs["requirements"],
            "gate_configuration": inputs["gate_configuration"],
            "model": inputs["model"], "reasoning": inputs["reasoning"]}


def terminate_review_process(process: subprocess.Popen[str]) -> None:
    if os.name == "posix":
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    elif process.poll() is None:
        process.kill()
    process.communicate()


def run_review_process(arguments: list[str], root: Path, environment: dict, prompt: str, diagnostic: Path) -> int:
    process = None
    previous_handlers = {}

    def terminate_parent(signum: int, _frame: object) -> None:
        raise SystemExit(128 + signum)

    manage_signals = os.name == "posix" and threading.current_thread() is threading.main_thread()
    previous_signal_mask = None
    if manage_signals:
        previous_handlers[signal.SIGTERM] = signal.signal(signal.SIGTERM, terminate_parent)
    try:
        with diagnostic.open("w") as errors:
            if manage_signals:
                # 子プロセス生成直後のシグナルを親で受け、必ず子グループの後始末へ進む。
                previous_signal_mask = signal.pthread_sigmask(
                    signal.SIG_BLOCK, {signal.SIGINT, signal.SIGTERM}
                )
            try:
                process = subprocess.Popen(
                    arguments, cwd=root, env=environment, stdin=subprocess.PIPE, text=True,
                    stdout=subprocess.DEVNULL, stderr=errors, start_new_session=True,
                )
            finally:
                if manage_signals and previous_signal_mask is not None:
                    signal.pthread_sigmask(signal.SIG_SETMASK, previous_signal_mask)
                    previous_signal_mask = None
            try:
                process.communicate(prompt, timeout=REVIEW_TIMEOUT_SECONDS)
            except BaseException:
                terminate_review_process(process)
                raise
            return process.returncode
    finally:
        if process is not None and process.returncode is None:
            terminate_review_process(process)
        for signum, handler in previous_handlers.items():
            signal.signal(signum, handler)


def invoke_review(root: Path, directory: Path, inputs: dict) -> dict:
    result_path = directory / "result.json"
    environment = {key: value for key, value in os.environ.items() if not key.startswith("GIT_")}
    environment["KRR_LOCAL_REVIEW_ACTIVE"] = "1"
    payload = compact_input(root, inputs)
    deadline = datetime.now(timezone.utc) + timedelta(seconds=REVIEW_ANALYSIS_SECONDS)
    payload["review_budget"] = {"process_timeout_seconds": REVIEW_TIMEOUT_SECONDS,
                                "analysis_seconds": REVIEW_ANALYSIS_SECONDS,
                                "analysis_deadline_utc": deadline.isoformat()}
    prompt = review_prompt() + "\n固定入力:\n" + canonical(payload).decode()
    diagnostic = directory.parent / "last-codex-stderr.log"
    reviewed_head_sha = command(
        ["git", "rev-parse", "--verify", "--end-of-options", "HEAD^{commit}"], root
    ).strip()
    atomic_json(directory.parent / "last-input.json", inputs)
    atomic_json(
        directory.parent / "last-input-context.json",
        retained_context_payload(root, inputs, reviewed_head_sha),
    )
    try:
        returncode = run_review_process(review_command(root, result_path), root, environment, prompt, diagnostic)
    except subprocess.TimeoutExpired as error:
        raise ReviewError(f"Codex review exceeded {REVIEW_TIMEOUT_SECONDS}s; no receipt accepted; inspect {diagnostic}") from error
    if returncode:
        raise ReviewError(f"Codex review failed (exit {returncode}); inspect {diagnostic}")
    if not result_path.is_file() or result_path.stat().st_size > 1024 * 1024:
        raise ReviewError("Codex did not return a bounded structured review")
    review = strict_json(result_path.read_text())
    retained = directory.parent / "last-review.json"
    atomic_json(retained, review)
    try:
        validate_review(review, inputs)
    except ReviewError as error:
        raise ReviewError(f"{error}; findings retained in {retained}") from error
    return review


def atomic_json(path: Path, value: dict) -> None:
    with tempfile.NamedTemporaryFile(mode="wb", dir=path.parent, delete=False) as output:
        output.write(canonical(value))
        temporary = Path(output.name)
    try:
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def obtain_receipt(root: Path, args: argparse.Namespace, path: Path, inputs: dict, numbers: list[int]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    lock = path.parent / "review.lock"
    with review_lock(lock):
        with tempfile.TemporaryDirectory(prefix="codex-", dir=path.parent) as directory:
            reviewed_head_sha = command(
                ["git", "rev-parse", "--verify", "--end-of-options", "HEAD^{commit}"], root
            ).strip()
            review = invoke_review(root, Path(directory), inputs)
        current_head_sha = command(
            ["git", "rev-parse", "--verify", "--end-of-options", "HEAD^{commit}"], root
        ).strip()
        if current_head_sha != reviewed_head_sha:
            raise ReviewError("HEAD changed during review; receipt was not accepted")
        if build_inputs(root, args, numbers) != inputs:
            raise ReviewError("review input changed during review; receipt was not accepted")
        atomic_json(path.with_suffix(".review.json"), review)
        atomic_json(path, receipt_payload(inputs, review, reviewed_head_sha))
        read_receipt(path, inputs, root)


def run(args: argparse.Namespace) -> int:
    if os.environ.get("KRR_LOCAL_REVIEW_ACTIVE") == "1":
        raise ReviewError("nested local review/check is prohibited")
    if os.environ.get("CI", "").lower() == "true" or os.environ.get("GITHUB_ACTIONS", "").lower() == "true":
        print("Local review SKIP in CI: existing cloud/native quality gates remain required.")
        return 0
    root = repository_root()
    remote = getattr(args, "remote", None) or os.environ.get("REVIEW_REMOTE") or "origin"
    remote_name = resolve_remote_name(root, remote)
    if not getattr(args, "base", None):
        args.base = os.environ.get("REVIEW_BASE") or f"{remote_name}/master"
    path = cache_path(root, args.receipt)
    numbers = issue_numbers(root, args, path)
    args.requirements = requirements_path(root, args, path, numbers)
    inputs = build_inputs(root, args, numbers)
    if args.print_input:
        print(json.dumps({"input_sha256": digest(inputs), "inputs": inputs}, ensure_ascii=True))
        return 0
    try:
        read_receipt(path, inputs, root)
        if build_inputs(root, args, numbers) != inputs:
            raise ReviewError("review input changed during receipt reuse")
        print(f"Local review REUSE {digest(inputs)}; full quality gate remains required.")
        return 0
    except (OSError, ReviewError) as error:
        if args.check_receipt:
            raise ReviewError(f"local review receipt rejected: {error}") from error
        print("Local review receipt missing/invalid/stale; starting read-only High review.")
    obtain_receipt(root, args, path, inputs, numbers)
    print(f"Local review PASS {digest(inputs)}; full quality gate remains required.")
    return 0


def main() -> int:
    try:
        return run(arguments())
    except (OSError, ReviewError, subprocess.SubprocessError) as error:
        print(f"Local review rejected: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
