"""PR governance の immutable journal と cohort 境界。外部入力は常に拒否優先。"""

import base64
import hashlib
import io
import json
import math
import os
import re
import stat
import time
import urllib.error
import urllib.parse
import urllib.request
import zipfile
from datetime import datetime, timezone


MAX_BYTES = 8192
MAX_MEMBERS = 200
MAX_BATCH_TARGETS = 50
ACTIVE = frozenset({"requested", "queued", "waiting", "pending", "in_progress"})
SENSOR_EVENTS = frozenset({"pull_request", "pull_request_review", "pull_request_review_comment"})
MUTATING_JOBS = frozenset({
    "Service bound cohort review sources", "Arm barrier before reserved review sensors", "Establish resolver-failure merge barrier",
    "Preserve admitted sources before obsolete heavy preemption", "Publish serialized resolver-failure barrier marker",
    "Prepare immutable governance target partitions", "Reconcile all current governance pull requests",
    "Audit complete pending Check Run histories", "Write complete pending reconciliation Check Runs",
    "Release complete affected-head merge barrier", "Complete terminal repository-wide governance writers",
    "Dispatch failed selected cohort recovery",
})
MEMBER_FIELDS = frozenset({
    "source_run_id", "source_run_attempt", "source_workflow_id", "source_run_number", "source_event",
    "pr_number", "base_ref", "base_sha", "head_sha", "pr_body_sha256", "latch_started_at",
    "source_deadline_epoch", "repository", "repository_id",
})
JOURNAL_FIELDS = frozenset({
    "v", "kind", "producer_run_id", "producer_run_attempt", "repository", "repository_id",
    "workflow_sha", "workflow_blob_sha", "root_deadline_epoch", "member",
})
HEADER_FIELDS = frozenset({
    "v", "kind", "owner_dispatcher_run_id", "owner_run_attempt", "repository", "repository_id",
    "workflow_sha", "workflow_blob_sha", "root_deadline_epoch", "cohort_digest", "member_ids",
    "member_pages", "member_segments", "alias_pages", "batches", "root_journal",
})
PAGE_FIELDS = frozenset({"v", "kind", "owner_dispatcher_run_id", "entries"})
BATCH_FIELDS = frozenset({"v", "kind", "owner_dispatcher_run_id", "segment", "target_members"})
PAYLOAD_PREFIX = "Record verified cohort payload "
JOURNAL_PREFIX = "Record verified cohort journal "
SELECTED_PREFIX = "Record elected cohort admission "
HANDOFF_PREFIX = "Record closed cohort handoff "
ADMITTED_PREFIX = "Record verified cohort backend admission "
REGISTER_PREFIX = "Record bound cohort early writer "
PRODUCER_JOB = "Publish immutable governance cohort journal"
ELECTION_JOB = "Elect read-only governance cohort owner"
ADMISSION_JOB = "Admit verified governance cohort backend"


class CohortError(RuntimeError):
    pass


def _require(condition, message):
    if not condition:
        raise CohortError(message)


def _integer(value):
    return type(value) is int and 1 <= value <= 2**63 - 1


def _hex(value, width):
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{%d}" % width, value) is not None


def _digest(value):
    return isinstance(value, str) and re.fullmatch(r"sha256:[0-9a-f]{64}", value) is not None


def canonical(value):
    try:
        data = json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True, allow_nan=False).encode("ascii")
    except (TypeError, ValueError, UnicodeError) as error:
        raise CohortError("Invalid canonical payload") from error
    _require(len(data) <= MAX_BYTES, "Cohort payload exceeds the existing marker byte bound")
    return data


def decode_payload(data, fields):
    _require(isinstance(data, bytes) and len(data) <= MAX_BYTES, "Invalid payload bytes")
    def pairs(items):
        result = {}
        for key, value in items:
            _require(key not in result, "Duplicate payload field")
            result[key] = value
        return result
    try:
        value = json.loads(data.decode("utf-8", "strict"), object_pairs_hook=pairs,
                           parse_constant=lambda _: (_ for _ in ()).throw(CohortError("Nonfinite number")))
    except (ValueError, UnicodeError) as error:
        raise CohortError("Malformed payload") from error
    _require(isinstance(value, dict) and set(value) == fields and canonical(value) == data,
             "Noncanonical payload schema")
    _require(type(value.get("v")) is int and value["v"] == 1, "Unknown cohort schema")
    return value


def decode_archive(data):
    _require(isinstance(data, bytes) and len(data) <= MAX_BYTES, "Artifact archive exceeds its bound")
    try:
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            entries = archive.infolist()
            _require(len(entries) == 1, "Artifact must have exactly one entry")
            entry = entries[0]
            mode = entry.external_attr >> 16
            _require(entry.filename == "binding.json" and not entry.is_dir() and not entry.flag_bits & 1
                     and not stat.S_ISLNK(mode) and (not stat.S_IFMT(mode) or stat.S_ISREG(mode))
                     and 0 < entry.file_size <= MAX_BYTES and entry.compress_size <= MAX_BYTES,
                     "Invalid artifact entry")
            with archive.open(entry) as stream:
                payload = stream.read(MAX_BYTES + 1)
            _require(len(payload) == entry.file_size and len(payload) <= MAX_BYTES, "Artifact expansion exceeds its bound")
            return payload
    except (zipfile.BadZipFile, OSError, RuntimeError, ValueError) as error:
        if isinstance(error, CohortError):
            raise
        raise CohortError("Invalid artifact archive") from error


class _NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def _open_no_redirect(request, timeout):
    try:
        return urllib.request.build_opener(_NoRedirect()).open(request, timeout=timeout)
    except urllib.error.HTTPError as response:
        return response


def read_artifact_archive(endpoint, token, timeout, open_request=None):
    _require(isinstance(endpoint, str) and re.fullmatch(r"repos/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/actions/artifacts/[1-9][0-9]*/zip", endpoint),
             "Noncanonical artifact API endpoint")
    open_request = open_request or _open_no_redirect
    request = urllib.request.Request("https://api.github.com/" + endpoint,
                                     headers={"Authorization": "Bearer " + token, "Accept": "application/vnd.github+json",
                                              "X-GitHub-Api-Version": "2022-11-28"})
    try:
        response = open_request(request, timeout)
    except (OSError, urllib.error.URLError) as error:
        raise CohortError("Artifact API request failed") from None
    _require(response.code == 302, "Artifact API did not return a signed archive redirect")
    location = response.headers.get("Location")
    parsed = urllib.parse.urlsplit(location) if isinstance(location, str) else None
    _require(parsed is not None and parsed.scheme == "https" and parsed.hostname
             and (parsed.hostname.endswith(".blob.core.windows.net") or parsed.hostname.endswith(".actions.githubusercontent.com")) and parsed.username is None and parsed.password is None
             and not parsed.fragment, "Invalid artifact storage redirect")
    # 署名済みstorage URLへAPI認証headerを転送しない。URL自体も例外・ログへ含めない。
    try:
        archive = open_request(urllib.request.Request(location), timeout)
    except (OSError, urllib.error.URLError):
        raise CohortError("Artifact storage request failed") from None
    _require(archive.code == 200, "Artifact storage download failed")
    data = archive.read(MAX_BYTES + 1)
    _require(len(data) <= MAX_BYTES, "Artifact archive exceeds its bound")
    return data


def canonical_timestamp(value):
    _require(isinstance(value, str) and re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]{1,6})?(?:Z|[+-][0-9]{2}:[0-9]{2})", value), "Invalid raw stage timestamp")
    try:
        stamp = datetime.fromisoformat(value.replace("Z", "+00:00")).astimezone(timezone.utc)
    except ValueError as error:
        raise CohortError("Invalid raw stage timestamp") from error
    text = stamp.strftime("%Y-%m-%dT%H:%M:%S")
    if stamp.microsecond:
        text += "." + f"{stamp.microsecond:06d}".rstrip("0")
    return text + "Z"


def source_deadline(started_at, *, now):
    if started_at is None:
        return None
    canonical = canonical_timestamp(started_at)
    _require(canonical == started_at, "Source journal timestamp is not canonical UTC")
    started = datetime.fromisoformat(canonical.replace("Z", "+00:00")).timestamp()
    _require(started <= now, "Future source latch clock")
    result = started + 5400
    return int(result) if result.is_integer() else result


def validate_member(member, *, now=None):
    _require(isinstance(member, dict) and set(member) == MEMBER_FIELDS, "Invalid member schema")
    for key in ("source_run_id", "source_workflow_id", "source_run_number", "pr_number", "repository_id"):
        _require(_integer(member[key]), "Invalid member integer")
    _require(type(member["source_run_attempt"]) is int and member["source_run_attempt"] == 1
             and member["source_event"] in SENSOR_EVENTS and _repository_name(member["repository"])
             and isinstance(member["base_ref"], str) and member["base_ref"] and not any(c.isspace() for c in member["base_ref"])
             and _hex(member["base_sha"], 40) and _hex(member["head_sha"], 40)
             and _hex(member["pr_body_sha256"], 64), "Invalid member identity")
    expected = source_deadline(member["latch_started_at"], now=time.time() if now is None else now)
    _require(member["source_deadline_epoch"] == expected and (expected is None or type(member["source_deadline_epoch"]) in (int, float)),
             "Source clock reset")
    return member


def _repository_name(value):
    return isinstance(value, str) and re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", value) is not None


def repository_matches(value, name, identifier):
    if not isinstance(value, dict) or type(value.get("id")) is not int or value["id"] != identifier:
        return False
    if value.get("full_name") == name:
        return value.get("url") in (None, "https://api.github.com/repos/" + name)
    return value.get("name") == name.split("/")[1] and value.get("url") == "https://api.github.com/repos/" + name


def _validate_member_source_identity(member, source):
    validate_member(member)
    _require(isinstance(source, dict), "Source/PR unavailable")
    repo, repo_id = member["repository"], member["repository_id"]
    path = ".github/workflows/pr-governance-review-events.yml"
    _require(type(source.get("id")) is int and source["id"] == member["source_run_id"]
             and type(source.get("run_attempt")) is int and source["run_attempt"] == 1
             and type(source.get("workflow_id")) is int and source["workflow_id"] == member["source_workflow_id"]
             and type(source.get("run_number")) is int and source["run_number"] == member["source_run_number"]
             and source.get("name") == "PR governance review sensor" and source.get("event") == member["source_event"]
             and source.get("head_sha") == member["head_sha"]
             and source.get("path") in {path, path + "@" + member["base_ref"], path + "@refs/heads/" + member["base_ref"]}
             and repository_matches(source.get("repository"), repo, repo_id)
             and repository_matches(source.get("head_repository"), repo, repo_id)
             and (source.get("status") in ACTIVE and source.get("conclusion") is None
                  or source.get("status") == "completed" and source.get("conclusion") == "success"),
             "Source identity/state drift")
    pulls = source.get("pull_requests")
    _require(isinstance(pulls, list) and len(pulls) == 1 and isinstance(pulls[0], dict), "Ambiguous source PR")
    pull = pulls[0]
    for actual in (pull,):
        base, head = actual.get("base"), actual.get("head")
        _require(type(actual.get("number")) is int and actual["number"] == member["pr_number"]
                 and isinstance(base, dict) and isinstance(head, dict)
                 and base.get("ref") == member["base_ref"] and base.get("sha") == member["base_sha"]
                 and head.get("sha") == member["head_sha"]
                 and repository_matches(base.get("repo"), repo, repo_id)
                 and repository_matches(head.get("repo"), repo, repo_id), "Current/source PR binding drift")


def member_source_matches(member, source, current_pr):
    try:
        validate_member(member)
        _require(isinstance(source, dict) and isinstance(current_pr, dict), "Source/PR unavailable")
        repo, repo_id = member["repository"], member["repository_id"]
        path = ".github/workflows/pr-governance-review-events.yml"
        _require(type(source.get("id")) is int and source["id"] == member["source_run_id"]
                 and type(source.get("run_attempt")) is int and source["run_attempt"] == 1
                 and type(source.get("workflow_id")) is int and source["workflow_id"] == member["source_workflow_id"]
                 and type(source.get("run_number")) is int and source["run_number"] == member["source_run_number"]
                 and source.get("name") == "PR governance review sensor" and source.get("event") == member["source_event"]
                 and source.get("head_sha") == member["head_sha"]
                 and source.get("path") in {path, path + "@" + member["base_ref"], path + "@refs/heads/" + member["base_ref"]}
                 and repository_matches(source.get("repository"), repo, repo_id)
                 and repository_matches(source.get("head_repository"), repo, repo_id)
                 and (source.get("status") in ACTIVE and source.get("conclusion") is None
                      or source.get("status") == "completed" and source.get("conclusion") == "success"),
                 "Source identity/state drift")
        pulls = source.get("pull_requests")
        _require(isinstance(pulls, list) and len(pulls) == 1 and isinstance(pulls[0], dict), "Ambiguous source PR")
        pull = pulls[0]
        for actual in (pull, current_pr):
            base, head = actual.get("base"), actual.get("head")
            _require(type(actual.get("number")) is int and actual["number"] == member["pr_number"]
                     and isinstance(base, dict) and isinstance(head, dict)
                     and base.get("ref") == member["base_ref"] and base.get("sha") == member["base_sha"]
                     and head.get("sha") == member["head_sha"]
                     and repository_matches(base.get("repo"), repo, repo_id)
                     and repository_matches(head.get("repo"), repo, repo_id), "Current/source PR binding drift")
        body = current_pr.get("body")
        _require(current_pr.get("state") == "open" and current_pr.get("draft") is False and isinstance(body, str)
                 and "\0" not in body and hashlib.sha256(body.encode("utf-8", "strict")).hexdigest() == member["pr_body_sha256"],
                 "Current Ready PR/body drift")
        return True
    except (CohortError, KeyError, TypeError, UnicodeError):
        return False


def build_batches(members):
    _require(isinstance(members, dict) and 1 <= len(members) <= MAX_MEMBERS
             and all(_integer(key) and isinstance(value, dict) and _integer(value.get("pr_number")) for key, value in members.items()),
             "Existing local reservation cap")
    targets = {}
    for identifier, member in members.items():
        targets.setdefault(member["pr_number"], []).append(identifier)
    numbers = sorted(targets)
    return [{"segment": index // MAX_BATCH_TARGETS + 1,
             "target_members": [[number, sorted(targets[number])] for number in numbers[index:index + MAX_BATCH_TARGETS]]}
            for index in range(0, len(numbers), MAX_BATCH_TARGETS)]


def _ref(value):
    _require(isinstance(value, dict) and set(value) == {"artifact_id", "artifact_digest"}
             and _integer(value["artifact_id"]) and _digest(value["artifact_digest"]), "Invalid immutable child reference")
    return value


class _Reader:
    def __init__(self, read_json, read_archive, budget, deadline, trusted_workflow_blob=None):
        _require(trusted_workflow_blob is None or _hex(trusted_workflow_blob, 40), "Invalid compiled dispatcher blob pin")
        self.trusted_workflow_blob = trusted_workflow_blob
        self.read_json, self.read_archive, self.budget, self.deadline = read_json, read_archive, budget, deadline
        self.cache = {}

    def request(self, endpoint, *, archive=False):
        remaining = self.deadline - time.time()
        _require(remaining > 0, "Original cohort deadline expired")
        self.budget.charge()
        return (self.read_archive if archive else self.read_json)(endpoint, timeout=min(20, remaining))

    def jobs(self, repository, identifier):
        key = ("jobs", repository, identifier)
        if key in self.cache:
            return self.cache[key]
        response = self.request(f"repos/{repository}/actions/runs/{identifier}/attempts/1/jobs?per_page=100&page=1")
        jobs = response.get("jobs") if isinstance(response, dict) else None
        total = response.get("total_count") if isinstance(response, dict) else None
        _require(type(total) is int and 0 <= total <= 100 and isinstance(jobs, list) and len(jobs) == total
                 and all(isinstance(job, dict) and _integer(job.get("id")) and job.get("run_id") == identifier
                         and ("run_attempt" not in job or type(job["run_attempt"]) is int and job["run_attempt"] == 1) for job in jobs)
                 and len({job["id"] for job in jobs}) == len(jobs), "Incomplete or ambiguous exact-attempt jobs")
        self.cache[key] = jobs
        return jobs

    def producer(self, repository, identifier, sha, blob):
        key = ("producer", repository, identifier)
        if key in self.cache:
            existing = self.cache[key]
            _require(existing["head_sha"] == sha, "Producer identity drift")
            return existing
        endpoint = f"repos/{repository}/actions/runs/{identifier}"
        before = self.request(endpoint)
        _require(isinstance(before, dict) and type(before.get("id")) is int and before["id"] == identifier
                 and type(before.get("run_attempt")) is int and before["run_attempt"] == 1
                 and (before.get("name") == "PR governance dispatcher" or before.get("name") == before.get("display_title")
                      and isinstance(before.get("display_title"), str) and re.fullmatch(r"sensor=[1-9][0-9]* action=(?:in_progress|completed)", before["display_title"]))
                 and before.get("head_sha") == sha,
                 "Journal producer run identity")
        branch = before.get("head_branch")
        path = ".github/workflows/pr-governance.yml"
        _require(isinstance(branch, str) and before.get("path") in {path, path + "@" + branch, path + "@refs/heads/" + branch},
                 "Journal producer workflow path")
        default_key = ("workflow", repository, sha, blob)
        if default_key not in self.cache:
            repository_data = self.request(f"repos/{repository}")
            _require(isinstance(repository_data, dict) and repository_data.get("default_branch") == branch,
                     "Producer is outside the current default branch")
            if self.trusted_workflow_blob is not None:
                _require(blob == self.trusted_workflow_blob, "Producer differs from compiled trusted dispatcher")
                self.cache[default_key] = None
                after = self.request(endpoint)
                keys = ("id", "run_attempt", "name", "head_sha", "head_branch", "path", "event", "workflow_id", "run_number", "repository", "head_repository")
                _require(isinstance(after, dict) and all(before.get(key) == after.get(key) for key in keys), "Pinned producer double-read drift")
                self.cache[key] = after
                return after
            ref = self.request(f"repos/{repository}/git/ref/heads/{urllib.parse.quote(branch, safe='')}")
            _require(isinstance(ref, dict) and ref.get("object", {}).get("sha") == sha, "Current default workflow changed")
            content = self.request(f"repos/{repository}/contents/{path}?ref={sha}")
            _require(isinstance(content, dict) and content.get("path") == path and content.get("encoding") == "base64"
                     and content.get("sha") == blob and isinstance(content.get("content"), str), "Producer workflow content metadata")
            try:
                code = base64.b64decode(content["content"].replace("\n", ""), validate=True)
            except (ValueError, TypeError) as error:
                raise CohortError("Producer workflow bytes") from error
            _require(type(content.get("size")) is int and content["size"] == len(code)
                     and hashlib.sha1(b"blob " + str(len(code)).encode() + b"\0" + code).hexdigest() == blob,
                     "Producer workflow blob drift")
            self.cache[default_key] = code
        after = self.request(endpoint)
        immutable = ("id", "run_attempt", "name", "head_sha", "head_branch", "path", "event", "workflow_id", "run_number", "repository", "head_repository")
        _require(isinstance(after, dict) and all(before.get(key) == after.get(key) for key in immutable), "Producer double-read drift")
        self.cache[key] = after
        return after

    def artifact(self, reference, kind, repository, *, expected_producer=None, failed_owner=None):
        reference = _ref(reference)
        identifier, digest = reference["artifact_id"], reference["artifact_digest"]
        cache_key = ("artifact", repository, identifier, digest, kind, failed_owner)
        if cache_key in self.cache:
            payload, metadata = self.cache[cache_key]
            producer = payload.get("producer_run_id") if kind == "source" else payload.get("owner_dispatcher_run_id")
            _require(expected_producer is None or producer == expected_producer, "Cached foreign artifact producer")
            return payload, metadata
        endpoint = f"repos/{repository}/actions/artifacts/{identifier}"
        metadata = self.request(endpoint)
        _require(isinstance(metadata, dict) and type(metadata.get("id")) is int and metadata["id"] == identifier
                 and metadata.get("expired") is False and metadata.get("digest") == digest
                 and type(metadata.get("size_in_bytes")) is int and 0 < metadata["size_in_bytes"] <= MAX_BYTES,
                 "Invalid artifact metadata")
        archive = self.request(endpoint + "/zip", archive=True)
        _require(isinstance(archive, bytes) and "sha256:" + hashlib.sha256(archive).hexdigest() == digest, "Artifact digest mismatch")
        fields = {"source": JOURNAL_FIELDS, "cohort": HEADER_FIELDS, "members": PAGE_FIELDS, "aliases": PAGE_FIELDS, "batch": BATCH_FIELDS}[kind]
        payload = decode_payload(decode_archive(archive), fields)
        _require(payload.get("kind") == kind, "Wrong artifact role")
        producer_id = payload.get("producer_run_id") if kind == "source" else payload.get("owner_dispatcher_run_id")
        _require(_integer(producer_id) and (expected_producer is None or producer_id == expected_producer), "Foreign artifact producer")
        workflow_run = metadata.get("workflow_run")
        _require(isinstance(workflow_run, dict) and workflow_run.get("id") == producer_id, "Artifact run binding")
        expected_name = f"krr-governance-{kind}-{producer_id}-attempt-1"
        if kind == "source":
            member = payload.get("member")
            expected_name = (f"krr-governance-source-{member.get('source_run_id')}-attempt-1" if isinstance(member, dict)
                             else f"krr-governance-source-driver-{producer_id}-attempt-1")
        if kind in {"members", "aliases", "batch"}:
            _require(isinstance(metadata.get("name"), str) and re.fullmatch(re.escape(expected_name) + r"-[1-4]", metadata["name"]), "Artifact child name")
        else:
            _require(metadata.get("name") == expected_name, "Artifact name binding")
        jobs = self.jobs(repository, producer_id)
        name = PRODUCER_JOB if kind == "source" else ELECTION_JOB
        creators = [job for job in jobs if job.get("name") == name]
        _require(len(creators) == 1, "Ambiguous artifact creator job")
        creator = creators[0]
        failed_header = (kind == "cohort" and failed_owner == producer_id and expected_producer == producer_id
                         and creator.get("status") == "completed" and creator.get("conclusion") == "failure")
        _require((kind != "source" or creator.get("status") == "completed" and creator.get("conclusion") == "success")
                 and creator.get("status") in ACTIVE | {"completed"}
                 and (creator.get("status") != "completed" or creator.get("conclusion") == "success" or failed_header), "Artifact creator failed")
        _require(creator.get("head_sha") == workflow_run.get("head_sha"), "Artifact creator workflow head drift")
        steps = creator.get("steps")
        marker = PAYLOAD_PREFIX + kind + " sha256=" + hashlib.sha256(canonical(payload)).hexdigest()
        _require(isinstance(steps, list) and all(isinstance(step, dict) for step in steps), "Artifact creator stages missing")
        candidates = [step for step in steps if step.get("name") == marker]
        _require(len(candidates) == 1 and candidates[0].get("status") == "completed" and candidates[0].get("conclusion") == "success",
                 "Missing/foreign/duplicate artifact binding stage")
        uploads = [step for step in steps if isinstance(step.get("name"), str) and step["name"] == "Upload immutable " + metadata["name"]]
        _require(len(uploads) == 1 and uploads[0].get("status") == "completed" and uploads[0].get("conclusion") == "success",
                 "Artifact upload stage incomplete")
        after = self.request(endpoint)
        _require(after == metadata, "Artifact metadata drift")
        self.cache[cache_key] = (payload, metadata)
        return payload, metadata


def load_cohort(artifact_id, digest, *, expected_owner, expected_segment, read_json, read_archive, budget, deadline, member_ids=None, trusted_workflow_blob=None, allow_empty=False):
    _require(type(allow_empty) is bool and (not allow_empty or expected_segment is None and member_ids is None), "Empty context cannot grant selective authority")
    _require(_integer(artifact_id) and _digest(digest) and _integer(expected_owner)
             and (expected_segment is None or type(expected_segment) is int and 1 <= expected_segment <= 4), "Invalid cohort binding")
    repository = os.environ.get("GITHUB_REPOSITORY", "")
    _require(_repository_name(repository), "Missing exact repository context")
    reader = _Reader(read_json, read_archive, budget, deadline, trusted_workflow_blob)
    header, metadata = reader.artifact({"artifact_id": artifact_id, "artifact_digest": digest}, "cohort", repository,
                                       expected_producer=expected_owner)
    _require(header["owner_run_attempt"] == 1 and type(header["owner_run_attempt"]) is int
             and header["repository"] == repository and _integer(header["repository_id"])
             and _hex(header["workflow_sha"], 40) and _hex(header["workflow_blob_sha"], 40)
             and _integer(header["root_deadline_epoch"]) and time.time() < header["root_deadline_epoch"],
             "Invalid cohort header identity/clock")
    reader.deadline = min(reader.deadline, header["root_deadline_epoch"])
    content = dict(header)
    content.pop("cohort_digest")
    _require(_hex(header["cohort_digest"], 64) and hashlib.sha256(canonical(content)).hexdigest() == header["cohort_digest"], "Cohort canonical digest")
    producer = reader.producer(repository, expected_owner, header["workflow_sha"], header["workflow_blob_sha"])
    _require(repository_matches(producer.get("repository"), repository, header["repository_id"])
             and metadata["workflow_run"].get("repository_id") == header["repository_id"]
             and metadata["workflow_run"].get("head_repository_id") == header["repository_id"]
             and metadata["workflow_run"].get("head_sha") == header["workflow_sha"], "Cohort repository/workflow binding")
    identifiers = header["member_ids"]
    _require(isinstance(identifiers, list) and (0 if allow_empty else 1) <= len(identifiers) <= MAX_MEMBERS
             and all(_integer(identifier) for identifier in identifiers) and identifiers == sorted(set(identifiers)), "Cohort member reservation cap")
    segments = header["member_segments"]
    _require(isinstance(segments, str) and len(segments) == len(identifiers) and re.fullmatch(r"[1-4]*" if allow_empty and not identifiers else r"[1-4]+", segments), "Invalid member segment index")
    indexed_segments = dict(zip(identifiers, map(int, segments)))
    selected = ({identifier for identifier in identifiers if expected_segment is None or indexed_segments[identifier] == expected_segment}
                if member_ids is None else member_ids)
    _require(isinstance(selected, set) and (bool(selected) or allow_empty and not identifiers) and selected <= set(identifiers), "Foreign selective member")
    selected_segments = {indexed_segments[identifier] for identifier in selected}
    _require(isinstance(header["member_pages"], list) and len(header["member_pages"]) == (len(identifiers) + 49) // 50
             and isinstance(header["batches"], list) and (0 if allow_empty and not identifiers else 1) <= len(header["batches"]) <= 4
             and set(indexed_segments.values()) == set(range(1, len(header["batches"]) + 1)), "Invalid cohort page count")
    _require(isinstance(header["alias_pages"], list) and len(header["alias_pages"]) == (len(identifiers) + 49) // 50, "Invalid alias page count")
    all_refs = [_ref(ref) for ref in header["member_pages"] + header["alias_pages"] + header["batches"] + [header["root_journal"]]]
    _require(len({ref["artifact_id"] for ref in all_refs}) == len(all_refs), "Duplicate cohort child artifact")
    journals, owner_ids = {}, {}
    for index, reference in enumerate(header["member_pages"]):
        slice_ids = identifiers[index * 50:(index + 1) * 50]
        if not selected.intersection(slice_ids):
            continue
        page, _ = reader.artifact(reference, "members", repository, expected_producer=expected_owner)
        entries = page["entries"]
        _require(isinstance(entries, list) and len(entries) == len(slice_ids), "Invalid source reference page")
        local_ids = []
        for entry in entries:
            _require(isinstance(entry, list) and len(entry) == 4 and all(_integer(value) for value in entry[:3]) and _digest(entry[3]), "Malformed journal reference")
            identifier, creator, journal_id, journal_digest = entry
            local_ids.append(identifier)
            if identifier in selected:
                journal, journal_metadata = reader.artifact({"artifact_id": journal_id, "artifact_digest": journal_digest}, "source", repository, expected_producer=creator)
                _require(type(journal["producer_run_attempt"]) is int and journal["producer_run_attempt"] == 1
                         and journal["repository"] == repository and journal["repository_id"] == header["repository_id"]
                         and _integer(journal["root_deadline_epoch"]) and journal["root_deadline_epoch"] >= header["root_deadline_epoch"], "Journal original root clock/binding")
                validate_journal_clock(journal, reader)
                member = validate_member(journal["member"])
                _require(member["source_run_id"] == identifier and member["repository"] == repository
                         and member["repository_id"] == header["repository_id"], "Member source identity drift")
                journal_producer = reader.producer(repository, creator, journal["workflow_sha"], journal["workflow_blob_sha"])
                _require(repository_matches(journal_producer.get("repository"), repository, header["repository_id"])
                         and journal_metadata["workflow_run"].get("head_sha") == journal["workflow_sha"], "Journal producer repository/workflow drift")
                journals[identifier] = member
                owner_ids[identifier] = creator
        _require(local_ids == slice_ids, "Incomplete/duplicate/unsorted source page")
    _require(set(journals) == selected, "Missing selected member proof")
    targets = {}
    for segment in sorted(selected_segments):
        batch, _ = reader.artifact(header["batches"][segment - 1], "batch", repository, expected_producer=expected_owner)
        _require(type(batch["segment"]) is int and batch["segment"] == segment, "Wrong cohort segment")
        entries = batch["target_members"]
        _require(isinstance(entries, list) and 1 <= len(entries) <= MAX_BATCH_TARGETS, "Early target cap")
        numbers, batch_ids = [], []
        for entry in entries:
            _require(isinstance(entry, list) and len(entry) == 2 and _integer(entry[0]) and isinstance(entry[1], list)
                     and entry[1] and all(_integer(identifier) for identifier in entry[1]) and entry[1] == sorted(set(entry[1])), "Invalid target/member mapping")
            number, ids = entry
            numbers.append(number)
            batch_ids.extend(ids)
            _require(number not in targets, "Duplicate cohort target")
            targets[number] = ids
            for identifier in set(ids) & selected:
                _require(journals[identifier]["pr_number"] == number, "Member target drift")
        _require(numbers == sorted(set(numbers)) and len(batch_ids) == len(set(batch_ids))
                 and sorted(batch_ids) == [identifier for identifier in identifiers if indexed_segments[identifier] == segment], "Incomplete batch member union")
    root_journal, root_metadata = reader.artifact(header["root_journal"], "source", repository)
    _require(root_journal["root_deadline_epoch"] == header["root_deadline_epoch"]
             and root_journal["repository_id"] == header["repository_id"], "Original root clock reset")
    validate_journal_clock(root_journal, reader)
    root_producer = reader.producer(repository, root_journal["producer_run_id"], root_journal["workflow_sha"], root_journal["workflow_blob_sha"])
    _require(repository_matches(root_producer.get("repository"), repository, header["repository_id"])
             and root_metadata["workflow_run"].get("head_sha") == root_journal["workflow_sha"], "Root clock creator drift")
    if not identifiers:
        _require(header["member_pages"] == header["alias_pages"] == header["batches"] == [] and segments == "", "Empty cohort has hidden selective authority")
        jobs = reader.jobs(repository, expected_owner)
        for job_name, prefix in ((ELECTION_JOB, SELECTED_PREFIX), (ADMISSION_JOB, ADMITTED_PREFIX)):
            matches = [job for job in jobs if job.get("name") == job_name]
            _require(len(matches) == 1 and matches[0].get("head_sha") == header["workflow_sha"], "Empty cohort owner stage missing/ambiguous")
            job = matches[0]
            _require((job_name == ELECTION_JOB and job.get("status") == "in_progress" and job.get("conclusion") is None)
                     or job.get("status") == "completed" and job.get("conclusion") == "success", "Empty cohort owner was not selected/admitted")
            steps = job.get("steps")
            marker = prefix + f"owner={expected_owner} artifact={artifact_id} digest={digest}"
            _require(isinstance(steps, list) and all(isinstance(step, dict) for step in steps), "Empty owner native stages malformed")
            stages = [step for step in steps if isinstance(step.get("name"), str) and step["name"].startswith(prefix)]
            _require(len(stages) == 1 and stages[0].get("name") == marker and stages[0].get("status") == "completed" and stages[0].get("conclusion") == "success", "Empty owner native binding missing/ambiguous")
    return {"header": header, "members_by_id": journals, "target_members_by_number": targets,
            "registered_writers": {}, "artifact_id": artifact_id, "artifact_digest": digest,
            "cohort_digest": header["cohort_digest"], "producer_ids_by_member": owner_ids, "loaded_segments": sorted(selected_segments), "trusted_workflow_blob": trusted_workflow_blob,
            "recovered_predecessor_ids": sorted(reader.cache.get("recovered_predecessor_ids", set()))}


def validate_target_members(cohort, segment, target_numbers):
    _require(isinstance(cohort, dict) and type(segment) is int and 1 <= segment <= 4
             and isinstance(target_numbers, list) and 1 <= len(target_numbers) <= MAX_BATCH_TARGETS
             and all(_integer(number) for number in target_numbers) and target_numbers == sorted(set(target_numbers)), "Invalid early batch target scope")
    _require(cohort.get("loaded_segments") == [segment], "Wrong loaded cohort segment")
    mapping = cohort["target_members_by_number"]
    _require(set(target_numbers) == set(mapping), "Early targets differ from immutable cohort segment")
    members = cohort["members_by_id"]
    _require(all(identifier in members for number in target_numbers for identifier in mapping[number]), "Missing batch member proof")
    return {number: [members[identifier] for identifier in mapping[number]] for number in target_numbers}


def verify_cohort_lease(cohort, *, phase, read_json, budget, deadline):
    _require(phase in {"early", "all"}, "Unknown cohort lease phase")
    header = cohort["header"]
    repository, owner = header["repository"], header["owner_dispatcher_run_id"]
    reader = _Reader(read_json, None, budget, deadline)
    endpoint = f"repos/{repository}/actions/runs/{owner}"
    before = reader.request(endpoint)
    _require(isinstance(before, dict) and before.get("id") == owner and type(before.get("run_attempt")) is int
             and before["run_attempt"] == 1 and before.get("head_sha") == header["workflow_sha"]
             and before.get("status") in ACTIVE | {"completed"}
             and (before.get("status") != "completed" or before.get("conclusion") == "success"), "Cohort owner lifecycle drift")
    jobs = reader.jobs(repository, owner)
    admissions = [job for job in jobs if job.get("name") == ADMISSION_JOB]
    _require(len(admissions) == 1 and admissions[0].get("status") == "completed" and admissions[0].get("conclusion") == "success", "Cohort backend admission did not succeed")
    admitted = ADMITTED_PREFIX + f"owner={owner} artifact={cohort['artifact_id']} digest={cohort['artifact_digest']}"
    admission_steps = admissions[0].get("steps")
    _require(isinstance(admission_steps, list), "Backend admission stage missing")
    bound_admissions = [step for step in admission_steps if isinstance(step, dict) and step.get("name") == admitted]
    _require(len(bound_admissions) == 1 and bound_admissions[0].get("status") == "completed" and bound_admissions[0].get("conclusion") == "success", "Backend admission artifact binding drift")
    election = [job for job in jobs if job.get("name") == ELECTION_JOB]
    _require(len(election) == 1, "Missing/ambiguous cohort election")
    election = election[0]
    marker = SELECTED_PREFIX + f"owner={owner} artifact={cohort['artifact_id']} digest={cohort['artifact_digest']}"
    steps = election.get("steps")
    _require(isinstance(steps, list) and all(isinstance(step, dict) for step in steps), "Election stages missing")
    candidates = [step for step in steps if step.get("name") == marker]
    _require(len(candidates) == 1 and candidates[0].get("status") == "completed" and candidates[0].get("conclusion") == "success", "Unselected/foreign owner")
    if phase == "early":
        _require(election.get("status") == "in_progress" and election.get("conclusion") is None, "Cohort election lease is not active")
    else:
        closed = HANDOFF_PREFIX + f"owner={owner} artifact={cohort['artifact_id']} digest={cohort['artifact_digest']}"
        handoffs = [step for step in steps if step.get("name") == closed]
        _require(election.get("status") == "completed" and election.get("conclusion") == "success"
                 and len(handoffs) == 1 and handoffs[0].get("status") == "completed" and handoffs[0].get("conclusion") == "success", "Cohort handoff incomplete")
    registered, seen_registrations = {}, set()
    pattern = re.compile(REGISTER_PREFIX + r"segment=([1-4]) run=([1-9][0-9]*) artifact=([1-9][0-9]*) digest=(sha256:[0-9a-f]{64})")
    for job in jobs:
        for step in job.get("steps", []):
            match = pattern.fullmatch(step.get("name", "")) if isinstance(step, dict) else None
            if match:
                segment, writer, artifact, digest = int(match[1]), int(match[2]), int(match[3]), match[4]
                _require(segment not in seen_registrations and artifact == cohort["artifact_id"] and digest == cohort["artifact_digest"]
                         and _integer(writer), "Ambiguous/foreign writer registration")
                seen_registrations.add(segment)
                if step.get("status") in ACTIVE and step.get("conclusion") is None:
                    continue
                _require(step.get("status") == "completed" and step.get("conclusion") == "success", "Failed writer registration")
                registered[segment] = writer
            elif isinstance(step, dict) and isinstance(step.get("name"), str) and step["name"].startswith(REGISTER_PREFIX):
                raise CohortError("Malformed writer registration")
    after = reader.request(endpoint)
    keys = ("id", "run_attempt", "head_sha", "head_branch", "path", "event", "workflow_id", "run_number", "repository", "head_repository")
    _require(isinstance(after, dict) and all(before.get(key) == after.get(key) for key in keys)
             and after.get("status") in ACTIVE | {"completed"}
             and (after.get("status") != "completed" or after.get("conclusion") == "success"), "Owner double-read lifecycle drift")
    cohort["registered_writers"] = registered


def consumer_is_covered(cohort, generation, *, read_json, read_archive, budget, deadline):
    """選出済memberの取消consumerだけを、ACKとは独立して肯定分類する。"""
    return _consumer_follower_proof(cohort, generation, read_json=read_json, read_archive=read_archive,
                                    budget=budget, deadline=deadline, deferred=False)


def consumer_is_deferred(cohort, generation, *, read_json, read_archive, budget, deadline):
    """超過分の非mutation followerを分類し、member追加やACK権限を与えない。"""
    return _consumer_follower_proof(cohort, generation, read_json=read_json, read_archive=read_archive,
                                    budget=budget, deadline=deadline, deferred=True)


def _consumer_follower_proof(cohort, generation, *, read_json, read_archive, budget, deadline, deferred):
    try:
        header = cohort["header"]
        repository, identifier = header["repository"], generation.get("id")
        _require(_integer(identifier) and identifier != header["owner_dispatcher_run_id"]
                 and generation.get("status") == "completed" and generation.get("conclusion") in {"cancelled", "failure"}
                 and (generation.get("event") == "workflow_run" or identifier in cohort.get("recovered_predecessor_ids", [])), "Not a cancelled source consumer")
        reader = _Reader(read_json, read_archive, budget, deadline, cohort.get("trusted_workflow_blob"))
        if identifier in cohort.get("recovered_predecessor_ids", []):
            _require(not deferred, "Recovered predecessor is not a deferred source")
            current = reader.producer(repository, header["owner_dispatcher_run_id"], header["workflow_sha"], header["workflow_blob_sha"])
            _require(repository_matches(current.get("repository"), repository, header["repository_id"]), "Recovery owner repository drift")
            preflights = [job for job in reader.jobs(repository, current["id"]) if job.get("name") == "Preflight workflow_run governance source"]
            _require(len(preflights) == 1 and preflights[0].get("status") == "completed" and preflights[0].get("conclusion") == "success"
                     and preflights[0].get("head_sha") == header["workflow_sha"] and isinstance(preflights[0].get("steps"), list), "Recovery owner preflight did not succeed")
            recovery = recovery_marker(preflights[0]["steps"])
            _require(recovery is not None and recovery["owner"] == identifier and recovery["root_deadline_epoch"] == header["root_deadline_epoch"], "Missing exact recovered predecessor native stage")
            proof = recover_failed_owner(reader, repository, identifier, header["workflow_sha"], header["workflow_blob_sha"],
                                         {"artifact_id": recovery["journal"], "artifact_digest": recovery["digest"]})
            _require(proof == recovery, "Recovered predecessor root proof changed")
            old = reader.cache[("producer", repository, identifier)]
            keys = ("id", "run_attempt", "name", "head_sha", "head_branch", "path", "event", "workflow_id", "run_number", "repository", "status", "conclusion")
            _require(all(old.get(key) == generation.get(key) for key in keys), "Recovered predecessor generation drift")
            return True
        listing = reader.request(f"repos/{repository}/actions/runs/{identifier}/artifacts?per_page=100&page=1")
        artifacts = listing.get("artifacts") if isinstance(listing, dict) else None
        _require(isinstance(artifacts, list) and type(listing.get("total_count")) is int
                 and listing["total_count"] == len(artifacts) <= 100, "Incomplete consumer artifact inventory")
        candidates = [item for item in artifacts if isinstance(item, dict) and isinstance(item.get("name"), str) and item["name"].startswith("krr-governance-source-")]
        _require(len(candidates) == 1, "Missing/ambiguous consumer journal")
        journal, _ = reader.artifact({"artifact_id": candidates[0]["id"], "artifact_digest": candidates[0]["digest"]}, "source", repository, expected_producer=identifier)
        member = validate_member(journal["member"])
        _require(journal["repository"] == repository and journal["repository_id"] == header["repository_id"]
                 and journal["workflow_sha"] == header["workflow_sha"] and journal["workflow_blob_sha"] == header["workflow_blob_sha"],
                 "Consumer journal differs from the trusted default workflow")
        if deferred:
            _require(member["source_run_id"] not in header["member_ids"], "Selected source is not deferred")
            title = generation.get("display_title")
            _require(isinstance(title, str) and re.fullmatch(rf"sensor={member['source_run_id']} action=(?:in_progress|completed)", title), "Deferred callback source hint differs from immutable journal")
            validate_journal_clock(journal, reader)
            source_endpoint = f"repos/{repository}/actions/runs/{member['source_run_id']}"
            before = reader.request(source_endpoint)
            source_jobs = reader.jobs(repository, member["source_run_id"])
            pull = reader.request(f"repos/{repository}/pulls/{member['pr_number']}")
            after = reader.request(source_endpoint)
            keys = ("id", "run_attempt", "workflow_id", "run_number", "name", "event", "path", "head_sha", "repository", "head_repository", "pull_requests")
            current_after = reader.request(f"repos/{repository}/pulls/{member['pr_number']}")
            _require(isinstance(before, dict) and isinstance(after, dict)
                     and before.get("status") in ACTIVE and after.get("status") in ACTIVE
                     and all(before.get(key) == after.get(key) for key in keys)
                     and isinstance(current_after, dict) and current_after == pull,
                     "Deferred original source/current PR observation changed")
            _validate_member_source_identity(member, before)
            _validate_member_source_identity(member, after)
            eligible = current_source_eligible(after, pull, repository, header["repository_id"], member["base_ref"])
            if eligible:
                _require(member_source_matches(member, before, pull) and member_source_matches(member, after, pull),
                         "Deferred eligible source/current PR body drift")
            started = source_latch_clock(after, source_jobs, now=time.time())
            _require(member["latch_started_at"] in (None, started), "Deferred source original clock drift")
            if started is not None:
                _require(time.time() < source_deadline(started, now=time.time()), "Deferred original latch expired")
        else:
            _require(member["source_run_id"] in header["member_ids"], "Cancelled consumer source is outside the exact cohort")
            bound = cohort["members_by_id"].get(member["source_run_id"])
            if bound is None:
                projection = load_cohort(cohort["artifact_id"], cohort["artifact_digest"], expected_owner=header["owner_dispatcher_run_id"],
                                         expected_segment=None, read_json=read_json, read_archive=read_archive, budget=budget,
                                         deadline=deadline, member_ids={member["source_run_id"]}, trusted_workflow_blob=cohort.get("trusted_workflow_blob"))
                bound = projection["members_by_id"][member["source_run_id"]]
            # null hintは現在時刻へ置換せず、元attemptのAwait stageから時計だけを復元する。
            member = _covered_member_clock(member, reader)
            bound = _covered_member_clock(bound, reader)
            _require(bound == member, "Cancelled consumer immutable source tuple drift")
        producer = reader.producer(repository, identifier, journal["workflow_sha"], journal["workflow_blob_sha"])
        _require(producer.get("status") == "completed" and producer.get("conclusion") in {"cancelled", "failure"}, "Consumer cancellation drift")
        keys = ("id", "run_attempt", "name", "head_sha", "head_branch", "path", "event", "workflow_id", "run_number", "repository", "status", "conclusion")
        _require(all(producer.get(key) == generation.get(key) for key in keys), "Consumer generation identity drift")
        jobs = reader.jobs(repository, identifier)
        by_name = {}
        for job in jobs:
            _require(job.get("name") not in by_name, "Ambiguous consumer job")
            by_name[job.get("name")] = job
        election = by_name.get(ELECTION_JOB)
        admission = by_name.get(ADMISSION_JOB)
        _require(isinstance(election, dict) and election.get("status") == "completed"
                 and election.get("conclusion") in {"cancelled", "skipped"}, "Consumer election is not positively cancelled")
        _require(isinstance(admission, dict) and admission.get("status") == "completed"
                 and admission.get("conclusion") in {"cancelled", "skipped", "failure"}, "Consumer admission is not positively terminal")
        steps = election.get("steps")
        _require(isinstance(steps, list) and all(isinstance(step, dict) for step in steps)
                 and not any(isinstance(step.get("name"), str) and step["name"].startswith(SELECTED_PREFIX)
                             and step.get("conclusion") == "success" for step in steps), "Cancelled selected owner is never a follower")
        for name in MUTATING_JOBS:
            job = by_name.get(name)
            _require(isinstance(job, dict) and job.get("status") == "completed" and job.get("conclusion") == "skipped",
                     "Mutation path is not positively skipped")
        return True
    except (CohortError, KeyError, TypeError):
        return False


def source_latch_clock(source, jobs, *, now):
    _require(isinstance(source, dict) and _integer(source.get("id")), "Invalid source clock run")
    matches = [job for job in jobs if job.get("name") == "KRR / PR governance review latch"]
    if not matches:
        _require(source.get("status") in ACTIVE and not jobs, "Source latch job missing")
        return None
    _require(len(matches) == 1, "Source latch job ambiguous")
    job = matches[0]
    _require(job.get("run_id") == source["id"] and ("run_attempt" not in job or type(job["run_attempt"]) is int and job["run_attempt"] == 1)
             and job.get("head_sha") == source.get("head_sha"), "Source latch attempt/head drift")
    steps = job.get("steps")
    _require(isinstance(steps, list) and all(isinstance(step, dict) for step in steps), "Source latch steps missing")
    if not steps:
        _require(source.get("status") in ACTIVE and job.get("status") in {"queued", "in_progress"}
                 and job.get("conclusion") is None, "Source terminal clock missing")
        return None
    matches = [step for step in steps if step.get("name") == "Await matching trusted governance Check Run"]
    _require(len(matches) == 1, "Source latch step ambiguous")
    step = matches[0]
    if step.get("started_at") is None:
        _require(source.get("status") in ACTIVE and step.get("status") in {"queued", "in_progress"}, "Source terminal clock missing")
        return None
    _require(type(step.get("number")) is int and step["number"] > 0 and step.get("status") in {"in_progress", "completed"}, "Source latch step identity")
    started = canonical_timestamp(step["started_at"])
    source_deadline(started, now=now)
    return started


def resolve_member_clock(member, *, read_json, budget, deadline):
    validate_member(member)
    reader = _Reader(read_json, None, budget, deadline)
    repo, identifier = member["repository"], member["source_run_id"]
    endpoint = f"repos/{repo}/actions/runs/{identifier}"
    source = reader.request(endpoint)
    jobs = reader.jobs(repo, identifier)
    after = reader.request(endpoint)
    keys = ("id", "run_attempt", "workflow_id", "run_number", "name", "event", "path", "head_sha", "repository", "head_repository", "pull_requests")
    _require(isinstance(source, dict) and isinstance(after, dict) and all(source.get(key) == after.get(key) for key in keys), "Original source clock identity drift")
    started = source_latch_clock(after, jobs, now=time.time())
    _require(started is not None, "Source latch is not admitted")
    _require(member["latch_started_at"] in (None, started), "Source original clock drift")
    normalized = dict(member)
    normalized["latch_started_at"] = started
    normalized["source_deadline_epoch"] = source_deadline(started, now=time.time())
    _require(time.time() < min(normalized["source_deadline_epoch"], deadline), "Source original deadline expired")
    return normalized


def _covered_member_clock(member, reader):
    """履歴ACKだけは元Awaitの時刻内successful完了を肯定証明する。"""
    repository, identifier = member["repository"], member["source_run_id"]
    before = reader.request(f"repos/{repository}/actions/runs/{identifier}")
    jobs = reader.jobs(repository, identifier)
    pull = reader.request(f"repos/{repository}/pulls/{member['pr_number']}")
    after = reader.request(f"repos/{repository}/actions/runs/{identifier}")
    started = source_latch_clock(after, jobs, now=time.time())
    _require(started is not None and member["latch_started_at"] in (None, started), "Historical source clock missing/drift")
    normalized = dict(member, latch_started_at=started, source_deadline_epoch=source_deadline(started, now=time.time()))
    _require(member_source_matches(normalized, before, pull) and member_source_matches(normalized, after, pull), "Historical source/current Ready PR binding drift")
    if time.time() >= normalized["source_deadline_epoch"]:
        _require(before.get("status") == after.get("status") == "completed"
                 and before.get("conclusion") == after.get("conclusion") == "success", "Expired active source is not an ACK")
        latch = [job for job in jobs if job.get("name") == "KRR / PR governance review latch"]
        steps = [step for step in latch[0]["steps"] if step.get("name") == "Await matching trusted governance Check Run"]
        _require(len(steps) == 1 and steps[0].get("status") == "completed" and steps[0].get("conclusion") == "success"
                 and _utc_epoch(started) <= _utc_epoch(steps[0].get("completed_at")) <= normalized["source_deadline_epoch"], "Historical ACK completed after original source clock")
    return normalized


def _utc_epoch(value):
    return datetime.fromisoformat(canonical_timestamp(value).replace("Z", "+00:00")).timestamp()


RECOVERY_PREFIX = "Record verified failed cohort recovery "


class RecoveryNotDrained(CohortError):
    pass


def recover_failed_owner(reader, repository, identifier, sha, blob, reference=None):
    reader.cache.pop(("producer", repository, identifier), None)
    producer = reader.producer(repository, identifier, sha, blob)
    _require((producer.get("status") in ACTIVE and producer.get("conclusion") is None
              or producer.get("status") == "completed" and producer.get("conclusion") == "failure")
             and repository_matches(producer.get("repository"), repository, producer["repository"]["id"]),
             "Recovery source is not an exact failed trusted owner")
    reader.cache.pop(("jobs", repository, identifier), None)
    jobs = reader.jobs(repository, identifier)
    by_name = {}
    for job in jobs:
        _require(job.get("name") not in by_name, "Ambiguous failed owner job")
        by_name[job.get("name")] = job
    listed = reader.request(f"repos/{repository}/actions/runs/{identifier}/artifacts?per_page=100&page=1")
    entries = listed.get("artifacts")
    _require(type(listed.get("total_count")) is int and isinstance(entries, list)
             and listed["total_count"] == len(entries) <= 100 and all(isinstance(item, dict) for item in entries)
             and len({item.get("id") for item in entries}) == len(entries), "Incomplete failed owner journal inventory")
    candidates = [item for item in entries if isinstance(item.get("name"), str) and item["name"].startswith("krr-governance-source-")]
    _require(len(candidates) == 1, "Failed owner source journal is missing or ambiguous")
    root_ref = {"artifact_id": candidates[0]["id"], "artifact_digest": candidates[0]["digest"]}
    journal, metadata = reader.artifact(root_ref, "source", repository, expected_producer=identifier)
    _require(journal["workflow_sha"] == sha and journal["workflow_blob_sha"] == blob
             and journal["repository_id"] == producer["repository"]["id"]
             and metadata["workflow_run"].get("head_sha") == sha, "Failed owner journal creator drift")
    validate_journal_clock(journal, reader)
    admission = by_name.get(ADMISSION_JOB)
    _require(isinstance(admission, dict) and admission.get("status") == "completed"
             and admission.get("conclusion") == "success" and admission.get("head_sha") == sha,
             "Failed recovery owner never successfully admitted a backend")
    election = by_name.get(ELECTION_JOB)
    _require(isinstance(election, dict) and election.get("status") == "completed"
             and isinstance(election.get("steps"), list), "Failed election evidence missing")
    stages = [step for step in election["steps"] if isinstance(step, dict) and isinstance(step.get("name"), str)
              and step["name"].startswith(SELECTED_PREFIX)]
    _require(len(stages) == 1, "Failed owner selection is missing or ambiguous")
    selected = stages[0]
    match = re.fullmatch(SELECTED_PREFIX + rf"owner={identifier} artifact=([1-9][0-9]*) digest=(sha256:[0-9a-f]{{64}})", selected["name"])
    _require(match is not None and selected.get("status") == "completed" and selected.get("conclusion") == "success", "Failed owner selection was not bound")
    header, _ = reader.artifact({"artifact_id": int(match[1]), "artifact_digest": match[2]}, "cohort", repository,
                                expected_producer=identifier, failed_owner=identifier)
    content = dict(header); content.pop("cohort_digest")
    _require(header["workflow_sha"] == sha and header["workflow_blob_sha"] == blob
             and header["repository_id"] == journal["repository_id"] and header["owner_run_attempt"] == 1
             and hashlib.sha256(canonical(content)).hexdigest() == header["cohort_digest"], "Failed selection header drift")
    root_ref = _ref(header["root_journal"])
    journal, _ = reader.artifact(root_ref, "source", repository)
    _require(journal["repository_id"] == header["repository_id"] and journal["root_deadline_epoch"] == header["root_deadline_epoch"], "Failed selection restarted original root")
    reader.producer(repository, journal["producer_run_id"], journal["workflow_sha"], journal["workflow_blob_sha"])
    validate_journal_clock(journal, reader)
    _require(reference is None or _ref(reference) == root_ref, "Recovery marker substituted original root journal")
    root = journal["root_deadline_epoch"]
    _require(time.time() < root, "Failed owner original root expired")
    reader.deadline = min(reader.deadline, root)
    if producer.get("status") != "completed":
        raise RecoveryNotDrained("Recovery predecessor has not completed failure yet")
    for name in MUTATING_JOBS:
        job = by_name.get(name)
        _require(isinstance(job, dict) and job.get("status") == "completed"
                 and job.get("conclusion") in {"success", "failure", "cancelled", "skipped", "timed_out", "action_required", "neutral"},
                 "Failed owner mutation job is not positively terminal")
    drain_owner_backend(reader, repository, producer, journal["repository_id"], sha)
    final = reader.request(f"repos/{repository}/actions/runs/{identifier}")
    _require(final == producer, "Failed owner recovery double-read drift")
    return {"owner": identifier, "journal": root_ref["artifact_id"], "digest": root_ref["artifact_digest"], "root_deadline_epoch": root}


def drain_owner_backend(reader, repository, producer, repository_id, sha):
    identifier = producer["id"]
    created = producer.get("created_at")
    _require(_utc_epoch(created) <= time.time(), "Failed owner creation timestamp missing/future")
    query = urllib.parse.urlencode({"created": ">=" + created, "per_page": 100, "page": 1})
    endpoint = f"repos/{repository}/actions/workflows/pr-governance-status-writer.yml/runs?{query}"
    snapshot = reader.request(endpoint)
    runs = snapshot.get("workflow_runs")
    _require(type(snapshot.get("total_count")) is int and isinstance(runs, list)
             and snapshot["total_count"] == len(runs) <= 100 and all(isinstance(run, dict) and _integer(run.get("id")) for run in runs)
             and len({run["id"] for run in runs}) == len(runs), "Incomplete failed backend drain inventory")
    for run in runs:
        title = run.get("display_title", "")
        if not isinstance(title, str) or not title.startswith(f"source={identifier} "):
            continue
        _require(re.fullmatch(rf"source={identifier} scope=(?:early|all) segment=[0-4]", title)
                 and run.get("event") == "workflow_dispatch" and run.get("run_attempt") == 1
                 and run.get("head_sha") == sha and run.get("head_branch") == producer.get("head_branch")
                 and run.get("path") in {".github/workflows/pr-governance-status-writer.yml", ".github/workflows/pr-governance-status-writer.yml@" + producer["head_branch"]}
                 and repository_matches(run.get("repository"), repository, repository_id), "Foreign failed owner backend")
        before = reader.request(f"repos/{repository}/actions/runs/{run['id']}")
        _require(before == run, "Failed backend drain listing drift")
        if before.get("status") != "completed":
            _require(before.get("status") in ACTIVE and before.get("conclusion") is None, "Invalid failed backend state")
            raise RecoveryNotDrained("Failed owner backend is still active")
        _require(before.get("conclusion") in {"success", "failure", "cancelled", "timed_out", "action_required", "neutral", "skipped"}, "Unknown failed backend terminal state")
        after = reader.request(f"repos/{repository}/actions/runs/{run['id']}")
        _require(after == before, "Failed backend terminal double-read drift")
    _require(reader.request(endpoint) == snapshot, "Failed backend inventory changed during drain")


def await_failed_owner_recovery(reader, repository, identifier, sha, blob):
    while time.time() < reader.deadline:
        try:
            return recover_failed_owner(reader, repository, identifier, sha, blob)
        except RecoveryNotDrained:
            time.sleep(min(30, max(0, reader.deadline - time.time())))
    raise CohortError("Failed owner original recovery deadline expired")


def recovery_marker(steps):
    candidates = [step for step in steps if isinstance(step.get("name"), str) and step["name"].startswith(RECOVERY_PREFIX)]
    _require(len(candidates) <= 1, "Ambiguous recovery native stage")
    if not candidates:
        return None
    step = candidates[0]
    match = re.fullmatch(RECOVERY_PREFIX + r"owner=([1-9][0-9]*) journal=([1-9][0-9]*) digest=(sha256:[0-9a-f]{64}) root=([1-9][0-9]*)", step["name"])
    _require(match is not None and step.get("status") == "completed" and step.get("conclusion") == "success", "Failed recovery native stage")
    return {"owner": int(match[1]), "journal": int(match[2]), "digest": match[3], "root_deadline_epoch": int(match[4])}


def validate_journal_clock(journal, reader):
    _require(type(journal["producer_run_attempt"]) is int and journal["producer_run_attempt"] == 1
             and _integer(journal["root_deadline_epoch"]), "Invalid journal original clock")
    jobs = reader.jobs(journal["repository"], journal["producer_run_id"])
    preflight = [job for job in jobs if job.get("name") == "Preflight workflow_run governance source"]
    _require(len(preflight) == 1 and preflight[0].get("status") == "completed" and preflight[0].get("conclusion") == "success",
             "Journal preflight creator did not succeed")
    steps = preflight[0].get("steps")
    _require(isinstance(steps, list) and all(isinstance(step, dict) for step in steps), "Preflight stages missing")
    scopes = [step for step in steps if step.get("name") == "Exclude unavailable fork sources before dispatcher lock"]
    _require(len(scopes) == 1 and scopes[0].get("status") == "completed" and scopes[0].get("conclusion") == "success",
             "Original root admission stage is ambiguous")
    recovery = recovery_marker(steps)
    if recovery is not None:
        _require(recovery["owner"] != journal["producer_run_id"] and recovery["root_deadline_epoch"] == journal["root_deadline_epoch"], "Recovery root restarted or cycle")
        active = reader.cache.setdefault("recovery_clock_active", set())
        _require(journal["producer_run_id"] not in active, "Recovery clock lineage cycle")
        active.add(journal["producer_run_id"])
        try:
            proof = recover_failed_owner(reader, journal["repository"], recovery["owner"], journal["workflow_sha"], journal["workflow_blob_sha"],
                                         {"artifact_id": recovery["journal"], "artifact_digest": recovery["digest"]})
            _require(proof == recovery, "Recovery native stage differs from original journal")
        finally:
            active.remove(journal["producer_run_id"])
        reader.cache.setdefault("recovered_predecessor_ids", set()).add(recovery["owner"])
        return
    origin = journal["root_deadline_epoch"] - 21000
    _require(_utc_epoch(scopes[0].get("started_at")) <= origin <= _utc_epoch(scopes[0].get("completed_at")) <= time.time(),
             "Root clock restarted/future/outside creator stage")


def cohort_known_producer_ids(cohort, *, read_json, read_archive, budget, deadline):
    """Indexは候補を限定するだけで、取消consumerの免除authorityではない。"""
    header = cohort["header"]
    reader = _Reader(read_json, read_archive, budget, deadline, cohort.get("trusted_workflow_blob"))
    source_ids, producers = [], {}
    for index, reference in enumerate(header["alias_pages"]):
        page, _ = reader.artifact(reference, "aliases", header["repository"], expected_producer=header["owner_dispatcher_run_id"])
        entries = page["entries"]
        expected = header["member_ids"][index * 50:(index + 1) * 50]
        _require(isinstance(entries, list) and len(entries) == len(expected), "Incomplete alias reference page")
        for entry in entries:
            _require(isinstance(entry, list) and len(entry) == 2 and _integer(entry[0]) and isinstance(entry[1], list)
                     and 1 <= len(entry[1]) <= 2 and all(_integer(identifier) for identifier in entry[1])
                     and entry[1] == sorted(set(entry[1])), "Malformed bounded source aliases")
            source, aliases = entry
            source_ids.append(source)
            for alias in aliases:
                _require(alias not in producers, "Duplicate/foreign producer alias")
                producers[alias] = source
        _require([entry[0] for entry in entries] == expected, "Alias page/source index mismatch")
    _require(source_ids == header["member_ids"] and len(producers) <= 2 * MAX_MEMBERS, "Incomplete bounded alias union")
    for identifier in cohort.get("recovered_predecessor_ids", []):
        _require(_integer(identifier) and identifier != header["owner_dispatcher_run_id"], "Invalid recovered predecessor index")
        producers.setdefault(identifier, None)
    return producers


class ReadBudget:
    def __init__(self, limit):
        self.limit = limit
        self.used = 0

    def charge(self):
        _require(self.used < self.limit, "Cohort role read budget exhausted")
        self.used += 1


class _Transport:
    def __init__(self, token, budget):
        _require(isinstance(token, str) and bool(token), "Read credentials unavailable")
        self.token, self.budget, self.etags = token, budget, {}
        self.primary_reads = 0

    def json(self, endpoint, *, timeout):
        import subprocess
        _require(type(timeout) in {int, float} and math.isfinite(timeout) and 0 < timeout <= 20, "Invalid bounded JSON transport timeout")
        _require(isinstance(endpoint, str) and endpoint.startswith("repos/") and "\n" not in endpoint, "Invalid read endpoint")
        args = ["gh", "api", "--hostname", "github.com", "--include", endpoint]
        prior = self.etags.get(endpoint)
        if prior is not None:
            args.extend(["-H", "If-None-Match: " + prior[0]])
        environment = dict(os.environ, GH_TOKEN=self.token)
        try:
            response = subprocess.run(args, capture_output=True, text=True, check=False, timeout=timeout, env=environment)
        except subprocess.TimeoutExpired:
            raise CohortError("Cohort JSON read timed out") from None
        raw = response.stdout.replace("\r\n", "\n")
        headers, separator, data = raw.partition("\n\n")
        status = re.match(r"HTTP/\S+ ([0-9]{3})(?: |$)", headers)
        _require(separator and status is not None, "Cohort read transport metadata invalid")
        if status[1] == "304":
            etags = re.findall(r'(?im)^etag:\s*(W/"[^"\r\n]+"|"[^"\r\n]+")\s*$', headers)
            _require(response.returncode in {0, 1} and prior is not None and not data
                     and len(etags) == 1 and etags[0] == prior[0], "Unbound conditional response")
            return json.loads(json.dumps(prior[1]))
        self.primary_reads += 1
        _require(response.returncode == 0 and status[1] == "200", "Cohort JSON read failed closed")
        try:
            value = json.loads(data)
        except ValueError:
            raise CohortError("Invalid cohort JSON response") from None
        _require(isinstance(value, dict), "Cohort read object unavailable")
        etags = re.findall(r'(?im)^etag:\s*(W/"[^"\r\n]+"|"[^"\r\n]+")\s*$', headers)
        _require(len(etags) <= 1, "Ambiguous conditional read metadata")
        if etags:
            self.etags[endpoint] = (etags[0], value)
        else:
            self.etags.pop(endpoint, None)
        return value

    def archive(self, endpoint, *, timeout):
        _require(type(timeout) in {int, float} and math.isfinite(timeout) and 0 < timeout <= 20, "Invalid bounded archive transport timeout")
        # API archive GETはreaderが計上済み。署名済storage GETも同roleのattempt枠へ計上する。
        self.budget.charge()
        return read_artifact_archive(endpoint, self.token, timeout)


def _write_outputs(values):
    destination = os.environ.get("GITHUB_OUTPUT")
    _require(isinstance(destination, str) and bool(destination), "Output destination unavailable")
    with open(destination, "a", encoding="utf-8") as output:
        for key, value in values.items():
            text = str(value)
            _require(re.fullmatch(r"[a-z_][a-z_0-9]*", key) and "\n" not in text and "\r" not in text, "Invalid output wire")
            output.write(key + "=" + text + "\n")


def _write_binding(directory, payload):
    from pathlib import Path
    path = Path(directory)
    _require(not path.is_symlink(), "Binding directory is a symlink")
    path.mkdir(parents=True, exist_ok=True, mode=0o700)
    target = path / "binding.json"
    _require(not target.exists() and not target.is_symlink(), "Immutable binding path already exists")
    target.write_bytes(canonical(payload))
    target.chmod(0o600)


def _decode_journal_stage(step):
    _require(isinstance(step, dict) and step.get("status") == "completed" and step.get("conclusion") == "success", "Journal creator stage incomplete")
    text = step.get("name")
    _require(isinstance(text, str) and text.startswith(JOURNAL_PREFIX) and len(text.encode("utf-8")) <= MAX_BYTES, "Journal creator wire invalid")
    try:
        data = base64.b64decode(text[len(JOURNAL_PREFIX):], validate=True)
    except (ValueError, TypeError):
        raise CohortError("Journal creator wire invalid") from None
    return decode_payload(data, JOURNAL_FIELDS)


def _producer_context(reader):
    repository = os.environ.get("GITHUB_REPOSITORY", "")
    identifier = os.environ.get("GITHUB_RUN_ID", "")
    sha = os.environ.get("WORKFLOW_SHA", "")
    branch = os.environ.get("DEFAULT_BRANCH", "")
    _require(_repository_name(repository) and re.fullmatch(r"[1-9][0-9]*", identifier) and _hex(sha, 40)
             and os.environ.get("GITHUB_RUN_ATTEMPT") == "1"
             and os.environ.get("WORKFLOW_REF") == f"{repository}/.github/workflows/pr-governance.yml@refs/heads/{branch}", "Untrusted cohort runtime identity")
    record = reader.request(f"repos/{repository}")
    _require(record.get("full_name") == repository and _integer(record.get("id")) and record.get("default_branch") == branch, "Runtime repository identity drift")
    contents = reader.request(f"repos/{repository}/contents/.github/workflows/pr-governance.yml?ref={sha}")
    _require(contents.get("encoding") == "base64" and _hex(contents.get("sha"), 40) and isinstance(contents.get("content"), str), "Runtime workflow content unavailable")
    try:
        code = base64.b64decode(contents["content"].replace("\n", ""), validate=True)
    except ValueError:
        raise CohortError("Runtime workflow bytes invalid") from None
    _require(type(contents.get("size")) is int and contents["size"] == len(code)
             and hashlib.sha1(b"blob " + str(len(code)).encode() + b"\0" + code).hexdigest() == contents["sha"], "Runtime workflow Git blob drift")
    return repository, record["id"], int(identifier), sha, contents["sha"]


def produce_journal(reader):
    repository, repository_id, identifier, sha, blob = _producer_context(reader)
    root = os.environ.get("COHORT_ORIGINAL_ROOT_DEADLINE", "")
    _require(re.fullmatch(r"[1-9][0-9]*", root) and time.time() < int(root), "Original admission root expired")
    try:
        member = json.loads(os.environ.get("COHORT_MEMBER_HINT", "null"))
    except ValueError:
        raise CohortError("Invalid preflight member hint") from None
    if member is not None:
        validate_member(member)
        source = reader.request(f"repos/{repository}/actions/runs/{member['source_run_id']}")
        pull = reader.request(f"repos/{repository}/pulls/{member['pr_number']}")
        _require(member_source_matches(member, source, pull), "Preflight source/current PR drift")
        started = source_latch_clock(source, reader.jobs(repository, member["source_run_id"]), now=time.time())
        member = dict(member, latch_started_at=started, source_deadline_epoch=source_deadline(started, now=time.time()))
        after = reader.request(f"repos/{repository}/actions/runs/{member['source_run_id']}")
        final = reader.request(f"repos/{repository}/pulls/{member['pr_number']}")
        _require(member_source_matches(member, after, final) and source.get("id") == after.get("id"), "Journal source double-read drift")
        name = f"krr-governance-source-{member['source_run_id']}-attempt-1"
    else:
        _require(os.environ.get("COHORT_SENSOR_SOURCE", "false") != "true" or os.environ.get("COHORT_VALID") != "true", "Valid sensor has no strict source journal")
        name = f"krr-governance-source-driver-{identifier}-attempt-1"
    journal = {"v": 1, "kind": "source", "producer_run_id": identifier, "producer_run_attempt": 1,
               "repository": repository, "repository_id": repository_id, "workflow_sha": sha,
               "workflow_blob_sha": blob, "root_deadline_epoch": int(root), "member": member}
    data = canonical(journal)
    binding = base64.b64encode(data).decode("ascii")
    _require(len((JOURNAL_PREFIX + binding).encode()) <= MAX_BYTES, "Journal stage exceeds existing marker bound")
    _write_binding("cohort-journal", journal)
    _write_outputs({"artifact_name": name, "payload_sha256": hashlib.sha256(data).hexdigest(), "binding_base64": binding})


def current_source_eligible(source, current_pr, repository, repository_id, branch):
    """対象外は現在のPRを肯定観測した場合だけ除外し、観測不備は拒否する。"""
    _require(isinstance(current_pr, dict) and isinstance(source, dict), "Current source PR observation unavailable")
    pulls = source.get("pull_requests")
    _require(isinstance(pulls, list) and len(pulls) == 1 and isinstance(pulls[0], dict), "Current source PR binding ambiguous")
    original = pulls[0]
    original_base, original_head = original.get("base"), original.get("head")
    _require(_integer(original.get("number")) and isinstance(original_base, dict) and isinstance(original_head, dict)
             and _hex(original_base.get("sha"), 40) and _hex(original_head.get("sha"), 40)
             and source.get("head_sha") == original_head["sha"], "Source embedded PR/head identity malformed")
    _require(_integer(current_pr.get("number")) and current_pr["number"] == original.get("number")
             and current_pr.get("state") in ("open", "closed") and type(current_pr.get("draft")) is bool,
             "Current source PR state/identity malformed")
    base, head = current_pr.get("base"), current_pr.get("head")
    _require(isinstance(base, dict) and isinstance(head, dict)
             and isinstance(base.get("ref"), str) and bool(base["ref"]) and not any(c.isspace() for c in base["ref"])
             and _hex(base.get("sha"), 40) and _hex(head.get("sha"), 40), "Current source PR refs malformed")
    for repo in (base.get("repo"), head.get("repo")):
        _require(isinstance(repo, dict) and _integer(repo.get("id"))
                 and (_repository_name(repo.get("full_name"))
                      or isinstance(repo.get("name"), str) and bool(repo["name"])
                      and isinstance(repo.get("url"), str) and repo["url"].startswith("https://api.github.com/repos/")),
                 "Current source PR repository malformed")
        if repo["id"] == repository_id or repo.get("full_name") == repository:
            _require(repository_matches(repo, repository, repository_id), "Current source PR repository identity contradicts itself")
    _require("body" in current_pr, "Current source PR body malformed")
    body = "" if current_pr["body"] is None else current_pr["body"]
    _require(isinstance(body, str) and "\0" not in body, "Current source PR body malformed")
    try:
        body.encode("utf-8", "strict")
    except UnicodeError as error:
        raise CohortError("Current source PR body malformed") from error
    # 本文なしのPRはIssue契約を持てないため、journal照合へ進めず対象外とする。
    if (not body or current_pr["state"] != "open" or current_pr["draft"]
            or not repository_matches(base["repo"], repository, repository_id)
            or not repository_matches(head["repo"], repository, repository_id)
            or base["ref"] != branch or base["sha"] != original["base"]["sha"]
            or head["sha"] != original["head"]["sha"] or head["sha"] != source.get("head_sha")):
        return False

    return True


def active_source_snapshot(reader, repository, repository_id, branch):
    runs = {}
    identities, current_prs, observed = {}, {}, set()
    for status in ("requested", "queued", "waiting", "pending", "in_progress"):
        endpoint = f"repos/{repository}/actions/workflows/pr-governance-review-events.yml/runs?status={status}&per_page=100&page="
        for attempt in range(4):
            first = reader.request(endpoint + "1")
            _require(isinstance(first, dict), "Active sensor inventory observation malformed")
            total = first.get("total_count")
            _require(type(total) is int and 0 <= total <= 600, "Incomplete active sensor inventory")
            entries, seen = [], set()
            for page in range(1, max(1, (total + 99) // 100) + 1):
                response = first if page == 1 else reader.request(endpoint + str(page))
                _require(isinstance(response, dict), "Active sensor page observation malformed")
                items = response.get("workflow_runs")
                _require(type(response.get("total_count")) is int and response["total_count"] == total and isinstance(items, list)
                         and len(items) == min(100, max(0, total - (page - 1) * 100)), "Incomplete active sensor page")
                for item in items:
                    _require(isinstance(item, dict) and _integer(item.get("id")) and item["id"] not in seen, "Duplicate active sensor inventory")
                    seen.add(item["id"])
                entries.extend(items)
            anchor = reader.request(endpoint + "1")
            _require(isinstance(anchor, dict), "Active sensor anchor observation malformed")
            try:
                # Pythonの等値比較はbool/intやfloat/intの型変化を見逃すため、JSONの型を保持する。
                stable = json.dumps(anchor, sort_keys=True, allow_nan=False) == json.dumps(first, sort_keys=True, allow_nan=False)
            except (TypeError, ValueError) as error:
                raise CohortError("Invalid active sensor anchor observation") from error
            if stable:
                break
        else:
            raise CohortError("Active sensor inventory did not stabilize")
        page_ids = set()
        for run in entries:
            _require(isinstance(run, dict) and _integer(run.get("id")) and run["id"] not in page_ids
                     and run.get("status") in ACTIVE and run.get("conclusion") is None
                     and run.get("name") == "PR governance review sensor" and run.get("event") in SENSOR_EVENTS
                     and type(run.get("run_attempt")) is int and run["run_attempt"] == 1
                     and _integer(run.get("workflow_id")) and _integer(run.get("run_number"))
                     and _hex(run.get("head_sha"), 40)
                     and run.get("path") in {".github/workflows/pr-governance-review-events.yml",
                                            ".github/workflows/pr-governance-review-events.yml@" + branch,
                                            ".github/workflows/pr-governance-review-events.yml@refs/heads/" + branch}
                     and repository_matches(run.get("repository"), repository, repository_id), "Invalid active sensor identity")
            page_ids.add(run["id"])
            observed.add(run["id"])
            _require(len(observed) <= 600, "Aggregate active sensor inventory exceeds existing page bound")
            pulls = run.get("pull_requests")
            _require(isinstance(pulls, list) and len(pulls) == 1 and isinstance(pulls[0], dict), "Ambiguous active sensor PR")
            pull = pulls[0]
            base, head = pull.get("base"), pull.get("head")
            _require(_integer(pull.get("number")) and isinstance(base, dict) and isinstance(head, dict)
                     and isinstance(base.get("ref"), str) and bool(base["ref"]) and not any(c.isspace() for c in base["ref"])
                     and _hex(base.get("sha"), 40) and _hex(head.get("sha"), 40)
                     and run["head_sha"] == head["sha"]
                     and repository_matches(base.get("repo"), repository, repository_id), "Active sensor PR binding invalid")
            for source_repo in (head.get("repo"), run.get("head_repository")):
                _require(isinstance(source_repo, dict) and _integer(source_repo.get("id"))
                         and (_repository_name(source_repo.get("full_name"))
                              or isinstance(source_repo.get("name"), str) and bool(source_repo["name"])
                              and isinstance(source_repo.get("url"), str) and source_repo["url"].startswith("https://api.github.com/repos/")),
                         "Active sensor head repository observation malformed")
            _require(head["repo"]["id"] == run["head_repository"]["id"], "Active sensor head repository identity contradicts itself")
            identity = (run["id"], run["run_attempt"], run["workflow_id"], run["run_number"], run["event"], run["path"],
                        run["head_sha"], pull["number"], base["ref"], base["sha"], head["sha"],
                        head["repo"]["id"], head["repo"].get("full_name"), run["head_repository"].get("full_name"))
            _require(run["id"] not in identities or identities[run["id"]] == identity, "Cross-partition sensor identity drift")
            identities[run["id"]] = identity
            local = repository_matches(head["repo"], repository, repository_id) and repository_matches(run["head_repository"], repository, repository_id)
            if not local or base["ref"] != branch:
                continue
            number = pull["number"]
            if number not in current_prs:
                current_prs[number] = reader.request(f"repos/{repository}/pulls/{number}")
            if current_source_eligible(run, current_prs[number], repository, repository_id, branch):
                runs[run["id"]] = run
    return runs


def _journal_from_metadata(reader, metadata, source_id, repository, repository_id, sha, blob):
    _require(isinstance(metadata, dict) and _integer(metadata.get("id")) and _digest(metadata.get("digest"))
             and metadata.get("expired") is False and metadata.get("name") == f"krr-governance-source-{source_id}-attempt-1"
             and type(metadata.get("size_in_bytes")) is int and 0 < metadata["size_in_bytes"] <= MAX_BYTES, "Source journal metadata invalid")
    binding = metadata.get("workflow_run")
    _require(isinstance(binding, dict) and _integer(binding.get("id")) and binding.get("repository_id") == repository_id
             and binding.get("head_repository_id") == repository_id and binding.get("head_sha") == sha, "Source journal immutable creator binding")
    producer = reader.producer(repository, binding["id"], sha, blob)
    _require(repository_matches(producer.get("repository"), repository, repository_id), "Source journal creator repository drift")
    jobs = reader.jobs(repository, binding["id"])
    publishers = [job for job in jobs if job.get("name") == PRODUCER_JOB]
    _require(len(publishers) == 1 and publishers[0].get("status") == "completed" and publishers[0].get("conclusion") == "success", "Source journal publisher is not complete")
    steps = publishers[0].get("steps")
    _require(isinstance(steps, list) and all(isinstance(step, dict) for step in steps), "Source journal publisher stages missing")
    bindings = [step for step in steps if isinstance(step.get("name"), str) and step["name"].startswith(JOURNAL_PREFIX)]
    _require(len(bindings) == 1, "Source journal publisher binding ambiguous")
    journal = _decode_journal_stage(bindings[0])
    _require(journal["producer_run_id"] == binding["id"] and journal["repository"] == repository
             and journal["repository_id"] == repository_id and journal["workflow_sha"] == sha and journal["workflow_blob_sha"] == blob,
             "Source journal canonical binding drift")
    member = validate_member(journal["member"])
    _require(member["source_run_id"] == source_id, "Foreign source journal")
    validate_journal_clock(journal, reader)
    upload = [step for step in steps if step.get("name") == "Upload immutable " + metadata["name"]]
    _require(len(upload) == 1 and upload[0].get("status") == "completed" and upload[0].get("conclusion") == "success", "Source journal upload stage missing")
    after = reader.request(f"repos/{repository}/actions/artifacts/{metadata['id']}")
    keys = ("id", "name", "digest", "expired", "size_in_bytes", "workflow_run")
    _require(isinstance(after, dict) and all(after.get(key) == metadata.get(key) for key in keys), "Source journal metadata changed")
    return journal, producer


def select_cohort(reader):
    from pathlib import Path
    repository, repository_id, owner, sha, blob = _producer_context(reader)
    branch = os.environ["DEFAULT_BRANCH"]
    sources = active_source_snapshot(reader, repository, repository_id, branch)
    own_ref = {"artifact_id": int(os.environ["COHORT_OWN_JOURNAL_ID"]), "artifact_digest": os.environ["COHORT_OWN_JOURNAL_DIGEST"]}
    own_journal, _ = reader.artifact(own_ref, "source", repository, expected_producer=owner)
    validate_journal_clock(own_journal, reader)
    members, source_refs, aliases, roots, current_prs = {}, {}, {}, [(own_journal["root_deadline_epoch"], own_ref)], {}
    candidates = []
    for identifier, source in sorted(sources.items()):
        number = source["pull_requests"][0]["number"]
        if number not in current_prs:
            current_prs[number] = reader.request(f"repos/{repository}/pulls/{number}")
        if not current_source_eligible(source, current_prs[number], repository, repository_id, branch):
            continue
        # 未開始と超過分のjournalは変更せず、元Await期限が早いmemberを先に処理する。
        admitted_at = source_latch_clock(source, reader.jobs(repository, identifier), now=time.time())
        if admitted_at is not None:
            candidates.append((source_deadline(admitted_at, now=time.time()), identifier, source))
    for _, identifier, source in sorted(candidates, key=lambda item: item[:2])[:MAX_MEMBERS]:
        name = f"krr-governance-source-{identifier}-attempt-1"
        listing = reader.request(f"repos/{repository}/actions/artifacts?name={name}&per_page=100&page=1")
        entries, total = listing.get("artifacts"), listing.get("total_count")
        _require(type(total) is int and 1 <= total <= 2 and isinstance(entries, list) and len(entries) == total
                 and all(isinstance(entry, dict) for entry in entries)
                 and len({entry.get("id") for entry in entries}) == len(entries), "Missing/ambiguous bounded source journals")
        verified = []
        for metadata in entries:
            journal, producer = _journal_from_metadata(reader, metadata, identifier, repository, repository_id, sha, blob)
            member = journal["member"]
            number = member["pr_number"]
            if number not in current_prs:
                current_prs[number] = reader.request(f"repos/{repository}/pulls/{number}")
            if member_source_matches(member, source, current_prs[number]):
                verified.append((producer["run_number"], producer["id"], metadata, journal))
        _require(bool(verified), "No journal binds the current source/Ready PR")
        _, creator, metadata, journal = max(verified, key=lambda item: item[:2])
        member = journal["member"]
        if member["latch_started_at"] is None:
            member = resolve_member_clock(member, read_json=reader.read_json, budget=reader.budget, deadline=reader.deadline)
        _require(time.time() < member["source_deadline_epoch"], "Source original latch expired before election")
        members[identifier] = member
        source_refs[identifier] = [identifier, creator, metadata["id"], metadata["digest"]]
        aliases[identifier] = sorted({item[1] for item in verified})
        roots.append((journal["root_deadline_epoch"], {"artifact_id": metadata["id"], "artifact_digest": metadata["digest"]}))
    root, root_ref = min(roots, key=lambda entry: (entry[0], entry[1]["artifact_id"]))
    if reader.cache.get("recovered_predecessor_ids"):
        _require(root == own_journal["root_deadline_epoch"], "Recovery selection cannot substitute an earlier original root")
        root_ref = own_ref
    _require(time.time() < root, "Original cohort root expired")
    identifiers = sorted(members)
    batches = build_batches(members) if members else []
    segment_by_id = {identifier: batch["segment"] for batch in batches for _, ids in batch["target_members"] for identifier in ids}
    outputs = {"member_page_count": (len(identifiers) + 49) // 50, "batch_count": len(batches), "root_deadline_epoch": root}
    for index in range(0, len(identifiers), 50):
        segment = index // 50 + 1
        subset = identifiers[index:index + 50]
        for kind, entries in (("members", [source_refs[identifier] for identifier in subset]), ("aliases", [[identifier, aliases[identifier]] for identifier in subset])):
            payload = {"v": 1, "kind": kind, "owner_dispatcher_run_id": owner, "entries": entries}
            _write_binding(f"cohort-selection/{kind}-{segment}", payload)
            outputs[f"{kind}_{segment}_sha256"] = hashlib.sha256(canonical(payload)).hexdigest()
    for batch in batches:
        payload = dict(batch, v=1, kind="batch", owner_dispatcher_run_id=owner)
        _write_binding(f"cohort-selection/batch-{batch['segment']}", payload)
        outputs[f"batch_{batch['segment']}_sha256"] = hashlib.sha256(canonical(payload)).hexdigest()
    state = {"owner": owner, "repository": repository, "repository_id": repository_id, "workflow_sha": sha,
             "workflow_blob_sha": blob, "root_deadline_epoch": root, "root_journal": root_ref,
             "member_ids": identifiers, "member_segments": "".join(str(segment_by_id[identifier]) for identifier in identifiers),
             "member_page_count": outputs["member_page_count"], "batch_count": len(batches), "members": members}
    Path("cohort-selection").mkdir(mode=0o700, exist_ok=True)
    Path("cohort-selection/state.json").write_text(json.dumps(state, sort_keys=True, separators=(",", ":")), encoding="utf-8")
    _write_outputs(outputs)


def build_cohort_header():
    from pathlib import Path
    path = Path("cohort-selection/state.json")
    _require(path.is_file() and not path.is_symlink() and path.stat().st_uid == os.getuid() and path.stat().st_size <= 300000,
             "Local election state invalid")
    state = json.loads(path.read_text(encoding="utf-8"))
    def native_ref(kind, index):
        identifier = os.environ.get(f"COHORT_{kind.upper()}_{index}_ID", "")
        digest = os.environ.get(f"COHORT_{kind.upper()}_{index}_DIGEST", "")
        _require(re.fullmatch(r"[1-9][0-9]*", identifier) and _hex(digest, 64), "Native immutable upload output unavailable")
        return {"artifact_id": int(identifier), "artifact_digest": "sha256:" + digest}
    header = {"v": 1, "kind": "cohort", "owner_dispatcher_run_id": state["owner"], "owner_run_attempt": 1,
              "repository": state["repository"], "repository_id": state["repository_id"], "workflow_sha": state["workflow_sha"],
              "workflow_blob_sha": state["workflow_blob_sha"], "root_deadline_epoch": state["root_deadline_epoch"],
              "member_ids": state["member_ids"], "member_segments": state["member_segments"], "root_journal": state["root_journal"],
              "member_pages": [native_ref("members", index) for index in range(1, state["member_page_count"] + 1)],
              "alias_pages": [native_ref("aliases", index) for index in range(1, state["member_page_count"] + 1)],
              "batches": [native_ref("batch", index) for index in range(1, state["batch_count"] + 1)]}
    header["cohort_digest"] = hashlib.sha256(canonical(header)).hexdigest()
    _write_binding("cohort-header", header)
    _write_outputs({"payload_sha256": hashlib.sha256(canonical(header)).hexdigest(), "cohort_digest": header["cohort_digest"],
                    "root_deadline_epoch": header["root_deadline_epoch"]})


def _selected_header(reader, repository, owner, jobs):
    elections = [job for job in jobs if job.get("name") == ELECTION_JOB]
    _require(len(elections) == 1, "Election job is missing or ambiguous")
    election = elections[0]
    if election.get("status") == "completed" and election.get("conclusion") != "success":
        raise CohortError("Election was replaced before backend admission")
    steps = election.get("steps")
    if not isinstance(steps, list):
        return None
    selected = []
    pattern = re.compile(SELECTED_PREFIX + rf"owner={owner} artifact=([1-9][0-9]*) digest=(sha256:[0-9a-f]{{64}})")
    for step in steps:
        match = pattern.fullmatch(step.get("name", "")) if isinstance(step, dict) else None
        if match and step.get("status") == "completed" and step.get("conclusion") == "success":
            selected.append(match)
    _require(len(selected) <= 1, "Election selected more than one cohort")
    if not selected:
        return None
    _require(election.get("status") == "in_progress" and election.get("conclusion") is None, "Selected election lease is not active")
    reference = {"artifact_id": int(selected[0][1]), "artifact_digest": selected[0][2]}
    header, _ = reader.artifact(reference, "cohort", repository, expected_producer=owner)
    _require(header["repository"] == repository and type(header["owner_run_attempt"]) is int and header["owner_run_attempt"] == 1
             and _integer(header["root_deadline_epoch"]) and time.time() < header["root_deadline_epoch"], "Selected cohort header clock/identity")
    record = dict(header)
    record.pop("cohort_digest")
    _require(_hex(header["cohort_digest"], 64) and hashlib.sha256(canonical(record)).hexdigest() == header["cohort_digest"], "Selected cohort canonical digest")
    producer = reader.producer(repository, owner, header["workflow_sha"], header["workflow_blob_sha"])
    _require(repository_matches(producer.get("repository"), repository, header["repository_id"]), "Selected owner repository binding")
    root, _ = reader.artifact(header["root_journal"], "source", repository)
    validate_journal_clock(root, reader)
    _require(root["root_deadline_epoch"] == header["root_deadline_epoch"], "Selected owner restarted the root clock")
    return header, reference


def admit_backend(reader):
    repository = os.environ["GITHUB_REPOSITORY"]
    owner = int(os.environ["GITHUB_RUN_ID"])
    while time.time() < reader.deadline:
        reader.cache.pop(("jobs", repository, owner), None)
        selected = _selected_header(reader, repository, owner, reader.jobs(repository, owner))
        if selected is not None:
            header, reference = selected
            _write_outputs({"owner": "true", "cohort_artifact_id": reference["artifact_id"],
                            "cohort_artifact_digest": reference["artifact_digest"], "cohort_digest": header["cohort_digest"],
                            "root_deadline_epoch": header["root_deadline_epoch"], "member_count": len(header["member_ids"]),
                            "batch_count": len(header["batches"]), "source_ids": json.dumps(header["member_ids"], separators=(",", ":"))})
            return
        time.sleep(min(5, max(0, reader.deadline - time.time())))
    raise CohortError("Original cohort admission deadline expired")


def await_handoff(reader):
    repository, owner = os.environ["GITHUB_REPOSITORY"], int(os.environ["GITHUB_RUN_ID"])
    artifact = os.environ.get("COHORT_ARTIFACT_ID", "")
    digest = os.environ.get("COHORT_ARTIFACT_DIGEST", "")
    _require(re.fullmatch(r"[1-9][0-9]*", artifact) and _digest(digest), "Handoff immutable artifact binding")
    terminal_jobs = {"Service bound cohort review sources", "Preserve pending review sensor before direct dispatch",
                     "Preserve admitted sources before obsolete heavy preemption"}
    while time.time() < reader.deadline:
        reader.cache.pop(("jobs", repository, owner), None)
        jobs = reader.jobs(repository, owner)
        by_name = {}
        for job in jobs:
            _require(job.get("name") not in by_name, "Ambiguous handoff job")
            by_name[job.get("name")] = job
        observed = [by_name.get(name) for name in terminal_jobs]
        if all(isinstance(job, dict) and job.get("status") == "completed" and job.get("conclusion") in {"success", "skipped"} for job in observed) and by_name["Preserve admitted sources before obsolete heavy preemption"].get("conclusion") == "success":
            _require(all(job.get("conclusion") in {"success", "skipped"} for job in observed)
                     and by_name["Preserve admitted sources before obsolete heavy preemption"].get("conclusion") == "success",
                     "Cohort service/strict sensor wait/backend drain did not succeed")
            _write_outputs({"owner": owner, "cohort_artifact_id": artifact, "cohort_artifact_digest": digest})
            return
        failed = any(isinstance(job, dict) and job.get("status") == "completed" and job.get("conclusion") not in {"success", "skipped"} for job in observed)
        failed |= all(isinstance(job, dict) and job.get("status") == "completed" for job in observed) and by_name["Preserve admitted sources before obsolete heavy preemption"].get("conclusion") != "success"
        if failed:
            _, repository_id, identifier, sha, blob = _producer_context(reader)
            _require(identifier == owner, "Failure handoff owner drift")
            producer = reader.producer(repository, owner, sha, blob)
            try:
                drain_owner_backend(reader, repository, producer, repository_id, sha)
            except RecoveryNotDrained:
                time.sleep(min(30, max(0, reader.deadline - time.time())))
                continue
            raise CohortError("Cohort backend failed after a verified terminal drain")
        time.sleep(min(30, max(0, reader.deadline - time.time())))
    raise CohortError("Original cohort handoff deadline expired")


COMPLETED_NOOP_PREFIX = "Record verified completed review sensor callback no-op source="


def completed_sensor_noop(source, pull, *, reader, workflow_sha, workflow_blob):
    repository = os.environ["GITHUB_REPOSITORY"]
    _require(isinstance(source, dict) and source.get("status") == "completed" and source.get("conclusion") == "success"
             and _integer(source.get("id")), "Completed callback is not successful")
    jobs = reader.jobs(repository, source["id"])
    started = source_latch_clock(source, jobs, now=time.time())
    _require(started is not None, "Completed callback lacks original Await clock")
    deadline = source_deadline(started, now=time.time())
    _require(time.time() < min(deadline, reader.deadline), "Completed callback original source expired")
    latch = [job for job in jobs if job.get("name") == "KRR / PR governance review latch"]
    steps = [step for step in latch[0]["steps"] if step.get("name") == "Await matching trusted governance Check Run"]
    _require(latch[0].get("status") == "completed" and latch[0].get("conclusion") == "success"
             and len(steps) == 1 and steps[0].get("status") == "completed" and steps[0].get("conclusion") == "success"
             and _utc_epoch(started) <= _utc_epoch(steps[0].get("completed_at")) <= deadline, "Completed callback Await did not succeed within original clock")
    name = f"krr-governance-source-{source['id']}-attempt-1"
    page = reader.request(f"repos/{repository}/actions/artifacts?name={name}&per_page=100&page=1")
    entries, total = page.get("artifacts"), page.get("total_count")
    _require(type(total) is int and 1 <= total <= 2 and isinstance(entries, list) and len(entries) == total
             and all(isinstance(entry, dict) and _integer(entry.get("id")) for entry in entries)
             and len({entry["id"] for entry in entries}) == total, "Completed callback original journal missing/ambiguous")
    matches = []
    repository_id = source.get("repository", {}).get("id")
    for metadata in entries:
        journal, _ = _journal_from_metadata(reader, metadata, source["id"], repository, repository_id, workflow_sha, workflow_blob)
        member = journal["member"]
        _require(member["latch_started_at"] in (None, started), "Completed callback restarted source clock")
        normalized = dict(member, latch_started_at=started, source_deadline_epoch=deadline)
        if member_source_matches(normalized, source, pull):
            matches.append(normalized)
    _require(bool(matches), "Completed callback current head/body differs from original journal")
    after = reader.request(f"repos/{repository}/actions/runs/{source['id']}")
    current = reader.request(f"repos/{repository}/pulls/{pull['number']}")
    _require(after.get("status") == "completed" and after.get("conclusion") == "success"
             and any(member_source_matches(member, after, current) for member in matches), "Completed callback current source/PR drift")
    return True


def completed_consumer_is_noop(cohort, generation, *, reader):
    try:
        header = cohort["header"]
        repository, identifier = header["repository"], generation.get("id")
        _require(_integer(identifier) and generation.get("event") == "workflow_run"
                 and generation.get("status") == "completed" and generation.get("conclusion") == "success", "Not a completed no-op consumer")
        producer = reader.producer(repository, identifier, header["workflow_sha"], header["workflow_blob_sha"])
        _require(producer.get("status") == "completed" and producer.get("conclusion") == "success", "No-op producer lifecycle drift")
        jobs = reader.jobs(repository, identifier)
        names = [job.get("name") for job in jobs]
        _require(len(names) == len(set(names)), "No-op jobs ambiguous")
        mapping = dict(zip(names, jobs))
        preflight = mapping.get("Preflight workflow_run governance source")
        _require(isinstance(preflight, dict) and preflight.get("status") == "completed" and preflight.get("conclusion") == "success", "No-op preflight missing")
        pattern = re.compile(COMPLETED_NOOP_PREFIX + r"([1-9][0-9]*)")
        matches = [step for step in preflight.get("steps", []) if isinstance(step, dict) and pattern.fullmatch(step.get("name", ""))]
        _require(len(matches) == 1 and matches[0].get("status") == "completed" and matches[0].get("conclusion") == "success", "No-op stage missing/ambiguous")
        for name in MUTATING_JOBS | {PRODUCER_JOB, ELECTION_JOB, ADMISSION_JOB}:
            job = mapping.get(name)
            _require(isinstance(job, dict) and job.get("status") == "completed" and job.get("conclusion") == "skipped", "No-op mutation path not positively skipped")
        source_id = int(pattern.fullmatch(matches[0]["name"])[1])
        source = reader.request(f"repos/{repository}/actions/runs/{source_id}")
        pulls = source.get("pull_requests")
        _require(isinstance(pulls, list) and len(pulls) == 1 and isinstance(pulls[0], dict) and _integer(pulls[0].get("number")), "No-op source PR ambiguous")
        pull = reader.request(f"repos/{repository}/pulls/{pulls[0]['number']}")
        return completed_sensor_noop(source, pull, reader=reader, workflow_sha=header["workflow_sha"], workflow_blob=header["workflow_blob_sha"])
    except (CohortError, KeyError, TypeError, ValueError):
        return False


class DispatcherCohortFilter:
    """raw完全pageを読み、肯定証明済followerだけ旧100件domainから除く。"""
    def __init__(self, artifact_id, digest, owner, *, read_json, read_archive, budget, deadline, phase):
        repository = os.environ["GITHUB_REPOSITORY"]
        self.reader = _Reader(read_json, read_archive, budget, deadline)
        header, _ = self.reader.artifact({"artifact_id": artifact_id, "artifact_digest": digest}, "cohort", repository, expected_producer=owner)
        _require(isinstance(header.get("member_ids"), list), "Member cohort fence inventory malformed")
        empty = not header["member_ids"]
        self.cohort = load_cohort(artifact_id, digest, expected_owner=owner, expected_segment=None,
                                 member_ids=None if empty else {header["member_ids"][0]}, allow_empty=empty,
                                 read_json=read_json, read_archive=read_archive, budget=budget, deadline=deadline)
        verify_cohort_lease(self.cohort, phase=phase, read_json=read_json, budget=budget, deadline=deadline)
        self.known = cohort_known_producer_ids(self.cohort, read_json=read_json, read_archive=read_archive,
                                              budget=budget, deadline=deadline)
        self.owner, self.repository, self.covered = owner, repository, {}

    def page(self, endpoint, initial):
        from copy import deepcopy
        parsed = urllib.parse.urlsplit(endpoint)
        path = f"repos/{self.repository}/actions/workflows/"
        if not parsed.path.startswith(path) or not parsed.path.endswith("/runs"):
            return initial
        workflow = parsed.path[len(path):-5]
        owner_run = self.reader.producer(self.repository, self.owner, self.cohort["header"]["workflow_sha"], self.cohort["header"]["workflow_blob_sha"])
        if workflow not in {"pr-governance.yml", str(owner_run["workflow_id"])}:
            return initial
        pairs = urllib.parse.parse_qs(parsed.query, keep_blank_values=True)
        _require(all(len(values) == 1 for values in pairs.values()) and pairs.get("per_page") == ["100"], "Dispatcher query ambiguous")
        requested_page = int(pairs.get("page", ["1"])[0])
        pairs["page"] = ["1"]
        def endpoint_page(number):
            query = dict((key, value[0]) for key, value in pairs.items())
            query["page"] = number
            return parsed.path + "?" + urllib.parse.urlencode(query)
        first = initial if requested_page == 1 else self.reader.request(endpoint_page(1))
        total = first.get("total_count") if isinstance(first, dict) else None
        _require(type(total) is int and 0 <= total <= 600 and isinstance(first.get("workflow_runs"), list), "Raw dispatcher page incomplete")
        pages = [first]
        for number in range(2, max(1, (total + 99) // 100) + 1):
            pages.append(self.reader.request(endpoint_page(number)))
        flattened = []
        for page in pages:
            entries = page.get("workflow_runs") if isinstance(page, dict) else None
            _require(isinstance(entries, list) and len(entries) <= 100 and page.get("total_count") == total
                     and all(isinstance(run, dict) and _integer(run.get("id")) for run in entries), "Raw dispatcher page malformed")
            flattened.extend(entries)
        _require(len(flattened) == total and len({run["id"] for run in flattened}) == total
                 and self.reader.request(endpoint_page(1)) == first, "Raw dispatcher inventory duplicate/drift/truncated")
        retained = []
        immutable = ("id", "run_attempt", "name", "display_title", "head_sha", "head_branch", "path", "event", "workflow_id", "run_number", "repository", "head_repository", "status", "conclusion")
        for run in flattened:
            identifier = run["id"]
            binding = {key: deepcopy(run.get(key)) for key in immutable}
            if identifier in self.covered:
                _require(self.covered[identifier] == binding, "Covered immutable producer identity drift")
                continue
            if identifier in self.known and consumer_is_covered(self.cohort, run, read_json=self.reader.read_json,
                                                              read_archive=self.reader.read_archive, budget=self.reader.budget, deadline=self.reader.deadline):
                self.covered[identifier] = binding
                continue
            if (run.get("event") == "workflow_run" and isinstance(run.get("display_title"), str)
                    and re.fullmatch(r"sensor=[1-9][0-9]* action=(?:in_progress|completed)", run["display_title"])
                    and consumer_is_deferred(self.cohort, run, read_json=self.reader.read_json,
                                            read_archive=self.reader.read_archive, budget=self.reader.budget, deadline=self.reader.deadline)):
                self.covered[identifier] = binding
                continue
            if completed_consumer_is_noop(self.cohort, run, reader=self.reader):
                self.covered[identifier] = binding
                continue
            retained.append(run)
        _require(len(retained) <= 100, "Unknown/direct dispatcher inventory exceeds existing bound")
        return {"total_count": len(retained), "workflow_runs": retained if requested_page == 1 else []}


_process_fence_filter = None


def filter_dispatcher_page(endpoint, initial, *, artifact_id, digest, owner, token, read_json, read_archive, deadline, phase):
    global _process_fence_filter
    if _process_fence_filter is None:
        budget = ReadBudget(900)
        if read_archive is None:
            def read_archive(endpoint, *, timeout):
                budget.charge()
                return read_artifact_archive(endpoint, token, timeout)
        _process_fence_filter = DispatcherCohortFilter(artifact_id, digest, owner, read_json=read_json,
                                                       read_archive=read_archive, budget=budget, deadline=deadline, phase=phase)
    _require(_process_fence_filter.owner == owner and _process_fence_filter.cohort["artifact_id"] == artifact_id
             and _process_fence_filter.cohort["artifact_digest"] == digest, "Process fence binding changed")
    return _process_fence_filter.page(endpoint, initial)


def protection_canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def filter_runtime_dispatcher_page(endpoint, initial, *, read_json, deadline, phase, read_archive=None, token=""):
    return filter_dispatcher_page(endpoint, initial, artifact_id=int(os.environ["COHORT_ARTIFACT_ID"]),
                                  digest=os.environ["COHORT_ARTIFACT_DIGEST"], owner=int(os.environ["GITHUB_RUN_ID"]),
                                  token=token, read_json=read_json, read_archive=read_archive, deadline=deadline, phase=phase)


def protection_records(protection, status_checks_url, contexts_url):
    required=protection.get("required_status_checks") if isinstance(protection,dict) else None
    checks=required.get("checks") if isinstance(required,dict) else None; contexts=required.get("contexts") if isinstance(required,dict) else None; strict=required.get("strict") if isinstance(required,dict) else None
    if not isinstance(required,dict) or set(required)!={"url","contexts_url","strict","contexts","checks"} or required["url"]!=status_checks_url or required["contexts_url"]!=contexts_url or not isinstance(checks,list) or not isinstance(contexts,list) or type(strict) is not bool: raise CohortError("Affected-head barrier branch protection is invalid.")
    result=[]
    for item in checks:
        if not isinstance(item,dict) or set(item)!={"context","app_id"} or not isinstance(item["context"],str) or not item["context"] or "\x00" in item["context"] or (item["app_id"] is not None and type(item["app_id"]) is not int): raise CohortError("Affected-head barrier branch protection is invalid.")
        result.append((item["context"],item["app_id"]))
    if len(set(result))!=len(result) or len({name for name,_ in result})!=len(result) or [name for name,_ in result]!=contexts: raise CohortError("Affected-head barrier branch protection is invalid.")
    return required,result

def canonical_preserved_map(raw, preserved, manifest, legacy_writer):
    rows = json.loads(raw)
    _require(isinstance(rows, list) and json.dumps(rows, separators=(",", ":")) == raw
             and all(isinstance(row, list) and len(row) == 3 and all(_integer(value) for value in row) for row in rows)
             and len({row[0] for row in rows}) == len(rows) and len({row[2] for row in rows}) == len(rows), "Invalid cohort preservation map")
    _require(not rows or ([row[0] for row in rows] == preserved and [[row[0], row[2]] for row in rows] == manifest
                         and legacy_writer == "0"), "Cohort preservation coverage differs")
    return {row[0]: (str(row[1]), row[2]) for row in rows}


def _runtime_cohort(reader, segment):
    owner = int(os.environ["GITHUB_RUN_ID"])
    artifact = int(os.environ["COHORT_ARTIFACT_ID"])
    digest = os.environ["COHORT_ARTIFACT_DIGEST"]
    cohort = load_cohort(artifact, digest, expected_owner=owner, expected_segment=segment,
                         read_json=reader.read_json, read_archive=reader.read_archive,
                         budget=reader.budget, deadline=reader.deadline)
    reader.deadline = min(reader.deadline, cohort["header"]["root_deadline_epoch"])
    verify_cohort_lease(cohort, phase="early", read_json=reader.read_json, budget=reader.budget, deadline=reader.deadline)
    members = {}
    repository = cohort["header"]["repository"]
    for identifier, member in cohort["members_by_id"].items():
        normalized = resolve_member_clock(member, read_json=reader.read_json, budget=reader.budget, deadline=reader.deadline)
        source = reader.request(f"repos/{repository}/actions/runs/{identifier}")
        pull = reader.request(f"repos/{repository}/pulls/{member['pr_number']}")
        after = reader.request(f"repos/{repository}/actions/runs/{identifier}")
        _require(member_source_matches(normalized, source, pull) and member_source_matches(normalized, after, pull), "Current batch member drift")
        members[identifier] = normalized
    cohort["members_by_id"] = members
    return cohort


def _writer_identity(run, cohort, segment, identifier=None):
    header = cohort["header"]
    title = f"source={header['owner_dispatcher_run_id']} scope=early segment={segment}"
    return (isinstance(run, dict) and _integer(run.get("id")) and (identifier is None or run["id"] == identifier)
            and run.get("name") in {"PR governance status writer", title} and run.get("display_title") == title
            and run.get("path") in {".github/workflows/pr-governance-status-writer.yml", ".github/workflows/pr-governance-status-writer.yml@" + os.environ["DEFAULT_BRANCH"], ".github/workflows/pr-governance-status-writer.yml@refs/heads/" + os.environ["DEFAULT_BRANCH"]}
            and run.get("event") == "workflow_dispatch" and run.get("head_branch") == os.environ["DEFAULT_BRANCH"]
            and run.get("head_sha") == header["workflow_sha"] and type(run.get("run_attempt")) is int and run["run_attempt"] == 1
            and _integer(run.get("run_number")) and repository_matches(run.get("repository"), header["repository"], header["repository_id"])
            and run.get("status") in ACTIVE | {"completed"}
            and (run.get("conclusion") is None if run.get("status") in ACTIVE else run.get("conclusion") == "success"))


def dispatch_early(reader):
    import subprocess
    segment = int(os.environ["COHORT_SEGMENT"])
    cohort = _runtime_cohort(reader, segment)
    targets = sorted(cohort["target_members_by_number"])
    validate_target_members(cohort, segment, targets)
    repository = cohort["header"]["repository"]
    owner = cohort["header"]["owner_dispatcher_run_id"]
    created = reader.request(f"repos/{repository}/actions/runs/{owner}").get("created_at")
    canonical_timestamp(created)
    query = urllib.parse.urlencode({"branch": os.environ["DEFAULT_BRANCH"], "head_sha": cohort["header"]["workflow_sha"], "created": ">=" + created, "per_page": 100})
    endpoint = f"repos/{repository}/actions/workflows/pr-governance-status-writer.yml/runs?{query}"
    def inventory():
        page = reader.request(endpoint)
        runs, total = page.get("workflow_runs"), page.get("total_count")
        _require(isinstance(runs, list) and type(total) is int and 0 <= total <= 100 and total == len(runs)
                 and all(isinstance(run, dict) and _integer(run.get("id")) for run in runs)
                 and len({run["id"] for run in runs}) == len(runs), "Writer registration page incomplete/duplicated")
        return runs
    before = {run["id"] for run in inventory()}
    inputs = {"dispatcher_run_id": str(owner), "scope": "early", "target_numbers": json.dumps(targets, separators=(",", ":")),
              "preserved_target_numbers": "[]", "preserved_writer_run_id": "0", "check_manifest": "[]",
              "terminal_batch_numbers": "[]", "continuation_index": str(segment), "terminal_order_numbers": "[]",
              "completed_writer_run_ids": "[]", "cohort_artifact_id": str(cohort["artifact_id"]),
              "cohort_artifact_digest": cohort["artifact_digest"], "preserved_target_writer_map": "[]", "terminal_deadline_epoch": "0"}
    arguments = ["gh", "api", "--method", "POST", f"repos/{repository}/actions/workflows/pr-governance-status-writer.yml/dispatches", "-f", "ref=" + os.environ["DEFAULT_BRANCH"]]
    for key, value in inputs.items():
        arguments.extend(["-f", f"inputs[{key}]={value}"])
    remaining = reader.deadline - time.time()
    _require(remaining > 0 and bool(os.environ.get("COHORT_DISPATCH_TOKEN")), "Dispatch token/clock missing")
    result = subprocess.run(arguments, capture_output=True, text=True, check=False, timeout=min(20, remaining),
                            env={"GH_TOKEN": os.environ["COHORT_DISPATCH_TOKEN"], "PATH": os.environ["PATH"]})
    _require(result.returncode == 0, "Cohort writer dispatch failed")
    registration_deadline = min(reader.deadline, time.time() + 300)
    while time.time() < registration_deadline:
        title = f"source={owner} scope=early segment={segment}"
        candidates = [run for run in inventory() if run["id"] not in before and run.get("display_title") == title]
        _require(len(candidates) <= 1, "Ambiguous cohort writer registration")
        if candidates:
            _require(_writer_identity(candidates[0], cohort, segment), "Foreign cohort writer registration")
            _write_outputs({"writer_run_id": candidates[0]["id"]})
            return
        time.sleep(min(5, max(0, registration_deadline - time.time())))
    raise CohortError("Cohort writer registration expired")


def writer_clock(jobs, run, cohort, previous=None):
    matches = [job for job in jobs if job.get("name") == "Write authoritative governance Check Runs"]
    _require(len(matches) <= 1, "Ambiguous writer admission job")
    if not matches:
        return previous
    job = matches[0]
    repository = cohort["header"]["repository"]
    _require(job.get("run_id") == run["id"] and job.get("head_sha") == run["head_sha"]
             and job.get("head_branch") == run["head_branch"] and job.get("workflow_name") == "PR governance status writer"
             and job.get("run_url") == f"https://api.github.com/repos/{repository}/actions/runs/{run['id']}"
             and job.get("url") == f"https://api.github.com/repos/{repository}/actions/jobs/{job['id']}", "Writer admission job identity drift")
    steps = job.get("steps")
    _require(isinstance(steps, list) and all(isinstance(step, dict) and _integer(step.get("number")) for step in steps)
             and len({step["number"] for step in steps}) == len(steps), "Writer admission stages invalid")
    writes = [step for step in steps if step.get("name") == "Revalidate every current open pull request and publish fenced states"]
    _require(len(writes) <= 1, "Ambiguous writer publish stage")
    if not writes or writes[0].get("started_at") is None:
        return previous
    step = writes[0]
    started = datetime.fromisoformat(canonical_timestamp(step["started_at"]).replace("Z", "+00:00")).timestamp()
    job_started = datetime.fromisoformat(canonical_timestamp(job.get("started_at")).replace("Z", "+00:00")).timestamp()
    _require(cohort["header"]["root_deadline_epoch"] - 21000 <= job_started <= started <= time.time(), "Writer admission clock outside root")
    source_deadlines = [member["source_deadline_epoch"] for member in cohort["members_by_id"].values()]
    deadline = min(started + 1200, cohort["header"]["root_deadline_epoch"], *source_deadlines)
    pin = (job["id"], step["number"], started, deadline)
    _require(previous is None or previous == pin, "Writer admission clock restarted")
    for metadata in (job, step):
        _require(metadata.get("status") in ACTIVE | {"completed"}
                 and (metadata.get("conclusion") is None if metadata.get("status") in ACTIVE else metadata.get("conclusion") == "success"), "Writer write phase failed")
        if metadata.get("completed_at") is not None:
            completed = datetime.fromisoformat(canonical_timestamp(metadata["completed_at"]).replace("Z", "+00:00")).timestamp()
            _require(started <= completed <= deadline, "Writer published after original admission deadline")
    _require(time.time() < deadline, "Original writer admission deadline expired")
    return pin


def _completed_checks(reader, cohort, writer):
    repository = cohort["header"]["repository"]
    app_id = int(os.environ["CHECK_APP_ID"])
    _require(app_id == 4766933, "Unexpected governance Check Run App")
    manifest = []
    for number, identifiers in sorted(cohort["target_members_by_number"].items()):
        member = cohort["members_by_id"][identifiers[0]]
        pull = reader.request(f"repos/{repository}/pulls/{number}")
        for identifier in identifiers:
            source = reader.request(f"repos/{repository}/actions/runs/{identifier}")
            _require(member_source_matches(cohort["members_by_id"][identifier], source, pull), "Completed writer member/current PR drift")
        query = urllib.parse.urlencode({"check_name": "KRR / PR governance (trusted check)", "app_id": app_id, "filter": "all", "per_page": 100})
        endpoint = f"repos/{repository}/commits/{member['head_sha']}/check-runs?{query}"
        first = reader.request(endpoint + "&page=1")
        entries, total = first.get("check_runs"), first.get("total_count")
        _require(isinstance(entries, list) and type(total) is int and 0 <= total <= 600, "Check Run bounded page invalid")
        checks = list(entries)
        for page_number in range(2, 7):
            if len(checks) >= total:
                break
            page = reader.request(endpoint + f"&page={page_number}")
            _require(page.get("total_count") == total and isinstance(page.get("check_runs"), list) and len(page["check_runs"]) <= 100, "Check Run count drift")
            checks.extend(page["check_runs"])
        _require(len(checks) == total and all(isinstance(check, dict) and _integer(check.get("id")) for check in checks)
                 and len({check["id"] for check in checks}) == len(checks) and reader.request(endpoint + "&page=1") == first,
                 "Check Run pagination incomplete/duplicated/changed")
        external = f"krr-governance/v1/{member['head_sha']}/writer-{writer}"
        matches = [check for check in checks if check.get("name") == "KRR / PR governance (trusted check)"
                   and check.get("head_sha") == member["head_sha"] and check.get("external_id") == external
                   and isinstance(check.get("app"), dict) and check["app"].get("id") == app_id
                   and check.get("status") == "completed" and check.get("conclusion") == "success"]
        _require(len(matches) == 1, "Completed cohort Check Run ambiguous")
        parsed = urllib.parse.urlparse(matches[0].get("details_url", ""))
        _require(parsed.scheme == "https" and parsed.netloc == "github.com" and parsed.path == f"/{repository}/actions/runs/{writer}", "Check Run writer details binding")
        query = urllib.parse.parse_qs(parsed.query, keep_blank_values=True)
        expected = {"pr_body_sha256": member["pr_body_sha256"], "cohort_artifact_id": str(cohort["artifact_id"]),
                    "cohort_artifact_digest": cohort["artifact_digest"], "cohort_segment": os.environ["COHORT_SEGMENT"]}
        _require(all(query.get(key) == [value] for key, value in expected.items()), "Check Run cohort/body query binding")
        manifest.append([number, writer, matches[0]["id"]])
    return manifest


def await_early(reader):
    segment, writer = int(os.environ["COHORT_SEGMENT"]), int(os.environ["COHORT_WRITER_ID"])
    cohort = _runtime_cohort(reader, segment)
    _require(cohort["registered_writers"].get(segment) == writer, "Unregistered cohort writer")
    repository = cohort["header"]["repository"]
    original = min(reader.deadline, *(member["source_deadline_epoch"] for member in cohort["members_by_id"].values()))
    reader.deadline = original
    pin, polls = None, 0
    while time.time() < reader.deadline:
        polls += 1
        _require(polls * 3 <= 441, "Early waiter request budget expired")
        before = reader.request(f"repos/{repository}/actions/runs/{writer}")
        _require(_writer_identity(before, cohort, segment, writer), "Bound early writer identity/lifecycle drift")
        reader.cache.pop(("jobs", repository, writer), None)
        jobs = reader.jobs(repository, writer)
        after = reader.request(f"repos/{repository}/actions/runs/{writer}")
        _require(_writer_identity(after, cohort, segment, writer) and all(before.get(key) == after.get(key) for key in
                 ("id", "run_attempt", "head_sha", "head_branch", "path", "event", "run_number", "repository", "workflow_id")), "Writer changed during admission read")
        pin = writer_clock(jobs, after, cohort, pin)
        if pin is not None:
            reader.deadline = min(original, pin[3])
        if after.get("status") == "completed":
            _require(pin is not None, "Successful writer lacks exact admission clock")
            manifest = _completed_checks(reader, cohort, writer)
            _write_outputs({"preserved_target_writer_map": json.dumps(manifest, separators=(",", ":"))})
            return
        time.sleep(min(30, max(0, reader.deadline - time.time())))
    raise CohortError("Original early writer/source/root clock expired")


def acknowledge_members(reader):
    segment = int(os.environ["COHORT_SEGMENT"])
    cohort = _runtime_cohort(reader, segment)
    repository = cohort["header"]["repository"]
    pending = set(cohort["members_by_id"])
    reads = {identifier: 0 for identifier in pending}
    while pending:
        for identifier in sorted(pending):
            member = cohort["members_by_id"][identifier]
            _require(time.time() < min(reader.deadline, member["source_deadline_epoch"]), "Original member ACK deadline expired")
            reads[identifier] += 1
            _require(reads[identifier] <= 300, "Member ACK read budget expired")
            source = reader.request(f"repos/{repository}/actions/runs/{identifier}")
            if source.get("status") == "completed":
                pull = reader.request(f"repos/{repository}/pulls/{member['pr_number']}")
                after = reader.request(f"repos/{repository}/actions/runs/{identifier}")
                _require(source.get("conclusion") == "success" and member_source_matches(member, source, pull)
                         and member_source_matches(member, after, pull) and after.get("status") == "completed"
                         and after.get("conclusion") == "success", "Exact source ACK/current PR drift")
                pending.remove(identifier)
            else:
                _require(source.get("status") in ACTIVE and source.get("conclusion") is None
                         and source.get("id") == identifier and source.get("run_attempt") == 1, "Unknown source ACK lifecycle")
        if pending:
            time.sleep(min(5, max(0, reader.deadline - time.time())))
    _write_outputs({"acknowledged_source_ids": json.dumps(sorted(cohort["members_by_id"]), separators=(",", ":"))})


def dispatch_recovery(reader):
    import subprocess
    repository, repository_id, owner, sha, blob = _producer_context(reader)
    run = reader.producer(repository, owner, sha, blob)
    _require(run.get("status") in ACTIVE and run.get("conclusion") is None
             and repository_matches(run.get("repository"), repository, repository_id), "Recovery dispatch owner is not active")
    jobs = reader.jobs(repository, owner)
    by_name = {job.get("name"): job for job in jobs}
    _require(len(by_name) == len(jobs), "Ambiguous recovery dispatch job")
    admission = by_name.get(ADMISSION_JOB)
    election = by_name.get(ELECTION_JOB)
    _require(isinstance(admission, dict) and admission.get("status") == "completed" and admission.get("conclusion") == "success"
             and isinstance(election, dict) and isinstance(election.get("steps"), list), "Failed owner never admitted a backend")
    selected = [step for step in election["steps"] if isinstance(step, dict) and isinstance(step.get("name"), str)
                and re.fullmatch(SELECTED_PREFIX + rf"owner={owner} artifact=[1-9][0-9]* digest=sha256:[0-9a-f]{{64}}", step["name"])
                and step.get("status") == "completed" and step.get("conclusion") == "success"]
    _require(len(selected) == 1, "Failed owner selection is missing or ambiguous")
    required = MUTATING_JOBS - {"Dispatch failed selected cohort recovery"}
    required |= {ELECTION_JOB, ADMISSION_JOB, PRODUCER_JOB, "Preflight workflow_run governance source"}
    evidence = [by_name.get(name) for name in required]
    _require(all(isinstance(job, dict) and job.get("status") == "completed"
                 and job.get("conclusion") in {"success", "failure", "cancelled", "skipped", "timed_out", "action_required", "neutral"} for job in evidence)
             and any(job.get("conclusion") == "failure" for job in evidence), "No positive completed failure or backend still running")
    reader.budget.charge()
    remaining = reader.deadline - time.time()
    _require(remaining > 0, "Original recovery dispatch deadline expired")
    result = subprocess.run(["gh", "api", "--hostname", "github.com", "--method", "POST",
                             f"repos/{repository}/actions/workflows/pr-governance.yml/dispatches",
                             "-f", "ref=" + os.environ["DEFAULT_BRANCH"], "-f", f"inputs[recovery_owner]={owner}"],
                            capture_output=True, text=True, check=False, timeout=min(20, remaining))
    _require(result.returncode == 0, "Failed selected owner recovery dispatch was not accepted")
    _write_outputs({"recovery_owner": owner})


def main():
    mode = os.environ.get("COHORT_MODE", "")
    _require(mode in {"producer", "select", "build-header", "admission", "await-handoff", "dispatch-early", "await-early", "ack-members", "dispatch-recovery"}, "Unknown cohort CLI role")
    root = os.environ.get("COHORT_ORIGINAL_ROOT_DEADLINE", "")
    _require(re.fullmatch(r"[1-9][0-9]*", root) and time.time() < int(root), "Original cohort root expired")
    budget = ReadBudget(300 if mode in {"producer", "admission"} else 900)
    transport = _Transport(os.environ.get("GH_TOKEN", ""), budget)
    reader = _Reader(transport.json, transport.archive, budget, int(root))
    if mode == "producer":
        produce_journal(reader)
    elif mode == "select":
        select_cohort(reader)
    elif mode == "build-header":
        build_cohort_header()
    elif mode == "admission":
        admit_backend(reader)
    elif mode == "dispatch-early":
        dispatch_early(reader)
    elif mode == "await-early":
        await_early(reader)
    elif mode == "ack-members":
        acknowledge_members(reader)
    elif mode == "dispatch-recovery":
        dispatch_recovery(reader)
    else:
        await_handoff(reader)
    _write_outputs({"cohort_read_attempts": budget.used, "cohort_primary_reads": transport.primary_reads})


if __name__ == "__main__":
    try:
        main()
    except (CohortError, KeyError, ValueError, OSError) as error:
        # API endpoint・署名付きURL・tokenを含む例外の原文を表示しない。
        raise SystemExit("Strict governance cohort role failed closed.") from None
