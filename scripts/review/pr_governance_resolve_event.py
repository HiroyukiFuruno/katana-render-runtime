import json
import os
import re
import subprocess
import sys

repository = os.environ["GITHUB_REPOSITORY"]
event_name = os.environ["EVENT_NAME"]
workflow_run_out_of_scope = False
default_branch = os.environ.get("DEFAULT_BRANCH", "")
if re.fullmatch(r"[A-Za-z0-9._/-]+", default_branch) is None or re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository) is None:
    raise SystemExit("Repository default branch is invalid.")
owner, repository_name = repository.split("/", 1)
repository_url = f"https://api.github.com/repos/{repository}"
def workflow_path_matches(value, expected):
    if value == expected:
        return True
    if not isinstance(value, str) or not value.startswith(expected + "@"):
        return False
    ref = value[len(expected) + 1:]
    return (
        re.fullmatch(r"[A-Za-z0-9._/-]+", ref) is not None
        and ref not in {".", ".."} and not ref.startswith("/")
        and "//" not in ref
        and all(part not in {"", ".", ".."} for part in ref.split("/"))
    )
def source_repository_matches(value, identifier, name, url):
    return (
        isinstance(value, dict)
        and type(value.get("id")) is int and value.get("id") == identifier
        and value.get("name") == name and value.get("url") == url
    )

def default_tip(branch):
    response = subprocess.run(["gh", "api", f"repos/{repository}/git/ref/heads/{branch}"], capture_output=True, text=True, check=False, timeout=20)
    if response.returncode != 0:
        return None
    try:
        payload = json.loads(response.stdout)
    except json.JSONDecodeError:
        return None
    candidate = payload.get("object", {}).get("sha") if isinstance(payload, dict) and isinstance(payload.get("object"), dict) else None
    return candidate if isinstance(candidate, str) and re.fullmatch(r"[0-9a-fA-F]{40}", candidate) else None

def workflow_blob(path, ref):
    response = subprocess.run(["gh", "api", f"repos/{repository}/contents/{path}?ref={ref}"], capture_output=True, text=True, check=False, timeout=20)
    if response.returncode != 0:
        return None
    try:
        payload = json.loads(response.stdout)
    except json.JSONDecodeError:
        return None
    candidate = payload.get("sha") if isinstance(payload, dict) else None
    return candidate if isinstance(candidate, str) and re.fullmatch(r"[0-9a-fA-F]{40}", candidate) else None

def base_reaches_tip(base, tip):
    response = subprocess.run(["gh", "api", f"repos/{repository}/compare/{base}...{tip}"], capture_output=True, text=True, check=False, timeout=20)
    if response.returncode != 0:
        return False
    try:
        comparison = json.loads(response.stdout)
    except json.JSONDecodeError:
        return False
    base_commit = comparison.get("base_commit") if isinstance(comparison, dict) else None
    merge_base = comparison.get("merge_base_commit") if isinstance(comparison, dict) else None
    head_commit = comparison.get("head_commit") if isinstance(comparison, dict) else None
    return (
        isinstance(comparison, dict) and comparison.get("status") in {"identical", "ahead"}
        and isinstance(base_commit, dict) and base_commit.get("sha") == base
        and isinstance(merge_base, dict) and merge_base.get("sha") == base
        and isinstance(head_commit, dict) and head_commit.get("sha") == tip
    )
if event_name == "workflow_run":
    run_id = os.environ.get("WORKFLOW_RUN_ID", "")
    source_attempt = os.environ.get("WORKFLOW_RUN_ATTEMPT", "")
    if re.fullmatch(r"[1-9][0-9]*", run_id) is None or re.fullmatch(r"[1-9][0-9]*", source_attempt) is None or os.environ.get("EVENT_ACTION", "") not in {"requested", "in_progress", "completed"}:
        raise SystemExit("Trusted workflow_run id is invalid.")
    result = subprocess.run(
        ["gh", "api", f"repos/{repository}/actions/runs/{run_id}"],
        capture_output=True, text=True, check=False,
        timeout=20,
    )
    if result.returncode != 0:
        raise SystemExit("Unable to load trusted workflow_run.")
    try:
        run = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise SystemExit("Trusted workflow_run is not JSON.") from error
    expected = {
        "PR governance review sensor": (
            ".github/workflows/pr-governance-review-events.yml",
            {"pull_request", "pull_request_review", "pull_request_review_comment"},
        ),
        "CI": (".github/workflows/test-and-build.yml", {"pull_request"}),
        "release-preflight": (".github/workflows/release-preflight.yml", {"pull_request"}),
    }
    name = run.get("name") if isinstance(run, dict) else None
    pulls = run.get("pull_requests") if isinstance(run, dict) else None
    source = pulls[0] if isinstance(pulls, list) and len(pulls) == 1 and isinstance(pulls[0], dict) else None
    base = source.get("base") if isinstance(source, dict) else None
    head = source.get("head") if isinstance(source, dict) else None
    run_repository = run.get("repository") if isinstance(run, dict) else None
    repository_id = run_repository.get("id") if isinstance(run_repository, dict) else None
    source_base_repo = base.get("repo") if isinstance(base, dict) else None
    missing = object()
    source_head_repo = head.get("repo") if isinstance(head, dict) and "repo" in head else missing
    number = source.get("number") if isinstance(source, dict) else None
    if (
        not isinstance(run, dict) or name not in expected
        or not workflow_path_matches(run.get("path"), expected[name][0]) or run.get("event") not in expected[name][1]
        or run.get("status") not in {"requested", "queued", "waiting", "pending", "in_progress", "completed"}
        or not isinstance(run_repository, dict) or type(repository_id) is not int or repository_id < 1 or run_repository.get("full_name") != repository
        or type(run.get("id")) is not int or run.get("id") != int(run_id) or type(run.get("run_number")) is not int
        or type(run.get("run_attempt")) is not int or run.get("run_attempt") != int(source_attempt)
        or (name == "PR governance review sensor" and run.get("run_attempt") != 1)
        or not isinstance(source, dict) or type(number) is not int or number < 1 or not isinstance(base, dict) or not isinstance(head, dict)
        or not isinstance(base.get("sha"), str) or not isinstance(head.get("sha"), str) or base.get("ref") != default_branch
        or re.fullmatch(r"[0-9a-fA-F]{40}", base["sha"]) is None or re.fullmatch(r"[0-9a-fA-F]{40}", head["sha"]) is None
        or run.get("head_sha") != head["sha"] or not source_repository_matches(source_base_repo, repository_id, repository_name, repository_url)
        or source_head_repo is missing
    ):
        raise SystemExit("workflow_run is not a trusted governance source.")
    current = subprocess.run(["gh", "api", f"repos/{repository}/pulls/{number}"], capture_output=True, text=True, check=False, timeout=20)
    if current.returncode != 0:
        raise SystemExit("Unable to bind workflow_run current pull request.")
    try:
        pull = json.loads(current.stdout)
    except json.JSONDecodeError as error:
        raise SystemExit("workflow_run current pull request is not JSON.") from error
    pull_base = pull.get("base") if isinstance(pull, dict) else None
    pull_head = pull.get("head") if isinstance(pull, dict) else None
    pull_base_repo = pull_base.get("repo") if isinstance(pull_base, dict) else None
    pull_head_repo = pull_head.get("repo") if isinstance(pull_head, dict) and "repo" in pull_head else missing
    if (
        not isinstance(pull, dict) or type(pull.get("number")) is not int or pull.get("number") != number or pull.get("state") != "open"
        or not isinstance(pull_base, dict) or not isinstance(pull_head, dict)
        or not isinstance(pull_base_repo, dict) or pull_base_repo.get("full_name") != repository
        or type(pull_base_repo.get("id")) is not int or pull_base_repo.get("id") != repository_id
        or pull_base.get("ref") != default_branch or not isinstance(pull_base.get("sha"), str)
        or re.fullmatch(r"[0-9a-fA-F]{40}", pull_base["sha"]) is None
        or pull_head.get("sha") != head["sha"] or pull_head_repo is missing
    ):
        raise SystemExit("workflow_run current pull request drifted.")
    if source_head_repo is None:
        if pull_head_repo is not None:
            raise SystemExit("workflow_run source head repository drifted.")
        workflow_run_out_of_scope = True
    elif not isinstance(source_head_repo, dict) or not isinstance(pull_head_repo, dict):
        raise SystemExit("workflow_run source head is invalid.")
    else:
        head_name = pull_head_repo.get("full_name")
        if (
            not isinstance(head_name, str) or re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", head_name) is None
            or type(pull_head_repo.get("id")) is not int or pull_head_repo.get("id") < 1
            or not source_repository_matches(source_head_repo, pull_head_repo["id"], head_name.rsplit("/", 1)[1], f"https://api.github.com/repos/{head_name}")
        ):
            raise SystemExit("workflow_run source head repository drifted.")
        if head_name != repository:
            workflow_run_out_of_scope = True
    # PRs may edit a workflow file. The run is a sensor only when
    # its source base, head, and the current default tip all contain
    # the same trusted workflow blob. A normal default-branch advance
    # remains valid only when that source base reaches the current tip.
    if not workflow_run_out_of_scope:
        tip = default_tip(default_branch)
        base_digest = workflow_blob(expected[name][0], base["sha"])
        head_digest = workflow_blob(expected[name][0], head["sha"])
        tip_digest = workflow_blob(expected[name][0], tip) if tip is not None else None
        ancestry_valid = base_reaches_tip(base["sha"], tip) if tip is not None else False
        final_repository = subprocess.run(["gh", "api", f"repos/{repository}"], capture_output=True, text=True, check=False, timeout=20)
        final_tip = default_tip(default_branch)
        try:
            final_repo = json.loads(final_repository.stdout) if final_repository.returncode == 0 else None
        except json.JSONDecodeError:
            final_repo = None
        if (
            tip is None or pull_base.get("sha") != tip
            or not ancestry_valid
            or base_digest is None or head_digest is None or tip_digest is None
            or base_digest != head_digest or base_digest != tip_digest
            or not isinstance(final_repo, dict) or final_repo.get("default_branch") != default_branch
            or final_tip != tip
        ):
            raise SystemExit("workflow_run current default source drifted.")
def pull_page(page_number):
    arguments = ["gh", "api", f"repos/{repository}/pulls?state=open&per_page=100&page={page_number}"]
    if page_number == 6:
        arguments.insert(2, "--include")
    try:
        result = subprocess.run(arguments, capture_output=True, text=True, check=False, timeout=20)
    except subprocess.TimeoutExpired as error:
        raise SystemExit("Open pull request response timed out.") from error
    if result.returncode != 0:
        raise SystemExit("Unable to enumerate open pull requests.")
    try:
        raw = result.stdout
        if page_number == 6:
            headers, separator, raw = raw.replace("\r\n", "\n").partition("\n\n")
            if not separator or not headers.startswith("HTTP/") or re.search(r'(?im)^link:.*rel="next"', headers):
                raise SystemExit("Open pull request response exceeds the fixed page window.")
        page = json.loads(raw)
    except json.JSONDecodeError as error:
        raise SystemExit("Open pull request response is not JSON.") from error
    if not isinstance(page, list) or len(page) > 100 or not all(isinstance(pull, dict) for pull in page):
        raise SystemExit("Open pull request response is invalid.")
    return page
pages = [pull_page(1)]
for page_number in range(2, 7):
    if len(pages[-1]) < 100:
        break
    pages.append(pull_page(page_number))
if pull_page(1) != pages[0]:
    raise SystemExit("Open pull request response first page changed.")
# Validate all API records before narrowing to local/default-base
# governance targets.  Foreign PRs cannot claim local Issues, but a
# duplicate foreign record is still an invalid paginated response.
seen_all: set[int] = set()
numbers: set[int] = set()
bodies: dict[int, str] = {}
for page in pages:
    for pull in page:
        number = pull.get("number") if isinstance(pull, dict) else None
        if (
            type(number) is not int
            or number < 1
            or number in seen_all
            or pull.get("state") != "open"
        ):
            raise SystemExit("Open pull request response is invalid.")
        seen_all.add(number)
        body = pull.get("body")
        if body is not None and not isinstance(body, str):
            raise SystemExit("Open pull request body is invalid.")
        base = pull.get("base")
        head = pull.get("head")
        base_repository = base.get("repo") if isinstance(base, dict) else None
        head_repository = head.get("repo") if isinstance(head, dict) and "repo" in head else None
        if not isinstance(base_repository, dict):
            raise SystemExit("Open pull request repository binding is invalid.")
        if not isinstance(head, dict) or "repo" not in head:
            raise SystemExit("Open pull request repository binding is invalid.")
        # A deleted/unavailable fork is represented by a null head
        # repository. It cannot be a local governance claimant.
        if head_repository is None:
            continue
        if not isinstance(head_repository, dict):
            raise SystemExit("Open pull request repository binding is invalid.")
        if (
            base_repository.get("full_name") != repository
            or head_repository.get("full_name") != repository
            or base.get("ref") != default_branch
        ):
            continue
        numbers.add(number)
        bodies[number] = body or ""
owner, repository_name = repository.split("/", 1)
issue_url_terminator = r"(?=$|[\s)\]}>.,!?;:'\"])"
closing = re.compile(
    r"\b(?:close(?:s|d)?|fix(?:es|ed)?|resolve(?:s|d)?)\b"
    r"(?:[ \t]*:[ \t]*|[ \t]+)"
    r"(?:#([1-9][0-9]*)\b|https://github\.com/"
    + re.escape(owner)
    + "/"
    + re.escape(repository_name)
    + r"/issues/([1-9][0-9]*)"
    + issue_url_terminator
    + r")",
    re.I,
)
def closing_numbers(body: str) -> set[str]:
    return {
        number
        for match in closing.finditer(body)
        for number in match.groups()
        if number is not None
    }
def referenced(issue: int) -> set[int]:
    text = str(issue)
    return {number for number, body in bodies.items() if text in closing_numbers(body)}
source_number_value: int | None = None
priority_event = False
if event_name in {"schedule", "workflow_dispatch"}:
    affected = numbers
elif event_name == "workflow_run":
    source_number = source.get("number")
    source_number_value = source_number
    affected = set() if workflow_run_out_of_scope else ({source_number} if source_number in numbers else set())
    if not workflow_run_out_of_scope:
        source_issues = closing_numbers(bodies.get(source_number, ""))
        for source_issue in source_issues:
            affected |= referenced(int(source_issue))
elif event_name == "pull_request_target":
    source_number = os.environ.get("PR_NUMBER", "")
    source_head = os.environ.get("PR_HEAD_SHA", "")
    source_base = os.environ.get("PR_BASE_SHA", "")
    if re.fullmatch(r"[1-9][0-9]*", source_number) is None or re.fullmatch(r"[0-9a-fA-F]{40}", source_head) is None or re.fullmatch(r"[0-9a-fA-F]{40}", source_base) is None:
        raise SystemExit("pull_request_target source is invalid.")
    source_number_value = int(source_number)
    source_payload = subprocess.run(["gh", "api", f"repos/{repository}/pulls/{source_number_value}"], capture_output=True, text=True, check=False, timeout=20)
    if source_payload.returncode != 0:
        raise SystemExit("Unable to bind pull_request_target source.")
    try: current_source = json.loads(source_payload.stdout)
    except json.JSONDecodeError as error: raise SystemExit("pull_request_target source is not JSON.") from error
    current_base = current_source.get("base") if isinstance(current_source, dict) else None
    current_head = current_source.get("head") if isinstance(current_source, dict) else None
    current_base_repository = current_base.get("repo") if isinstance(current_base, dict) else None
    current_head_repository = current_head.get("repo") if isinstance(current_head, dict) else None
    expected_state = "closed" if os.environ.get("PR_ACTION") == "closed" else "open"
    if not isinstance(current_source, dict) or type(current_source.get("number")) is not int or current_source.get("number") != source_number_value or current_source.get("state") != expected_state or not isinstance(current_base, dict) or not isinstance(current_head, dict) or "repo" not in current_head or not isinstance(current_base.get("sha"), str) or re.fullmatch(r"[0-9a-fA-F]{40}", current_base["sha"]) is None or current_head.get("sha") != source_head or not isinstance(current_base_repository, dict) or (current_head_repository is not None and not isinstance(current_head_repository, dict)):
        raise SystemExit("pull_request_target source changed before revalidation.")
    current_base_ref = current_base.get("ref")
    if (
        not isinstance(current_base_ref, str)
        or re.fullmatch(r"[A-Za-z0-9._/-]+", current_base_ref) is None
        or current_base_ref in {".", ".."}
        or current_base_ref.startswith("/")
        or "//" in current_base_ref
        or any(part in {"", ".", ".."} for part in current_base_ref.split("/"))
    ):
        raise SystemExit("pull_request_target source base is invalid.")
    source_is_local = (
        current_base_repository.get("full_name") == repository
        and isinstance(current_head_repository, dict)
        and current_head_repository.get("full_name") == repository
    )
    def foreign_head_binding(value):
        if value is None:
            return ("deleted",)
        if not isinstance(value, dict):
            return None
        full_name = value.get("full_name")
        identifier = value.get("id")
        if (
            not isinstance(full_name, str)
            or re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", full_name) is None
            or full_name == repository
            or type(identifier) is not int
            or identifier < 1
        ):
            return None
        return ("fork", full_name, identifier)
    # GitHub can queue a pull_request_target event at an older base
    # B and expose the same open PR at a later default tip T. A
    # local PR accepts only that monotonic advance. An explicitly
    # identified fork (including a deleted fork) is outside this
    # repository's governance domain, but still requires stable
    # default-tip and final-source reads before it may be a no-op.
    # Ambiguous head metadata, retargets, and every local race stay
    # fail-closed.
    if current_base["sha"] != source_base:
        if (
            os.environ.get("PR_BASE_REF") != default_branch
            or current_base_ref != default_branch
            or current_base_repository.get("full_name") != repository
        ):
            raise SystemExit("pull_request_target source base changed before revalidation.")
        if source_is_local:
            tip = default_tip(default_branch)
            source_blob = workflow_blob(".github/workflows/pr-governance.yml", source_base)
            head_blob = workflow_blob(".github/workflows/pr-governance.yml", source_head)
            tip_blob = workflow_blob(".github/workflows/pr-governance.yml", tip) if tip is not None else None
            ancestry_valid = base_reaches_tip(source_base, tip) if tip is not None else False
            final_repository = subprocess.run(["gh", "api", f"repos/{repository}"], capture_output=True, text=True, check=False, timeout=20)
            final_source = subprocess.run(["gh", "api", f"repos/{repository}/pulls/{source_number_value}"], capture_output=True, text=True, check=False, timeout=20)
            final_tip = default_tip(default_branch)
            try:
                final_repo = json.loads(final_repository.stdout) if final_repository.returncode == 0 else None
                final_pull = json.loads(final_source.stdout) if final_source.returncode == 0 else None
            except json.JSONDecodeError:
                final_repo = None
                final_pull = None
            final_base = final_pull.get("base") if isinstance(final_pull, dict) else None
            final_head = final_pull.get("head") if isinstance(final_pull, dict) else None
            final_base_repo = final_base.get("repo") if isinstance(final_base, dict) else None
            final_head_repo = final_head.get("repo") if isinstance(final_head, dict) else None
            if (
                tip is None or current_base["sha"] != tip or not ancestry_valid
                or source_blob is None or head_blob is None or tip_blob is None
                or source_blob != head_blob or source_blob != tip_blob
                or not isinstance(final_repo, dict) or final_repo.get("default_branch") != default_branch
                or final_tip != tip
                or not isinstance(final_pull, dict) or type(final_pull.get("number")) is not int or final_pull.get("number") != source_number_value or final_pull.get("state") != expected_state
                or not isinstance(final_base, dict) or not isinstance(final_head, dict)
                or final_base.get("ref") != default_branch or final_base.get("sha") != tip or final_head.get("sha") != source_head
                or not isinstance(final_base_repo, dict) or final_base_repo.get("full_name") != repository
                or not isinstance(final_head_repo, dict) or final_head_repo.get("full_name") != repository
            ):
                raise SystemExit("pull_request_target current default source drifted.")
        else:
            current_foreign_head = foreign_head_binding(current_head_repository)
            if current_foreign_head is None:
                raise SystemExit("pull_request_target source head is not an explicit fork.")
            tip = default_tip(default_branch)
            final_repository = subprocess.run(["gh", "api", f"repos/{repository}"], capture_output=True, text=True, check=False, timeout=20)
            final_source = subprocess.run(["gh", "api", f"repos/{repository}/pulls/{source_number_value}"], capture_output=True, text=True, check=False, timeout=20)
            final_tip = default_tip(default_branch)
            try:
                final_repo = json.loads(final_repository.stdout) if final_repository.returncode == 0 else None
                final_pull = json.loads(final_source.stdout) if final_source.returncode == 0 else None
            except json.JSONDecodeError:
                final_repo = None
                final_pull = None
            final_base = final_pull.get("base") if isinstance(final_pull, dict) else None
            final_head = final_pull.get("head") if isinstance(final_pull, dict) else None
            final_base_repo = final_base.get("repo") if isinstance(final_base, dict) else None
            final_head_repo = final_head.get("repo") if isinstance(final_head, dict) and "repo" in final_head else object()
            if (
                tip is None or current_base["sha"] != tip
                or not isinstance(final_repo, dict) or final_repo.get("default_branch") != default_branch
                or final_tip != tip
                or not isinstance(final_pull, dict) or type(final_pull.get("number")) is not int or final_pull.get("number") != source_number_value or final_pull.get("state") != expected_state
                or not isinstance(final_base, dict) or not isinstance(final_head, dict)
                or final_base.get("ref") != default_branch or final_base.get("sha") != tip or final_head.get("sha") != source_head
                or not isinstance(final_base_repo, dict) or final_base_repo.get("full_name") != repository
                or foreign_head_binding(final_head_repo) != current_foreign_head
            ):
                raise SystemExit("pull_request_target current fork source drifted.")
    if current_base_ref != default_branch or not source_is_local:
        # A public fork is outside the App's governance domain. A
        # local retarget can, however, remove a former duplicate
        # closer. Revalidate every current governed claimant of the
        # prior default-base contract before excluding its source.
        if (
            os.environ.get("PR_ACTION") == "edited"
            and source_is_local
            and current_base_ref != default_branch
        ):
            previous_base_ref = os.environ.get("PR_PREVIOUS_BASE_REF")
            affected = set()
            # GitHub omits changes.base on ordinary edits to an
            # already non-default PR. That is not a retarget and
            # retains the historical no-op domain boundary.
            if previous_base_ref == "":
                pass
            elif (
                not isinstance(previous_base_ref, str)
                or re.fullmatch(r"[A-Za-z0-9._/-]+", previous_base_ref) is None
                or previous_base_ref in {".", ".."}
                or previous_base_ref.startswith("/")
                or "//" in previous_base_ref
                or any(part in {"", ".", ".."} for part in previous_base_ref.split("/"))
            ):
                raise SystemExit("pull_request_target prior base is invalid.")
            elif previous_base_ref == default_branch:
                # The event provides the old body only when it also
                # changed. Union both forms so a base-only retarget
                # preserves the former body while a combined edit
                # cannot leave either claimant stale.
                for body in (os.environ.get("PR_BODY", ""), os.environ.get("PR_PREVIOUS_BODY", "")):
                    if not isinstance(body, str):
                        raise SystemExit("pull_request_target body is invalid.")
                    for source_issue in closing_numbers(body):
                        affected |= referenced(int(source_issue))
        else:
            affected = set()
    else:
        affected = {source_number_value} if source_number_value in numbers else set()
        for body in (os.environ.get("PR_BODY", ""), os.environ.get("PR_PREVIOUS_BODY", "")):
            if not isinstance(body, str):
                raise SystemExit("pull_request_target body is invalid.")
            for source_issue in closing_numbers(body):
                affected |= referenced(int(source_issue))
    # Every PR mutation can change the Issue/closer contract, so
    # preempt the repository-wide arbiter for the exact affected
    # target set.  CI/release workflow_run traffic is handled below
    # and never receives this priority path.
    priority_event = True
else:
    issue_number = os.environ.get("ISSUE_NUMBER", "")
    if re.fullmatch(r"[1-9][0-9]*", issue_number) is None:
        raise SystemExit("Issue event number is invalid.")
    issue = int(issue_number)
    affected = referenced(issue)
    if event_name == "issue_comment" and os.environ.get("ISSUE_PULL_REQUEST_URL", "") and issue in numbers:
        affected.add(issue)
        source_number_value = issue
    priority_event = True
ordered_targets = (
    ([source_number_value] if source_number_value in affected else [])
    + sorted(affected - ({source_number_value} if source_number_value in affected else set()))
)
# Contract mutations receive the bounded priority writer.  CI and
# release workflow_run events remain normal reconciliations; only
# the review-sensor workflow_run is a priority source.
if event_name == "workflow_run":
    priority_event = (
        run.get("name") == "PR governance review sensor"
        and run.get("event") in {"pull_request", "pull_request_review", "pull_request_review_comment"}
    )
priority_targets = ordered_targets if priority_event else []
with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
    # resolver-failure barrier is armed before this mutable
    # snapshot. Even an event with no current direct target needs
    # the complete reconciliation below to prove it may release
    # that repository-wide required context.
    output.write("reconcile=true\n")
    output.write("event_targets=" + json.dumps(ordered_targets, separators=(",", ":")) + "\n")
    output.write("priority_targets=" + json.dumps(priority_targets, separators=(",", ":")) + "\n")
