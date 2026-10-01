import json
import os
import re
import subprocess
import time

repository = os.environ["GITHUB_REPOSITORY"]
repository_valid = re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository) is not None
owner, repository_name = repository.split("/", 1) if repository_valid else ("", "")
repository_url = f"https://api.github.com/repos/{repository}"
event_name = os.environ.get("EVENT_NAME", "")
event_action = os.environ.get("EVENT_ACTION", "")
# Keep the complete serialized dispatcher below GitHub's six-hour
# job ceiling, with ten minutes reserved for terminal start margin.
root_deadline_epoch = int(time.time()) + 21_000
reconcile = True
valid = True
pull_request_target_noop = False
issue_event_noop = False
# Only a fully re-read, local/default source may join the normal
# serialized lane. Every other event keeps the historical preemptive
# behavior until the resolver has established its own target set.
priority = True
sensor_reservation = False
sensor_run_ids = []
cohort_recovery = None

def request(path):
    try:
        result = subprocess.run(
            ["gh", "api", "--hostname", "github.com", path],
            capture_output=True, text=True, check=False,
            timeout=20,
        )
    except subprocess.TimeoutExpired:
        return None
    if result.returncode != 0:
        return None
    try:
        value = json.loads(result.stdout)
    except json.JSONDecodeError:
        return None
    return value if isinstance(value, dict) else None

def request_pages(path):
    if not isinstance(path, str) or "?" not in path:
        return None
    pages = []
    for page_number in range(1, 7):
        arguments = ["gh", "api", "--hostname", "github.com", f"{path}&page={page_number}"]
        if page_number == 6:
            arguments.insert(4, "--include")
        try:
            result = subprocess.run(arguments, capture_output=True, text=True, check=False, timeout=20)
        except subprocess.TimeoutExpired:
            return None
        if result.returncode != 0:
            return None
        raw = result.stdout
        if page_number == 6:
            headers, separator, raw = raw.replace("\r\n", "\n").partition("\n\n")
            if not separator or not headers.startswith("HTTP/") or re.search(r'(?im)^link:.*rel="next"', headers):
                return None
        try:
            value = json.loads(raw)
        except json.JSONDecodeError:
            return None
        if not isinstance(value, list) or len(value) > 100 or not all(isinstance(item, dict) for item in value):
            return None
        pages.append(value)
        if len(value) < 100 or page_number == 6:
            break
    try:
        anchor = subprocess.run(
            ["gh", "api", "--hostname", "github.com", f"{path}&page=1"],
            capture_output=True, text=True, check=False, timeout=20,
        )
    except subprocess.TimeoutExpired:
        return None
    if anchor.returncode != 0:
        return None
    try:
        return pages if json.loads(anchor.stdout) == pages[0] else None
    except json.JSONDecodeError:
        return None

def workflow_path_matches(value, expected):
    if value == expected:
        return True
    if not isinstance(value, str) or not value.startswith(expected + "@"):
        return False
    ref = value[len(expected) + 1:]
    return (
        re.fullmatch(r"[A-Za-z0-9._/-]+", ref) is not None
        and ref not in {".", ".."} and not ref.startswith("/")
        and "//" not in ref and all(part not in {"", ".", ".."} for part in ref.split("/"))
    )

def source_repository_matches(value, identifier, name, url):
    return (
        isinstance(value, dict)
        and type(value.get("id")) is int and value.get("id") == identifier
        and value.get("name") == name and value.get("url") == url
    )

def default_tip(branch):
    reference = request(f"repos/{repository}/git/ref/heads/{branch}")
    candidate = reference.get("object", {}).get("sha") if isinstance(reference, dict) and isinstance(reference.get("object"), dict) else None
    return candidate if isinstance(candidate, str) and re.fullmatch(r"[0-9a-fA-F]{40}", candidate) else None

def workflow_blob(path, ref):
    payload = request(f"repos/{repository}/contents/{path}?ref={ref}")
    candidate = payload.get("sha") if isinstance(payload, dict) else None
    return candidate if isinstance(candidate, str) and re.fullmatch(r"[0-9a-fA-F]{40}", candidate) else None

def base_reaches_tip(base, tip):
    comparison = request(f"repos/{repository}/compare/{base}...{tip}")
    base_commit = comparison.get("base_commit") if isinstance(comparison, dict) else None
    merge_base = comparison.get("merge_base_commit") if isinstance(comparison, dict) else None
    head_commit = comparison.get("head_commit") if isinstance(comparison, dict) else None
    return (
        isinstance(comparison, dict) and comparison.get("status") in {"identical", "ahead"}
        and isinstance(base_commit, dict) and base_commit.get("sha") == base
        and isinstance(merge_base, dict) and merge_base.get("sha") == base
        and isinstance(head_commit, dict) and head_commit.get("sha") == tip
    )

def ref_valid(value):
    return (
        isinstance(value, str)
        and re.fullmatch(r"[A-Za-z0-9._/-]+", value) is not None
        and value not in {".", ".."}
        and not value.startswith("/")
        and "//" not in value
        and all(part not in {"", ".", ".."} for part in value.split("/"))
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

def repository_identity(value):
    identifier = value.get("id") if isinstance(value, dict) else None
    name = value.get("full_name") if isinstance(value, dict) else None
    default = value.get("default_branch") if isinstance(value, dict) else None
    if (
        type(identifier) is not int or identifier < 1
        or not isinstance(name, str) or name != repository
        or not isinstance(default, str) or not ref_valid(default)
    ):
        return None
    return identifier, name, default

def governed_snapshot(pages, issue, repository_id, default_branch):
    owner, repository_name = repository.split("/", 1)
    terminator = r"(?=$|[\s)\]}>.,!?;:'\"])"
    closing = re.compile(
        r"\b(?:close(?:s|d)?|fix(?:es|ed)?|resolve(?:s|d)?)\b"
        r"(?:[ \t]*:[ \t]*|[ \t]+)"
        r"(?:#([1-9][0-9]*)\b|https://github\.com/"
        + re.escape(owner) + "/" + re.escape(repository_name)
        + r"/issues/([1-9][0-9]*)" + terminator + r")",
        re.I,
    )
    seen = set()
    claimants = set()
    snapshots = []
    missing = object()
    for page in pages:
        for pull in page:
            number = pull.get("number") if isinstance(pull, dict) else None
            body = pull.get("body") if isinstance(pull, dict) else None
            draft = pull.get("draft") if isinstance(pull, dict) else None
            base = pull.get("base") if isinstance(pull, dict) else None
            head = pull.get("head") if isinstance(pull, dict) else None
            base_repository = base.get("repo") if isinstance(base, dict) else None
            head_repository = head.get("repo") if isinstance(head, dict) and "repo" in head else missing
            base_identifier = base_repository.get("id") if isinstance(base_repository, dict) else None
            base_name = base_repository.get("full_name") if isinstance(base_repository, dict) else None
            base_sha = base.get("sha") if isinstance(base, dict) else None
            head_sha = head.get("sha") if isinstance(head, dict) else None
            if (
                type(number) is not int or number < 1 or number in seen
                or pull.get("state") != "open" or (body is not None and not isinstance(body, str)) or type(draft) is not bool
                or not isinstance(base, dict) or not isinstance(head, dict)
                or not isinstance(base_repository, dict) or head_repository is missing
                or type(base_identifier) is not int or base_identifier < 1
                or not isinstance(base_name, str) or re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", base_name) is None
                or not ref_valid(base.get("ref"))
                or not isinstance(base_sha, str) or re.fullmatch(r"[0-9a-fA-F]{40}", base_sha) is None
                or not isinstance(head_sha, str) or re.fullmatch(r"[0-9a-fA-F]{40}", head_sha) is None
                or base_identifier != repository_id or base_name != repository
            ):
                return None
            seen.add(number)
            if head_repository is None:
                head_identity = None
            else:
                head_identifier = head_repository.get("id") if isinstance(head_repository, dict) else None
                head_name = head_repository.get("full_name") if isinstance(head_repository, dict) else None
                if (
                    not isinstance(head_repository, dict)
                    or type(head_identifier) is not int or head_identifier < 1
                    or not isinstance(head_name, str)
                    or re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", head_name) is None
                ):
                    return None
                head_identity = (head_identifier, head_name)
            if head_identity is not None and ((head_identity[0] == repository_id) != (head_identity[1] == repository)):
                return None
            snapshots.append((number, pull["state"], body, draft, base["ref"], base_sha.lower(), base_identifier, base_name, head_sha.lower(), head_identity))
            if (
                head_identity != (repository_id, repository)
                or base.get("ref") != default_branch
            ):
                continue
            matches = {
                candidate
                for match in closing.finditer(body or "")
                for candidate in match.groups()
                if candidate is not None
            }
            if str(issue) in matches:
                claimants.add(number)
    return tuple(sorted(snapshots)), frozenset(claimants)

def pull_identity(value, repository_id):
    number = value.get("number") if isinstance(value, dict) else None
    state = value.get("state") if isinstance(value, dict) else None
    base = value.get("base") if isinstance(value, dict) else None
    head = value.get("head") if isinstance(value, dict) else None
    base_repository = base.get("repo") if isinstance(base, dict) else None
    head_repository = head.get("repo") if isinstance(head, dict) and "repo" in head else object()
    base_identifier = base_repository.get("id") if isinstance(base_repository, dict) else None
    base_name = base_repository.get("full_name") if isinstance(base_repository, dict) else None
    base_ref = base.get("ref") if isinstance(base, dict) else None
    base_sha = base.get("sha") if isinstance(base, dict) else None
    head_sha = head.get("sha") if isinstance(head, dict) else None
    if (
        type(number) is not int or number < 1 or not isinstance(state, str) or state not in {"open", "closed"}
        or not isinstance(base, dict) or not isinstance(head, dict)
        or type(base_identifier) is not int or base_identifier != repository_id
        or base_name != repository or not ref_valid(base_ref)
        or not isinstance(base_sha, str) or re.fullmatch(r"[0-9a-fA-F]{40}", base_sha) is None
        or not isinstance(head_sha, str) or re.fullmatch(r"[0-9a-fA-F]{40}", head_sha) is None
    ):
        return None
    if head_repository is None:
        head_identity = None
    elif isinstance(head_repository, dict):
        head_identifier = head_repository.get("id")
        head_name = head_repository.get("full_name")
        if (
            type(head_identifier) is not int or head_identifier < 1
            or not isinstance(head_name, str)
            or re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", head_name) is None
            or ((head_identifier == repository_id) != (head_name == repository))
        ):
            return None
        head_identity = (head_identifier, head_name)
    else:
        return None
    return number, state, base_ref, base_sha.lower(), head_sha.lower(), head_identity

if event_name == "pull_request_target":
    source_number = os.environ.get("PR_NUMBER", "")
    source_head = os.environ.get("PR_HEAD_SHA", "")
    source_base = os.environ.get("PR_BASE_SHA", "")
    source_base_ref = os.environ.get("PR_BASE_REF", "")
    expected_state = "closed" if os.environ.get("PR_ACTION") == "closed" else "open"
    if (
        not repository_valid
        or os.environ.get("PR_ACTION") not in {"opened", "edited", "synchronize", "reopened", "ready_for_review", "converted_to_draft", "closed"}
        or re.fullmatch(r"[1-9][0-9]*", source_number) is None
        or re.fullmatch(r"[0-9a-fA-F]{40}", source_head) is None
        or re.fullmatch(r"[0-9a-fA-F]{40}", source_base) is None
        or not ref_valid(source_base_ref)
    ):
        valid = False
    else:
        source = request(f"repos/{repository}/pulls/{int(source_number)}")
        current_base = source.get("base") if isinstance(source, dict) else None
        current_head = source.get("head") if isinstance(source, dict) else None
        current_base_repository = current_base.get("repo") if isinstance(current_base, dict) else None
        current_head_repository = current_head.get("repo") if isinstance(current_head, dict) and "repo" in current_head else object()
        if (
            not isinstance(source, dict)
            or type(source.get("number")) is not int or source.get("number") != int(source_number)
            or source.get("state") != expected_state
            or not isinstance(current_base, dict) or not isinstance(current_head, dict)
            or not isinstance(current_base.get("sha"), str)
            or re.fullmatch(r"[0-9a-fA-F]{40}", current_base["sha"]) is None
            or current_head.get("sha") != source_head
            or not isinstance(current_base_repository, dict)
            or current_base_repository.get("full_name") != repository
            or not ref_valid(current_base.get("ref"))
            or current_base.get("ref") != source_base_ref
            or (current_head_repository is not None and not isinstance(current_head_repository, dict))
        ):
            valid = False
        elif current_base["sha"] == source_base:
            # Only an unchanged base can be excluded here. A queued
            # historical base remains in the resolver, which proves
            # its monotonic/default-branch evolution before any
            # fork event becomes a no-op.
            current_identity = repository_identity(request(f"repos/{repository}"))
            initial_source = pull_identity(
                source, current_identity[0] if current_identity is not None else 0,
            )
            if (
                current_identity is None or initial_source is None
                or initial_source[0] != int(source_number) or initial_source[1] != expected_state
                or initial_source[2] != source_base_ref or initial_source[3] != source_base.lower()
                or initial_source[4] != source_head.lower()
            ):
                valid = False
            elif initial_source[5] != (current_identity[0], repository):
                final_source = request(f"repos/{repository}/pulls/{int(source_number)}")
                final_identity = repository_identity(request(f"repos/{repository}"))
                final_source_identity = pull_identity(final_source, current_identity[0])
                if final_identity != current_identity or final_source_identity != initial_source:
                    valid = False
                else:
                    reconcile = False
            else:
                default_branch = current_identity[2]
                previous_base_ref = os.environ.get("PR_PREVIOUS_BASE_REF", "")
                if previous_base_ref != "" and not ref_valid(previous_base_ref):
                    valid = False
                elif source_base_ref == default_branch or previous_base_ref == default_branch:
                    pass
                else:
                    final_source = request(f"repos/{repository}/pulls/{int(source_number)}")
                    final_identity = repository_identity(request(f"repos/{repository}"))
                    final_source_identity = pull_identity(final_source, current_identity[0])
                    if (
                        final_identity != current_identity
                        or initial_source[5] != (current_identity[0], repository)
                        or final_source_identity != initial_source
                    ):
                        valid = False
                    else:
                        reconcile = False
elif event_name in {"issues", "issue_comment"}:
    issue_number = os.environ.get("ISSUE_NUMBER", "")
    issue_pull_request_url = os.environ.get("ISSUE_PULL_REQUEST_URL", "")
    if event_name == "issue_comment":
        priority = False
    if re.fullmatch(r"[1-9][0-9]*", issue_number) is None or not isinstance(issue_pull_request_url, str):
        valid = False
    elif issue_pull_request_url == "":
        initial_identity = repository_identity(request(f"repos/{repository}"))
        default_branch = initial_identity[2] if initial_identity is not None else None
        initial_pages = request_pages(f"repos/{repository}/pulls?state=open&per_page=100")
        initial_snapshot = (
            governed_snapshot(initial_pages, int(issue_number), initial_identity[0], default_branch)
            if initial_identity is not None and initial_pages is not None else None
        )
        final_pages = request_pages(f"repos/{repository}/pulls?state=open&per_page=100")
        final_identity = repository_identity(request(f"repos/{repository}"))
        final_snapshot = (
            governed_snapshot(final_pages, int(issue_number), initial_identity[0], default_branch)
            if initial_snapshot is not None and final_pages is not None else None
        )
        if initial_snapshot is None or final_identity != initial_identity or final_snapshot is None:
            valid = False
        elif initial_snapshot == final_snapshot and not initial_snapshot[1]:
            reconcile = False
    elif event_name == "issue_comment":
        match = re.fullmatch(
            re.escape(f"https://api.github.com/repos/{repository}/pulls/") + r"([1-9][0-9]*)",
            issue_pull_request_url,
        )
        if match is None:
            valid = False
        else:
            source_number = int(match.group(1))
            initial_identity = repository_identity(request(f"repos/{repository}"))
            initial_source = pull_identity(
                request(f"repos/{repository}/pulls/{source_number}"),
                initial_identity[0] if initial_identity is not None else 0,
            )
            final_source = pull_identity(
                request(f"repos/{repository}/pulls/{source_number}"),
                initial_identity[0] if initial_identity is not None else 0,
            )
            final_identity = repository_identity(request(f"repos/{repository}"))
            if (
                source_number != int(issue_number)
                or initial_identity is None or final_identity != initial_identity
                or initial_source is None or final_source is None
                or initial_source[0] != source_number or final_source[0] != source_number
            ):
                valid = False
            elif initial_source == final_source and (
                initial_source[1] == "closed"
                or initial_source[2] != initial_identity[2]
                or initial_source[5] != (initial_identity[0], repository)
            ):
                reconcile = False
            elif initial_source == final_source:
                # 同じAPI上の現行PRがopen・local・defaultなら直接priorityへ送る。
                priority = (
                    initial_source[1] == "open"
                    and initial_source[2] == initial_identity[2]
                    and initial_source[5] == (initial_identity[0], repository)
                )
elif event_name == "workflow_dispatch" and os.environ.get("RECOVERY_OWNER_HINT", ""):
    valid = False
    priority = False
    try:
        import base64, hashlib
        identifier = os.environ.get("RECOVERY_OWNER_HINT", "")
        if re.fullmatch(r"[1-9][0-9]*", identifier) is None:
            raise ValueError("Invalid failed owner callback")
        workflow_sha = os.environ["WORKFLOW_SHA"]
        content = request(f"repos/{repository}/contents/scripts/review/pr_governance_cohort.py?ref={workflow_sha}")
        if not isinstance(content, dict) or content.get("type") != "file" or content.get("encoding") != "base64" or not isinstance(content.get("content"), str) or len(content["content"]) > 300000:
            raise ValueError("Untrusted recovery reader")
        data = base64.b64decode(content["content"].replace("\n", ""), validate=True)
        if hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest() != content.get("sha"):
            raise ValueError("Recovery helper blob drift")
        namespace = {"__name__": "trusted_cohort_recovery"}
        exec(compile(data.decode("utf-8", "strict"), "trusted-cohort-recovery", "exec"), namespace)
        workflow = request(f"repos/{repository}/contents/.github/workflows/pr-governance.yml?ref={workflow_sha}")
        if not isinstance(workflow, dict) or not isinstance(workflow.get("sha"), str): raise ValueError("Recovery workflow blob missing")
        budget = namespace["ReadBudget"](300)
        transport = namespace["_Transport"](os.environ["GH_TOKEN"], budget)
        reader = namespace["_Reader"](transport.json, transport.archive, budget, root_deadline_epoch)
        cohort_recovery = namespace["await_failed_owner_recovery"](reader, repository, int(identifier), workflow_sha, workflow["sha"])
        root_deadline_epoch = cohort_recovery["root_deadline_epoch"]
        valid = True
    except (ValueError, TypeError, KeyError, RuntimeError, UnicodeError):
        cohort_recovery = None
elif event_name == "workflow_run":
    run_id = os.environ.get("WORKFLOW_RUN_ID", "")
    source_attempt = os.environ.get("WORKFLOW_RUN_ATTEMPT", "")
    expected = {
        "PR governance review sensor": (
            ".github/workflows/pr-governance-review-events.yml",
            {"pull_request", "pull_request_review", "pull_request_review_comment"},
        ),
        "CI": (".github/workflows/test-and-build.yml", {"pull_request"}),
        "release-preflight": (".github/workflows/release-preflight.yml", {"pull_request"}),
    }
    lifecycle = {"requested", "queued", "waiting", "pending", "in_progress", "completed"}
    if (
        not repository_valid
        or re.fullmatch(r"[1-9][0-9]*", run_id) is None
        or re.fullmatch(r"[1-9][0-9]*", source_attempt) is None
        or event_action not in {"requested", "in_progress", "completed"}
    ):
        valid = False
    else:
        repo = request(f"repos/{repository}")
        default_branch = repo.get("default_branch") if isinstance(repo, dict) else None
        run = request(f"repos/{repository}/actions/runs/{run_id}")
        pulls = run.get("pull_requests") if isinstance(run, dict) else None
        source = pulls[0] if isinstance(pulls, list) and len(pulls) == 1 and isinstance(pulls[0], dict) else None
        number = source.get("number") if isinstance(source, dict) else None
        name = run.get("name") if isinstance(run, dict) else None
        source_base = source.get("base") if isinstance(source, dict) else None
        source_head = source.get("head") if isinstance(source, dict) else None
        source_base_repo = source_base.get("repo") if isinstance(source_base, dict) else None
        missing = object()
        source_head_repo = source_head.get("repo") if isinstance(source_head, dict) and "repo" in source_head else missing
        run_repository = run.get("repository") if isinstance(run, dict) else None
        repository_id = run_repository.get("id") if isinstance(run_repository, dict) else None
        if (
            not isinstance(default_branch, str) or not default_branch
            or not isinstance(run, dict) or name not in expected
            or not workflow_path_matches(run.get("path"), expected[name][0])
            or run.get("event") not in expected[name][1]
            or run.get("status") not in lifecycle
            or type(run.get("id")) is not int or run.get("id") != int(run_id)
            or type(run.get("run_number")) is not int
            or type(run.get("run_attempt")) is not int or run.get("run_attempt") != int(source_attempt)
            or (name == "PR governance review sensor" and run.get("run_attempt") != 1)
            or not isinstance(run_repository, dict) or type(repository_id) is not int or repository_id < 1
            or run_repository.get("full_name") != repository
            or type(number) is not int or number < 1
            or not isinstance(source_base, dict) or not isinstance(source_head, dict)
            or not source_repository_matches(source_base_repo, repository_id, repository_name, repository_url)
            or source_base.get("ref") != default_branch or not isinstance(source_base.get("sha"), str)
            or re.fullmatch(r"[0-9a-fA-F]{40}", source_base["sha"]) is None
            or not isinstance(source_head.get("sha"), str)
            or re.fullmatch(r"[0-9a-fA-F]{40}", source_head["sha"]) is None
            or run.get("head_sha") != source_head["sha"]
            or source_head_repo is missing
        ):
            valid = False
        else:
            pull = request(f"repos/{repository}/pulls/{number}")
            pull_base = pull.get("base") if isinstance(pull, dict) else None
            pull_head = pull.get("head") if isinstance(pull, dict) else None
            pull_base_repo = pull_base.get("repo") if isinstance(pull_base, dict) else None
            pull_head_repo = pull_head.get("repo") if isinstance(pull_head, dict) and "repo" in pull_head else missing
            if (
                not isinstance(pull, dict) or type(pull.get("number")) is not int or pull.get("number") != number or pull.get("state") not in {"open", "closed"}
                or (name == "PR governance review sensor" and type(pull.get("draft")) is not bool)
                or not isinstance(pull_base, dict) or not isinstance(pull_head, dict)
                or not isinstance(pull_base_repo, dict) or pull_base_repo.get("full_name") != repository
                or type(pull_base_repo.get("id")) is not int or pull_base_repo.get("id") != repository_id
                or pull_base.get("ref") != default_branch or not isinstance(pull_base.get("sha"), str)
                or re.fullmatch(r"[0-9a-fA-F]{40}", pull_base["sha"]) is None
                or pull_head.get("sha") != source_head["sha"]
                or pull_head_repo is missing
            ):
                valid = False
            elif pull.get("state") == "closed":
                # A delayed source from a local PR closed after the
                # run started has no open target to reconcile.  It is
                # safe to skip the shared barrier only after both
                # mutable resources have been re-read and retain the
                # exact workflow/PR binding. Foreign and deleted
                # forks are equally outside this repository's
                # governance domain, but their identity must remain
                # stable across both reads.
                closed_head_binding = None
                if source_head_repo is None:
                    if pull_head_repo is None:
                        closed_head_binding = ("deleted",)
                elif isinstance(source_head_repo, dict) and isinstance(pull_head_repo, dict):
                    pull_head_name = pull_head_repo.get("full_name")
                    pull_head_id = pull_head_repo.get("id")
                    if (
                        isinstance(pull_head_name, str)
                        and re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", pull_head_name) is not None
                        and type(pull_head_id) is int and pull_head_id >= 1
                        and source_repository_matches(
                            source_head_repo, pull_head_id, pull_head_name.rsplit("/", 1)[1],
                            f"https://api.github.com/repos/{pull_head_name}",
                        )
                    ):
                        if pull_head_name == repository and pull_head_id == repository_id:
                            closed_head_binding = ("local",)
                        elif pull_head_id != repository_id:
                            closed_head_binding = foreign_head_binding(pull_head_repo)
                if (
                    closed_head_binding is None
                    or pull_base.get("sha") != source_base["sha"]
                ):
                    valid = False
                else:
                    final_run = request(f"repos/{repository}/actions/runs/{run_id}")
                    final_pull = request(f"repos/{repository}/pulls/{number}")
                    final_repo = request(f"repos/{repository}")
                    final_pulls = final_run.get("pull_requests") if isinstance(final_run, dict) else None
                    final_source = final_pulls[0] if isinstance(final_pulls, list) and len(final_pulls) == 1 and isinstance(final_pulls[0], dict) else None
                    final_source_base = final_source.get("base") if isinstance(final_source, dict) else None
                    final_source_head = final_source.get("head") if isinstance(final_source, dict) else None
                    final_source_base_repo = final_source_base.get("repo") if isinstance(final_source_base, dict) else None
                    final_source_head_repo = final_source_head.get("repo") if isinstance(final_source_head, dict) and "repo" in final_source_head else missing
                    final_run_repository = final_run.get("repository") if isinstance(final_run, dict) else None
                    final_pull_base = final_pull.get("base") if isinstance(final_pull, dict) else None
                    final_pull_head = final_pull.get("head") if isinstance(final_pull, dict) else None
                    final_pull_base_repo = final_pull_base.get("repo") if isinstance(final_pull_base, dict) else None
                    final_pull_head_repo = final_pull_head.get("repo") if isinstance(final_pull_head, dict) and "repo" in final_pull_head else missing
                    final_head_binding_valid = False
                    if closed_head_binding == ("local",):
                        final_head_binding_valid = (
                            isinstance(final_source_head_repo, dict)
                            and source_repository_matches(final_source_head_repo, repository_id, repository_name, repository_url)
                            and isinstance(final_pull_head_repo, dict)
                            and final_pull_head_repo.get("full_name") == repository
                            and type(final_pull_head_repo.get("id")) is int and final_pull_head_repo.get("id") == repository_id
                        )
                    elif closed_head_binding == ("deleted",):
                        final_head_binding_valid = final_source_head_repo is None and final_pull_head_repo is None
                    elif len(closed_head_binding) == 3:
                        _, foreign_name, foreign_id = closed_head_binding
                        final_head_binding_valid = (
                            isinstance(final_source_head_repo, dict)
                            and foreign_id != repository_id
                            and source_repository_matches(
                                final_source_head_repo, foreign_id, foreign_name.rsplit("/", 1)[1],
                                f"https://api.github.com/repos/{foreign_name}",
                            )
                            and foreign_head_binding(final_pull_head_repo) == closed_head_binding
                        )
                    if (
                        not isinstance(final_run, dict) or final_run.get("name") != name
                        or not workflow_path_matches(final_run.get("path"), expected[name][0])
                        or final_run.get("path") != run.get("path")
                        or final_run.get("event") not in expected[name][1]
                        or final_run.get("event") != run.get("event")
                        or final_run.get("status") not in lifecycle
                        or type(final_run.get("id")) is not int or final_run.get("id") != int(run_id)
                        or type(final_run.get("run_number")) is not int or final_run.get("run_number") != run.get("run_number")
                        or type(final_run.get("run_attempt")) is not int or final_run.get("run_attempt") != int(source_attempt)
                        or (name == "PR governance review sensor" and final_run.get("run_attempt") != 1)
                        or not isinstance(final_run_repository, dict)
                        or final_run_repository.get("full_name") != repository
                        or type(final_run_repository.get("id")) is not int or final_run_repository.get("id") != repository_id
                        or not isinstance(final_source, dict)
                        or type(final_source.get("number")) is not int or final_source.get("number") != number
                        or not isinstance(final_source_base, dict) or final_source_base.get("ref") != default_branch or final_source_base.get("sha") != source_base["sha"]
                        or not source_repository_matches(final_source_base_repo, repository_id, repository_name, repository_url)
                        or not isinstance(final_source_head, dict) or final_source_head.get("sha") != source_head["sha"]
                        or final_run.get("head_sha") != source_head["sha"]
                        or not isinstance(final_pull, dict)
                        or type(final_pull.get("number")) is not int or final_pull.get("number") != number
                        or final_pull.get("state") != "closed"
                        or not isinstance(final_pull_base, dict) or final_pull_base.get("ref") != default_branch or final_pull_base.get("sha") != source_base["sha"]
                        or not isinstance(final_pull_base_repo, dict) or final_pull_base_repo.get("full_name") != repository
                        or type(final_pull_base_repo.get("id")) is not int or final_pull_base_repo.get("id") != repository_id
                        or not isinstance(final_pull_head, dict) or final_pull_head.get("sha") != source_head["sha"]
                        or not final_head_binding_valid
                        or not isinstance(final_repo, dict) or final_repo.get("default_branch") != default_branch
                    ):
                        valid = False
                    else:
                        reconcile = False
            elif source_head_repo is None:
                if pull_head_repo is not None:
                    valid = False
                else:
                    reconcile = False
            elif not isinstance(source_head_repo, dict) or not isinstance(pull_head_repo, dict):
                valid = False
            else:
                head_name = pull_head_repo.get("full_name")
                if (
                    not isinstance(head_name, str)
                    or re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", head_name) is None
                    or type(pull_head_repo.get("id")) is not int or pull_head_repo.get("id") < 1
                    or not source_repository_matches(
                        source_head_repo, pull_head_repo["id"], head_name.rsplit("/", 1)[1],
                        f"https://api.github.com/repos/{head_name}",
                    )
                ):
                    valid = False
                elif head_name != repository:
                    reconcile = False
                elif pull_head_repo["id"] != repository_id:
                    valid = False
                else:
                    tip = default_tip(default_branch)
                    base_digest = workflow_blob(expected[name][0], source_base["sha"])
                    head_digest = workflow_blob(expected[name][0], source_head["sha"])
                    tip_digest = workflow_blob(expected[name][0], tip) if tip is not None else None
                    ancestry_valid = base_reaches_tip(source_base["sha"], tip) if tip is not None else False
                    final_repo = request(f"repos/{repository}")
                    final_tip = default_tip(default_branch)
                    if (
                        tip is None or pull_base.get("sha") != tip
                        or not ancestry_valid
                        or base_digest is None or head_digest is None or tip_digest is None
                        or base_digest != head_digest or base_digest != tip_digest
                        or not isinstance(final_repo, dict) or final_repo.get("default_branch") != default_branch
                        or final_tip != tip
                    ):
                        valid = False
                    elif name == "PR governance review sensor" and pull["draft"]:
                        # Draft sensor は成功しても latch を待たないため、検証済みの callback が共有 writer を占有しないようにする。
                        reconcile = False
                        priority = False
            if (
                valid and reconcile and name in {"CI", "release-preflight"}
                and run.get("event") == "pull_request"
            ):
                priority = False
pull_request_target_noop = event_name == "pull_request_target" and valid and not reconcile
issue_event_noop = event_name in {"issues", "issue_comment"} and valid and not reconcile


def canonical_repository_binding(value):
    if not isinstance(value, dict) or type(value.get("id")) is not int or value["id"] < 1:
        return None
    full_name = value.get("full_name")
    if "name" in value or "url" in value:
        url = value.get("url")
        match = re.fullmatch(r"https://api\.github\.com/repos/([A-Za-z0-9_.-]+)/([A-Za-z0-9_.-]+)", url) if isinstance(url, str) else None
        if match is None or value.get("name") != match[2]:
            return None
        derived = match[1] + "/" + match[2]
        if "full_name" in value and full_name != derived:
            return None
        full_name = derived
    if not isinstance(full_name, str) or re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", full_name) is None:
        return None
    owner_name = full_name.split("/", 1)[0]
    if "html_url" in value and value["html_url"] != "https://github.com/" + full_name:
        return None
    if "owner" in value:
        owner_value = value["owner"]
        if (not isinstance(owner_value, dict) or owner_value.get("login") != owner_name
            or ("id" in owner_value and (type(owner_value["id"]) is not int or owner_value["id"] < 1))
            or ("url" in owner_value and owner_value["url"] != "https://api.github.com/users/" + owner_name)
            or ("html_url" in owner_value and owner_value["html_url"] != "https://github.com/" + owner_name)):
            return None
    return value["id"], full_name

def active_local_review_sensors() -> list[int] | None:
    """現在のAPIと結合したlocal/default・非Draftのsensorを取得する。"""
    expected_path = ".github/workflows/pr-governance-review-events.yml"
    current_repository = repository_identity(request(f"repos/{repository}"))
    if current_repository is None:
        return None
    local_binding = current_repository[:2]
    default_branch = current_repository[2]
    allowed_events = {"pull_request", "pull_request_review", "pull_request_review_comment"}
    active_sensor_statuses = ("requested", "queued", "waiting", "pending", "in_progress")
    identifiers = set()
    identities = {}
    for status in active_sensor_statuses:
        endpoint = f"repos/{repository}/actions/workflows/pr-governance-review-events.yml/runs?status={status}&per_page=100&page="
        for attempt in range(4):
            listed = request(endpoint + "1")
            total = listed.get("total_count") if isinstance(listed, dict) else None
            if type(total) is not int or not 0 <= total <= 600:
                return None
            runs = []
            inventory_ids = set()
            for page in range(1, max(1, (total + 99) // 100) + 1):
                response = listed if page == 1 else request(endpoint + str(page))
                items = response.get("workflow_runs") if isinstance(response, dict) else None
                if (not isinstance(response, dict) or type(response.get("total_count")) is not int or response["total_count"] != total
                    or not isinstance(items, list) or len(items) != min(100, max(0, total - (page - 1) * 100))):
                    return None
                for item in items:
                    run_id = item.get("id") if isinstance(item, dict) else None
                    if type(run_id) is not int or not 1 <= run_id <= 2**63 - 1 or run_id in inventory_ids:
                        return None
                    inventory_ids.add(run_id)
                runs.extend(items)
            anchor = request(endpoint + "1")
            if anchor is None:
                return None
            try:
                stable = json.dumps(anchor, sort_keys=True, allow_nan=False) == json.dumps(listed, sort_keys=True, allow_nan=False)
            except (ValueError, TypeError):
                return None
            if stable:
                break
        else:
            return None
        page_identifiers = set()
        for run in runs:
            if not isinstance(run, dict):
                return None
            run_id = run.get("id")
            pulls = run.get("pull_requests")
            run_head_binding = canonical_repository_binding(run.get("head_repository"))
            if (type(run_id) is not int or not 1 <= run_id <= 2**63 - 1 or run_id in page_identifiers
                or run.get("name") != "PR governance review sensor" or run.get("event") not in allowed_events
                or run.get("status") not in active_sensor_statuses or run.get("conclusion") is not None or type(run.get("run_attempt")) is not int or run["run_attempt"] != 1
                or not workflow_path_matches(run.get("path"), expected_path)
                or canonical_repository_binding(run.get("repository")) != local_binding
                or run_head_binding is None or not isinstance(pulls, list) or len(pulls) != 1):
                return None
            page_identifiers.add(run_id)
            pull = pulls[0]
            base = pull.get("base") if isinstance(pull, dict) else None
            head = pull.get("head") if isinstance(pull, dict) else None
            number = pull.get("number") if isinstance(pull, dict) else None
            if (type(number) is not int or number < 1 or not isinstance(base, dict) or not isinstance(head, dict)
                or not ref_valid(base.get("ref"))
                or canonical_repository_binding(base.get("repo")) != local_binding
                or canonical_repository_binding(head.get("repo")) != run_head_binding
                or not isinstance(head.get("sha"), str) or re.fullmatch(r"[0-9a-fA-F]{40}", head["sha"]) is None):
                return None
            identity = (
                run_id, run["name"], run["event"], run["run_attempt"], run["path"],
                canonical_repository_binding(run["repository"]), run_head_binding, run.get("head_sha"),
                number, base["ref"], canonical_repository_binding(base["repo"]), head["sha"], canonical_repository_binding(head["repo"]),
            )
            # 状態別APIの間で遷移した同一runは、固定identityが一致する場合だけ統合する。
            if run_id in identities and identities[run_id] != identity:
                return None
            identities[run_id] = identity
            if len(identities) > 600:
                return None
            current = request(f"repos/{repository}/pulls/{number}")
            current_base = current.get("base") if isinstance(current, dict) else None
            current_head = current.get("head") if isinstance(current, dict) else None
            current_head_binding = canonical_repository_binding(current_head.get("repo")) if isinstance(current_head, dict) else None
            if (not isinstance(current, dict) or type(current.get("number")) is not int or current["number"] != number
                or current.get("state") not in {"open", "closed"} or type(current.get("draft")) is not bool
                or not isinstance(current_base, dict) or not ref_valid(current_base.get("ref"))
                or canonical_repository_binding(current_base.get("repo")) != local_binding
                or not isinstance(current_head, dict) or current_head_binding is None
                or not isinstance(current_head.get("sha"), str) or re.fullmatch(r"[0-9a-fA-F]{40}", current_head["sha"]) is None):
                return None
            if ((run_head_binding[0] == local_binding[0]) != (run_head_binding[1] == local_binding[1])
                or (current_head_binding[0] == local_binding[0]) != (current_head_binding[1] == local_binding[1])):
                return None
            if (current.get("state") != "open" or current["draft"]
                or base["ref"] != current_base["ref"] or head["sha"] != current_head["sha"]
                or current_base["ref"] != default_branch or run_head_binding != current_head_binding
                or current_head_binding != local_binding):
                continue
            identifiers.add(run_id)
    # 予約は古い実runから200件に限定し、超過分の権限と時計はdurable collectorへ残す。
    return sorted(identifiers)[:200]


# A priority direct event retains its low-latency preemption when no sensor is
# active. If one is admitted, serialize behind it: cancelling the source would
# make a late-numbered PR miss the 50-target early writer and exceed its review
# latch.
if (
    os.environ.get("DIRECT_PRIORITY_FENCE") == "1"
    and valid and reconcile
    and (event_name in {"pull_request_target", "issues", "issue_comment"} or cohort_recovery is not None)
):
    active_sensors = active_local_review_sensors()
    if active_sensors is None:
        valid = False
    elif active_sensors:
        # Preserve the sensor's pending dispatcher before this direct event
        # reaches the shared group; GitHub otherwise replaces that slot.
        sensor_reservation = True
        sensor_run_ids = active_sensors

# 分類不能な直接イベントはbarrierを要求するが、予約できなかったsensorの
# 実行を取り消してはならない。予約走査自体の失敗もこの境界で扱う。
if not valid:
    priority = False

# 既存分類の成功結果だけをjournal producerへ渡す。未started時計はproducerが元attemptから読む。
cohort_member_hint = None
if event_name == "workflow_run" and valid and reconcile and isinstance(locals().get("run"), dict) and run.get("name") == "PR governance review sensor":
    try:
        body = pull.get("body")
        if (not isinstance(body, str) or "\0" in body or type(run.get("workflow_id")) is not int or run["workflow_id"] < 1):
            raise ValueError("Unavailable strict source binding")
        import hashlib
        cohort_member_hint = {
            "source_run_id": run["id"], "source_run_attempt": 1,
            "source_workflow_id": run["workflow_id"], "source_run_number": run["run_number"],
            "source_event": run["event"], "pr_number": pull["number"], "base_ref": pull["base"]["ref"],
            "base_sha": pull["base"]["sha"].lower(), "head_sha": pull["head"]["sha"].lower(),
            "pr_body_sha256": hashlib.sha256(body.encode("utf-8", "strict")).hexdigest(),
            "latch_started_at": None, "source_deadline_epoch": None,
            "repository": repository, "repository_id": repository_id,
        }
    except (ValueError, UnicodeError, KeyError, TypeError):
        # journalを作れない入力を既存分類の成功やsource ACKへ昇格しない。
        cohort_member_hint = None

completed_sensor_noop = False
if (valid and reconcile and event_name == "workflow_run" and isinstance(cohort_member_hint, dict)
    and isinstance(run, dict) and run.get("name") == "PR governance review sensor"
    and run.get("status") == "completed" and run.get("conclusion") == "success"):
    try:
        import base64, hashlib
        workflow_sha = os.environ["WORKFLOW_SHA"]
        content = request(f"repos/{repository}/contents/scripts/review/pr_governance_cohort.py?ref={workflow_sha}")
        if not isinstance(content, dict) or content.get("type") != "file" or content.get("encoding") != "base64" or not isinstance(content.get("content"), str) or len(content["content"]) > 300000:
            raise ValueError("Untrusted cohort reader")
        data = base64.b64decode(content["content"].replace("\n", ""), validate=True)
        if hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest() != content.get("sha"):
            raise ValueError("Cohort reader blob changed")
        namespace = {"__name__": "trusted_cohort_preflight"}
        exec(compile(data.decode("utf-8", "strict"), "trusted-cohort-preflight", "exec"), namespace)
        budget = namespace["ReadBudget"](300)
        transport = namespace["_Transport"](os.environ["GH_TOKEN"], budget)
        reader = namespace["_Reader"](transport.json, transport.archive, budget, root_deadline_epoch)
        workflow = request(f"repos/{repository}/contents/.github/workflows/pr-governance.yml?ref={workflow_sha}")
        if not isinstance(workflow, dict) or not isinstance(workflow.get("sha"), str): raise ValueError("Workflow blob missing")
        completed_sensor_noop = namespace["completed_sensor_noop"](run, pull, reader=reader, workflow_sha=workflow_sha, workflow_blob=workflow["sha"])
        if completed_sensor_noop:
            reconcile = False
            priority = False
    except (ValueError, TypeError, KeyError, RuntimeError, UnicodeError):
        # 追加no-op証明の欠落は通常reconciliationへ戻し、成功免除にしない。
        completed_sensor_noop = False

with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
    output.write("reconcile=" + ("true" if reconcile else "false") + "\n")
    output.write("valid=" + ("true" if valid else "false") + "\n")
    output.write("priority=" + ("true" if priority else "false") + "\n")
    output.write("sensor_reservation=" + ("true" if sensor_reservation else "false") + "\n")
    output.write("sensor_run_ids=" + json.dumps(sensor_run_ids, separators=(",", ":")) + "\n")
    output.write("cohort_recovery_owner=" + (str(cohort_recovery["owner"]) if cohort_recovery else "") + "\n")
    output.write("cohort_recovery_journal=" + (str(cohort_recovery["journal"]) if cohort_recovery else "") + "\n")
    output.write("cohort_recovery_digest=" + (cohort_recovery["digest"] if cohort_recovery else "") + "\n")
    output.write("completed_sensor_noop=" + ("true" if completed_sensor_noop else "false") + "\n")
    output.write("cohort_member_hint=" + json.dumps(cohort_member_hint, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n")
    output.write("pull_request_target_noop=" + ("true" if pull_request_target_noop else "false") + "\n")
    output.write("issue_event_noop=" + ("true" if issue_event_noop else "false") + "\n")
    output.write(f"root_deadline_epoch={root_deadline_epoch}\n")
