"""Activate the App-bound context that blocks stale governance evidence."""

import copy
import json
import os
import re
import subprocess


repository = os.environ.get("GITHUB_REPOSITORY", "")
branch = os.environ.get("DEFAULT_BRANCH", "")
configured_app = os.environ.get("CHECK_APP_ID", "")
context = "KRR / PR governance affected-head barrier"
app_id = 4_766_933
token = os.environ.get("ADMIN_TOKEN", "")
if (
    re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository) is None
    or configured_app != str(app_id)
    or re.fullmatch(r"[A-Za-z0-9._/-]+", branch) is None
    or not token
):
    raise SystemExit("Resolver-failure barrier identity is invalid.")

admin_env = {"GH_TOKEN": token, "PATH": os.environ["PATH"]}
protection_endpoint = f"repos/{repository}/branches/{branch}/protection"
status_checks_endpoint = protection_endpoint + "/required_status_checks"
status_checks_url = f"https://api.github.com/{status_checks_endpoint}"
contexts_url = status_checks_url + "/contexts"


def request(arguments: list[str]) -> dict[str, object]:
    result = subprocess.run(
        ["gh", "api", "--hostname", "github.com", *arguments],
        capture_output=True,
        text=True,
        check=False,
        env=admin_env,
        timeout=20,
    )
    if result.returncode != 0:
        raise SystemExit("Resolver-failure barrier API request failed.")
    try:
        value = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise SystemExit("Resolver-failure barrier API response is not JSON.") from error
    if not isinstance(value, dict):
        raise SystemExit("Resolver-failure barrier API response is invalid.")
    return value


def canonical(value: object) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def records(protection: dict[str, object]) -> tuple[dict[str, object], list[tuple[str, int | None]]]:
    required = protection.get("required_status_checks")
    checks = required.get("checks") if isinstance(required, dict) else None
    contexts = required.get("contexts") if isinstance(required, dict) else None
    strict = required.get("strict") if isinstance(required, dict) else None
    if (
        not isinstance(required, dict)
        or set(required) != {"url", "contexts_url", "strict", "contexts", "checks"}
        or required["url"] != status_checks_url
        or required["contexts_url"] != contexts_url
        or not isinstance(checks, list)
        or not isinstance(contexts, list)
        or type(strict) is not bool
    ):
        raise SystemExit("Resolver-failure barrier branch protection is invalid.")
    result = []
    for item in checks:
        if (
            not isinstance(item, dict)
            or set(item) != {"context", "app_id"}
            or not isinstance(item["context"], str)
            or not item["context"]
            or "\x00" in item["context"]
            or (item["app_id"] is not None and type(item["app_id"]) is not int)
        ):
            raise SystemExit("Resolver-failure barrier branch protection is invalid.")
        result.append((item["context"], item["app_id"]))
    if len(set(result)) != len(result) or len({name for name, _ in result}) != len(result) or [name for name, _ in result] != contexts:
        raise SystemExit("Resolver-failure barrier branch protection is invalid.")
    return required, result


before = request([protection_endpoint])
_before_required, before_records = records(before)
matches = [record for record in before_records if record[0] == context]
if matches == [(context, app_id)]:
    raise SystemExit(0)
if matches:
    raise SystemExit("Resolver-failure barrier has an unexpected App source.")

expected = copy.deepcopy(before)
expected_required = expected["required_status_checks"]
assert isinstance(expected_required, dict)
expected_required["checks"] = [
    {"context": context, "app_id": app_id} if name == context else {"context": name, "app_id": owner}
    for name, owner in before_records
]
expected_required["checks"].append({"context": context, "app_id": app_id})
expected_required["contexts"] = [*expected_required["contexts"], context]
_expected_required, expected_records = records(expected)
result = subprocess.run(
    ["gh", "api", "--hostname", "github.com", "--method", "PATCH", status_checks_endpoint, "--input", "-"],
    input=json.dumps({"strict": expected_required["strict"], "checks": expected_required["checks"]}, separators=(",", ":")),
    capture_output=True,
    text=True,
    check=False,
    env=admin_env,
    timeout=20,
)
try:
    response = json.loads(result.stdout) if result.returncode == 0 else None
except json.JSONDecodeError:
    response = None
after = request([protection_endpoint])
_after_required, after_records = records(after)
if response is None or canonical(response) != canonical(expected_required) or after_records != expected_records or canonical(after) != canonical(expected):
    raise SystemExit("Resolver-failure barrier atomic activation failed; manual recovery is required.")
