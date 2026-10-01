import base64
import hashlib
import io
import json
import unittest
import zipfile

import pr_governance_cohort as cohort


class TerminalInventoryNonmutationTests(unittest.TestCase):
    def fixture(self):
        import copy
        repo = {"id": 101, "full_name": "owner/repository"}
        header = {"repository": "owner/repository", "repository_id": 101, "owner_dispatcher_run_id": 100,
                  "workflow_sha": "a" * 40, "workflow_blob_sha": "b" * 40, "root_deadline_epoch": 5500}
        owner = {"id": 100, "workflow_id": 2, "head_branch": "master", "head_sha": "a" * 40,
                 "repository": repo}
        generation = dict(owner, id=99, run_number=89, run_attempt=1, name="sensor=11 action=in_progress",
                          display_title="sensor=11 action=in_progress", head_repository=repo,
                          path=".github/workflows/pr-governance.yml@refs/heads/master", event="workflow_run",
                          status="completed", conclusion="cancelled")
        def step(name, number):
            return {"name": name, "number": number, "status": "completed", "conclusion": "success"}
        names = ["Preflight workflow_run governance source", cohort.PRODUCER_JOB, cohort.ELECTION_JOB,
                 cohort.ADMISSION_JOB] + sorted(cohort.MUTATING_JOBS)
        jobs = [{"id": 990 + index, "run_id": 99, "run_attempt": 1, "head_sha": header["workflow_sha"],
                 "name": name, "status": "completed", "conclusion": "success" if index < 2 else
                 "cancelled" if index == 2 else "failure" if index == 3 else "skipped", "steps": []}
                for index, name in enumerate(names)]
        jobs[0]["steps"] = [step("Exclude unavailable fork sources before dispatcher lock", 1)]
        jobs[1]["steps"] = [step(cohort.PAYLOAD_PREFIX + "source sha256=" + "c" * 64, 1),
                             step("Upload immutable krr-governance-source-11-attempt-1", 2)]
        calls, budget = [], Budget(900)
        def read(endpoint, *, timeout):
            calls.append(endpoint)
            self.assertEqual(endpoint, "repos/owner/repository/actions/runs/99/attempts/1/jobs?per_page=100&page=1")
            return {"total_count": len(jobs), "jobs": copy.deepcopy(jobs)}
        def archive(endpoint, *, timeout):
            self.fail("Inventory classification must not read source journal archives")
        reader = cohort._Reader(read, archive, budget, 5500)
        reader.cache[("producer", header["repository"], 100)] = owner
        reader.cache[("workflow", header["repository"], header["workflow_sha"], header["workflow_blob_sha"])] = b"trusted"
        return {"header": header}, generation, jobs, reader, calls, budget

    def proof(self, fixture):
        from unittest.mock import patch
        context, generation, _, reader, _, _ = fixture
        with patch.object(cohort.time, "time", return_value=200):
            return cohort.terminal_inventory_is_nonmutating(context, generation, reader=reader)

    def test_native_positive_uses_one_job_read_without_authority_or_clock_changes(self):
        import copy
        fixture = self.fixture()
        before = copy.deepcopy(fixture[0])
        self.assertTrue(self.proof(fixture))
        self.assertEqual(fixture[5].used, 1)
        self.assertEqual(len(fixture[4]), 1)
        self.assertEqual(fixture[0], before)

    def completed_fixture(self):
        fixture = self.fixture()
        fixture[1].update(name="sensor=11 action=completed", display_title="sensor=11 action=completed", conclusion="success")
        for job in fixture[2][1:4]:
            job.update(conclusion="skipped", steps=[])
        fixture[2][0]["steps"].append({"name": cohort.COMPLETED_NOOP_PREFIX + "11", "number": 2,
                                     "status": "completed", "conclusion": "success"})
        return fixture

    def test_completed_callback_inventory_has_no_member_or_ack_authority(self):
        import copy
        fixture = self.completed_fixture(); before = copy.deepcopy(fixture[0])
        self.assertTrue(self.proof(fixture))
        self.assertEqual(fixture[5].used, 1)
        self.assertEqual(fixture[0], before)

    def test_completed_callback_requires_exact_unique_native_noop_stage(self):
        import copy
        for mutation in ("missing", "foreign", "malformed", "duplicate", "failed", "boolnumber", "active-title",
                         "published", "selected", "admitted", "mutated", "recovery"):
            with self.subTest(mutation=mutation):
                fixture = self.completed_fixture(); stages = fixture[2][0]["steps"]
                if mutation == "missing": stages.pop()
                elif mutation == "foreign": stages[-1]["name"] = cohort.COMPLETED_NOOP_PREFIX + "12"
                elif mutation == "malformed": stages[-1]["name"] += "extra"
                elif mutation == "duplicate":
                    duplicate = copy.deepcopy(stages[-1]); duplicate["number"] = 3; stages.append(duplicate)
                elif mutation == "failed": stages[-1]["conclusion"] = "failure"
                elif mutation == "boolnumber": stages[-1]["number"] = True
                elif mutation == "active-title": fixture[1].update(name="sensor=11 action=in_progress", display_title="sensor=11 action=in_progress")
                elif mutation in {"published", "selected", "admitted"}: fixture[2][{"published": 1, "selected": 2, "admitted": 3}[mutation]]["conclusion"] = "success"
                elif mutation == "mutated": fixture[2][4]["conclusion"] = "cancelled"
                else: stages.append({"name": cohort.RECOVERY_PREFIX + "malformed", "number": 3, "status": "completed", "conclusion": "success"})
                self.assertFalse(self.proof(fixture))

    def test_terminal_and_workflow_identity_types_are_strict(self):
        for field, value in (("id", 99.0), ("id", True), ("run_attempt", True), ("run_number", 89.0),
                             ("workflow_id", 2.0), ("head_sha", "c" * 40), ("head_branch", "foreign"),
                             ("path", ".github/workflows/unknown.yml"), ("event", "workflow_dispatch"),
                             ("status", "in_progress"), ("conclusion", "success"), ("display_title", "unknown")):
            with self.subTest(field=field, value=value):
                fixture = self.fixture(); fixture[1][field] = value
                self.assertFalse(self.proof(fixture))
                self.assertEqual(fixture[5].used, 0)
        for field in ("repository", "head_repository"):
            fixture = self.fixture(); fixture[1][field] = {"id": 101.0, "full_name": "owner/repository"}
            self.assertFalse(self.proof(fixture))

    def test_native_job_and_step_identity_types_are_strict(self):
        for field, value in (("id", 990.0), ("run_id", 99.0), ("run_id", True), ("run_attempt", True),
                             ("head_sha", "c" * 40), ("name", None), ("status", "in_progress")):
            with self.subTest(field=field, value=value):
                fixture = self.fixture(); fixture[2][0][field] = value
                self.assertFalse(self.proof(fixture))
        for field, value in (("number", True), ("number", 1.0), ("name", None), ("status", "in_progress")):
            fixture = self.fixture(); fixture[2][0]["steps"][0][field] = value
            self.assertFalse(self.proof(fixture))

    def test_missing_duplicate_jobs_or_steps_and_untrusted_reader_rejected(self):
        import copy
        for mutation in ("missing", "duplicate-name", "duplicate-step", "unknown-job", "missing-owner", "missing-workflow"):
            with self.subTest(mutation=mutation):
                fixture = self.fixture()
                if mutation == "missing": fixture[2].pop()
                elif mutation == "duplicate-name":
                    extra = copy.deepcopy(fixture[2][0]); extra["id"] += 1000; fixture[2].append(extra)
                elif mutation == "duplicate-step": fixture[2][1]["steps"].append(copy.deepcopy(fixture[2][1]["steps"][0]))
                elif mutation == "unknown-job": fixture[2][0]["name"] = "Unknown governance writer"
                elif mutation == "missing-owner": fixture[3].cache.pop(("producer", "owner/repository", 100))
                else: fixture[3].cache.pop(("workflow", "owner/repository", "a" * 40, "b" * 40))
                self.assertFalse(self.proof(fixture))

    def test_every_mutation_and_selected_admitted_recovery_path_is_retained(self):
        for name in cohort.MUTATING_JOBS:
            for conclusion in ("success", "failure", "cancelled"):
                fixture = self.fixture()
                next(job for job in fixture[2] if job["name"] == name)["conclusion"] = conclusion
                self.assertFalse(self.proof(fixture), (name, conclusion))
        for prefix in (cohort.SELECTED_PREFIX, cohort.ADMITTED_PREFIX, cohort.RECOVERY_PREFIX):
            for conclusion in ("success", "failure", "cancelled"):
                fixture = self.fixture()
                fixture[2][0]["steps"].append({"name": prefix + "malformed", "number": 2,
                                              "status": "completed", "conclusion": conclusion})
                self.assertFalse(self.proof(fixture))
        fixture = self.fixture()
        fixture[2][0]["steps"].append({"name": cohort.RECOVERY_PREFIX + "owner= journal= digest= root=",
                                     "number": 2, "status": "completed", "conclusion": "skipped"})
        self.assertTrue(self.proof(fixture))

    def test_required_stages_and_exact_source_upload_positive_evidence(self):
        for index in range(4):
            fixture = self.fixture(); fixture[2][index]["conclusion"] = "success" if index >= 2 else "failure"
            self.assertFalse(self.proof(fixture))
        for index in range(2):
            fixture = self.fixture(); fixture[2][1]["steps"][index]["conclusion"] = "skipped"
            self.assertFalse(self.proof(fixture))
        fixture = self.fixture(); fixture[2][1]["steps"][1]["name"] = "Upload immutable krr-governance-source-12-attempt-1"
        self.assertFalse(self.proof(fixture))
        fixture = self.fixture(); fixture[2][1]["steps"][0]["name"] = cohort.PAYLOAD_PREFIX + "source sha256=invalid"
        self.assertFalse(self.proof(fixture))

    def native_inventory_model(self, cancelled, completed, active):
        import copy
        import urllib.parse
        from unittest.mock import patch
        fixture = self.fixture()
        generations, all_jobs, calls = [], {}, []
        for index in range(cancelled + completed):
            identifier, source = 1000 + index, 11 + index
            generation = dict(fixture[1], id=identifier, run_number=identifier,
                              name=f"sensor={source} action=in_progress", display_title=f"sensor={source} action=in_progress")
            jobs = copy.deepcopy(fixture[2])
            for offset, job in enumerate(jobs):
                job.update(id=identifier * 100 + offset, run_id=identifier)
            jobs[1]["steps"][1]["name"] = f"Upload immutable krr-governance-source-{source}-attempt-1"
            if index >= cancelled:
                generation.update(name=f"sensor={source} action=completed", display_title=f"sensor={source} action=completed", conclusion="success")
                for job in jobs[1:4]:
                    job.update(conclusion="skipped", steps=[])
                jobs[0]["steps"].append({"name": cohort.COMPLETED_NOOP_PREFIX + str(source), "number": 2,
                                         "status": "completed", "conclusion": "success"})
            generations.append(generation); all_jobs[identifier] = jobs
        inventory = generations + [dict(fixture[1], id=2000 + index, status="in_progress", conclusion=None) for index in range(active)]
        self.assertEqual(len(inventory), 600)
        def read(endpoint, *, timeout):
            calls.append(endpoint)
            if "/workflows/" in endpoint:
                page = int(urllib.parse.parse_qs(urllib.parse.urlsplit(endpoint).query)["page"][0])
                return {"total_count": 600, "workflow_runs": copy.deepcopy(inventory[(page - 1) * 100:page * 100])}
            identifier = int(endpoint.split("/runs/")[1].split("/")[0])
            self.assertEqual(endpoint, f"repos/owner/repository/actions/runs/{identifier}/attempts/1/jobs?per_page=100&page=1")
            return {"total_count": len(all_jobs[identifier]), "jobs": copy.deepcopy(all_jobs[identifier])}
        fixture[3].read_json = read
        def pages():
            return [fixture[3].request(f"repos/owner/repository/actions/workflows/2/runs?per_page=100&page={page}") for page in range(1, 7)]
        with patch.object(cohort.time, "time", return_value=200):
            before = pages()
            for generation in generations:
                self.assertTrue(cohort.terminal_inventory_is_nonmutating(fixture[0], generation, reader=fixture[3]))
            self.assertEqual(json.dumps(before, sort_keys=True, allow_nan=False), json.dumps(pages(), sort_keys=True, allow_nan=False))
        self.assertEqual(fixture[5].used, cancelled + completed + 12)
        self.assertEqual(len(calls), cancelled + completed + 12)
        self.assertEqual(len({endpoint for endpoint in calls if "/attempts/" in endpoint}), cancelled + completed)
        self.assertFalse(any("/artifacts" in endpoint or "/pulls" in endpoint for endpoint in calls))

    def test_499_native_jobs_and_complete_inventory_fit_existing_budget(self):
        self.native_inventory_model(499, 0, 101)

    def test_bound_600_inventory_with_completed_callbacks_needs_400_native_jobs(self):
        self.native_inventory_model(200, 200, 200)


class CohortWireTests(unittest.TestCase):
    def test_canonical_json_rejects_duplicate_extra_unicode_and_whitespace(self):
        self.assertEqual(cohort.decode_payload(b'{"v":1}', {"v"}), {"v": 1})
        for data in (b'{"v":1,"v":1}', b'{ "v":1}', b'{"v":1,"extra":2}', b'{"v":NaN}'):
            with self.subTest(data=data), self.assertRaises(cohort.CohortError):
                cohort.decode_payload(data, {"v"})

    def test_archive_has_one_bounded_regular_canonical_file(self):
        def archive(names):
            out = io.BytesIO()
            with zipfile.ZipFile(out, "w") as z:
                for name in names:
                    z.writestr(name, b'{"v":1}')
            return out.getvalue()
        self.assertEqual(cohort.decode_archive(archive(["binding.json"])), b'{"v":1}')
        for names in (["../binding.json"], ["binding.json", "other.json"], ["binding.json", "binding.json"]):
            with self.subTest(names=names), self.assertRaises(cohort.CohortError):
                cohort.decode_archive(archive(names))
        with self.assertRaises(cohort.CohortError):
            cohort.decode_archive(b"x" * 8193)

    def test_many_to_one_batch_keeps_each_source_and_each_target_limit(self):
        members = {11: {"pr_number": 7}, 12: {"pr_number": 7}, 13: {"pr_number": 8}}
        pages = cohort.build_batches(members)
        self.assertEqual(pages, [{"segment": 1, "target_members": [[7, [11, 12]], [8, [13]]]}])
        self.assertEqual(len(cohort.build_batches({i: {"pr_number": i} for i in range(1, 201)})), 4)
        with self.assertRaises(cohort.CohortError):
            cohort.build_batches({i: {"pr_number": i} for i in range(1, 202)})

    def test_fixed_source_clock_unstarted_is_hint_and_never_reset(self):
        self.assertIsNone(cohort.source_deadline(None, now=100))
        self.assertEqual(cohort.source_deadline("1970-01-01T00:01:40Z", now=100), 5500)
        for value in ("1970-01-01T00:01:41Z", "1970-01-01T00:01:40+00:00", "bad"):
            with self.subTest(value=value), self.assertRaises(cohort.CohortError):
                cohort.source_deadline(value, now=100)

    def test_redirect_does_not_forward_authorization_or_accept_api_redirect(self):
        class Response:
            def __init__(self, code, data=b"", location=None):
                self.code, self.data = code, data
                self.headers = {"Location": location} if location else {}
            def read(self, limit): return self.data[:limit]
        requests = []
        def open_request(request, timeout):
            requests.append(request)
            return Response(302, location="https://productionresultssa0.blob.core.windows.net/opaque") if len(requests) == 1 else Response(200, b"zip")
        self.assertEqual(cohort.read_artifact_archive("repos/owner/repository/actions/artifacts/5/zip", "secret", 20, open_request), b"zip")
        self.assertEqual(requests[0].get_header("Authorization"), "Bearer secret")
        self.assertIsNone(requests[1].get_header("Authorization"))
        with self.assertRaises(cohort.CohortError):
            cohort.read_artifact_archive("https://evil.example/zip", "secret", 20, open_request)


class Budget:
    def __init__(self, limit=300):
        self.limit, self.used = limit, 0
    def charge(self):
        self.used += 1
        if self.used > self.limit:
            raise cohort.CohortError("Read budget exhausted")


class RawArtifactFixture:
    def __init__(self):
        self.repository = "owner/repository"
        self.sha = "a" * 40
        self.code = b"trusted-default-workflow"
        self.blob = hashlib.sha1(b"blob " + str(len(self.code)).encode() + b"\0" + self.code).hexdigest()
        self.owner = 100
        self.member = {"source_run_id": 11, "source_run_attempt": 1, "source_workflow_id": 9,
                       "source_run_number": 8, "source_event": "pull_request_review", "pr_number": 7,
                       "base_ref": "master", "base_sha": "b" * 40, "head_sha": "c" * 40,
                       "pr_body_sha256": hashlib.sha256(b"body").hexdigest(), "latch_started_at": "1970-01-01T00:01:40Z",
                       "source_deadline_epoch": 5500, "repository": self.repository, "repository_id": 101}
        self.journal = {"v": 1, "kind": "source", "producer_run_id": 100, "producer_run_attempt": 1,
                        "repository": self.repository, "repository_id": 101, "workflow_sha": self.sha,
                        "workflow_blob_sha": self.blob, "root_deadline_epoch": 21200, "member": self.member}
        self.data, self.archives, self.metadata, self.calls = {}, {}, {}, []
        self.steps = {cohort.PRODUCER_JOB: [], cohort.ELECTION_JOB: [],
                      "Preflight workflow_run governance source": [{"number": 1, "name": "Exclude unavailable fork sources before dispatcher lock",
                      "status": "completed", "conclusion": "success", "started_at": "1970-01-01T00:01:40Z", "completed_at": "1970-01-01T00:03:20Z"}]}
        journal_ref = self.artifact(4, self.journal)
        self.steps[cohort.PRODUCER_JOB].append({"number": 3, "name": cohort.JOURNAL_PREFIX + base64.b64encode(cohort.canonical(self.journal)).decode(),
                                              "status": "completed", "conclusion": "success"})
        page = {"v": 1, "kind": "members", "owner_dispatcher_run_id": 100, "entries": [[11, 100, 4, journal_ref["artifact_digest"]]]}
        batch = {"v": 1, "kind": "batch", "owner_dispatcher_run_id": 100, "segment": 1, "target_members": [[7, [11]]]}
        self.header = {"v": 1, "kind": "cohort", "owner_dispatcher_run_id": 100, "owner_run_attempt": 1,
                       "repository": self.repository, "repository_id": 101, "workflow_sha": self.sha,
                       "workflow_blob_sha": self.blob, "root_deadline_epoch": 21200, "member_ids": [11], "member_segments": "1",
                       "member_pages": [self.artifact(2, page, 1)],
                       "alias_pages": [self.artifact(5, {"v": 1, "kind": "aliases", "owner_dispatcher_run_id": 100, "entries": [[11, [100]]]}, 1)], "batches": [self.artifact(3, batch, 1)], "root_journal": journal_ref}
        self.header["cohort_digest"] = hashlib.sha256(cohort.canonical(self.header)).hexdigest()
        self.header_ref = self.artifact(1, self.header)
        self.run = {"id": 100, "run_attempt": 1, "name": "PR governance dispatcher", "head_sha": self.sha,
                    "head_branch": "master", "path": ".github/workflows/pr-governance.yml@master", "event": "workflow_run",
                    "workflow_id": 2, "run_number": 90, "repository": {"id": 101, "full_name": self.repository}}
        self.jobs = {"total_count": 3, "jobs": [{"id": i, "run_id": 100, "run_attempt": 1, "head_sha": self.sha,
                      "name": name, "status": "in_progress" if name == cohort.ELECTION_JOB else "completed",
                      "conclusion": None if name == cohort.ELECTION_JOB else "success", "steps": steps}
                     for i, (name, steps) in enumerate(self.steps.items(), 1)]}
        repo_path = "repos/" + self.repository
        self.data[repo_path] = {"default_branch": "master"}
        self.data[repo_path + "/git/ref/heads/master"] = {"object": {"sha": self.sha}}
        self.data[repo_path + "/contents/.github/workflows/pr-governance.yml?ref=" + self.sha] = {
            "path": ".github/workflows/pr-governance.yml", "encoding": "base64", "sha": self.blob,
            "size": len(self.code), "content": base64.b64encode(self.code).decode()}
        self.data[repo_path + "/actions/runs/100"] = self.run
        self.data[repo_path + "/actions/runs/100/attempts/1/jobs?per_page=100&page=1"] = self.jobs

    def artifact(self, identifier, payload, segment=None):
        out = io.BytesIO()
        with zipfile.ZipFile(out, "w", compression=zipfile.ZIP_DEFLATED) as archive:
            archive.writestr("binding.json", cohort.canonical(payload))
        data = out.getvalue()
        digest = "sha256:" + hashlib.sha256(data).hexdigest()
        kind = payload["kind"]
        name = f"krr-governance-{kind}-100-attempt-1" + (f"-{segment}" if segment else "")
        if kind == "source":
            name = f"krr-governance-source-{payload['member']['source_run_id']}-attempt-1"
        metadata = {"id": identifier, "name": name, "expired": False, "digest": digest,
                    "size_in_bytes": len(data), "workflow_run": {"id": 100, "repository_id": 101,
                    "head_repository_id": 101, "head_sha": self.sha}}
        self.archives[identifier], self.metadata[identifier] = data, metadata
        creator = cohort.PRODUCER_JOB if kind == "source" else cohort.ELECTION_JOB
        self.steps[creator].extend([
            {"name": cohort.PAYLOAD_PREFIX + kind + " sha256=" + hashlib.sha256(cohort.canonical(payload)).hexdigest(),
             "number": len(self.steps[creator]) + 1, "status": "completed", "conclusion": "success"},
            {"name": "Upload immutable " + name, "number": len(self.steps[creator]) + 2,
             "status": "completed", "conclusion": "success"}])
        return {"artifact_id": identifier, "artifact_digest": digest}

    def read_json(self, endpoint, *, timeout):
        import copy
        self.calls.append(endpoint)
        if "/actions/artifacts/" in endpoint:
            return copy.deepcopy(self.metadata[int(endpoint.rsplit("/", 1)[1])])
        return copy.deepcopy(self.data[endpoint])

    def read_archive(self, endpoint, *, timeout):
        self.calls.append(endpoint)
        return self.archives[int(endpoint.split("/")[-2])]

    def load(self, budget=None, **options):
        from unittest.mock import patch
        with patch.dict("os.environ", {"GITHUB_REPOSITORY": self.repository}), patch.object(cohort.time, "time", return_value=200):
            return cohort.load_cohort(1, self.header_ref["artifact_digest"], expected_owner=100,
                                     expected_segment=1, read_json=self.read_json, read_archive=self.read_archive,
                                     budget=budget or Budget(), deadline=300, **options)


class CohortRawBoundaryTests(unittest.TestCase):
    def native_child(self):
        import copy
        fixture = RawArtifactFixture()
        journal = copy.deepcopy(fixture.journal)
        journal["producer_run_id"] = 99
        reference = fixture.artifact(6, journal)
        fixture.metadata[6]["workflow_run"]["id"] = 99
        producer_steps = copy.deepcopy(fixture.steps[cohort.PRODUCER_JOB][-2:])
        producer_steps.append({"number": producer_steps[-1]["number"] + 1,
                              "name": cohort.JOURNAL_PREFIX + base64.b64encode(cohort.canonical(journal)).decode(),
                              "status": "completed", "conclusion": "success"})
        fixture.steps[cohort.PRODUCER_JOB][:] = fixture.steps[cohort.PRODUCER_JOB][:3]
        producer = copy.deepcopy(fixture.run)
        producer.update(id=99, run_number=89)
        endpoint = "repos/owner/repository/actions/runs/99"
        fixture.data[endpoint] = producer
        fixture.data[endpoint + "/attempts/1/jobs?per_page=100&page=1"] = {"total_count": 2, "jobs": [
            {"id": 991, "run_id": 99, "run_attempt": 1, "head_sha": fixture.sha, "name": cohort.PRODUCER_JOB,
             "status": "completed", "conclusion": "success", "steps": producer_steps},
            {"id": 992, "run_id": 99, "run_attempt": 1, "head_sha": fixture.sha,
             "name": "Preflight workflow_run governance source", "status": "completed", "conclusion": "success",
             "steps": copy.deepcopy(fixture.steps["Preflight workflow_run governance source"])}]}
        fixture.steps[cohort.ELECTION_JOB][:] = []
        fixture.header["member_pages"] = [fixture.artifact(2, {"v": 1, "kind": "members", "owner_dispatcher_run_id": 100,
                                                              "entries": [[11, 99, 6, reference["artifact_digest"]]]}, 1)]
        fixture.header["alias_pages"] = [fixture.artifact(5, {"v": 1, "kind": "aliases", "owner_dispatcher_run_id": 100,
                                                             "entries": [[11, [99, 100]]]}, 1)]
        fixture.header["batches"] = [fixture.artifact(3, {"v": 1, "kind": "batch", "owner_dispatcher_run_id": 100,
                                                         "segment": 1, "target_members": [[7, [11]]]}, 1)]
        fixture.header.pop("cohort_digest")
        fixture.header["cohort_digest"] = hashlib.sha256(cohort.canonical(fixture.header)).hexdigest()
        fixture.header_ref = fixture.artifact(1, fixture.header)
        fixture.archives.pop(6)
        return fixture, producer_steps, journal

    def test_native_child_journal_is_verified_without_reopening_unavailable_archive(self):
        fixture, _, _ = self.native_child()
        result = fixture.load()
        self.assertEqual(result["members_by_id"], {11: fixture.member})
        self.assertEqual(result["producer_ids_by_member"], {11: 99})
        self.assertNotIn("repos/owner/repository/actions/artifacts/6/zip", fixture.calls)
        self.assertIn("repos/owner/repository/actions/artifacts/4/zip", fixture.calls)
        self.assertEqual(fixture.calls.count("repos/owner/repository/actions/artifacts/6"), 2)

    def test_native_child_journal_rejects_marker_upload_and_typed_binding_drift(self):
        import copy
        cases = ("missing_marker", "duplicate_marker", "invalid_marker", "failed_marker", "wrong_payload",
                 "missing_payload", "wrong_upload", "failed_upload", "body", "root", "source", "workflow",
                 "attempt", "producer_attempt", "step_number", "job_run_id", "job_head", "job_attempt",
                 "metadata_id", "metadata_digest", "metadata_name", "metadata_creator", "metadata_repo",
                 "metadata_head_repo", "metadata_head", "metadata_after")
        for mutation in cases:
            fixture, steps, journal = self.native_child()
            metadata = fixture.metadata[6]
            jobs = fixture.data["repos/owner/repository/actions/runs/99/attempts/1/jobs?per_page=100&page=1"]["jobs"]
            if mutation == "missing_marker": steps.pop()
            elif mutation == "duplicate_marker": steps.append(copy.deepcopy(steps[-1]))
            elif mutation == "invalid_marker": steps[-1]["name"] = cohort.JOURNAL_PREFIX + "not base64"
            elif mutation == "failed_marker": steps[-1]["conclusion"] = "failure"
            elif mutation == "wrong_payload": steps[0]["name"] += "foreign"
            elif mutation == "missing_payload": steps.pop(0)
            elif mutation == "wrong_upload": steps[1]["name"] += "foreign"
            elif mutation == "failed_upload": steps[1]["conclusion"] = "failure"
            elif mutation in {"body", "root", "source", "workflow", "attempt"}:
                if mutation == "body": journal["member"]["pr_body_sha256"] = "d" * 64
                elif mutation == "root": journal["root_deadline_epoch"] += 1
                elif mutation == "source": journal["member"]["source_run_id"] = 12
                elif mutation == "workflow": journal["workflow_blob_sha"] = "d" * 40
                else: journal["producer_run_attempt"] = True
                steps[-1]["name"] = cohort.JOURNAL_PREFIX + base64.b64encode(cohort.canonical(journal)).decode()
            elif mutation == "producer_attempt": fixture.data["repos/owner/repository/actions/runs/99"]["run_attempt"] = 2
            elif mutation == "step_number": steps[-1]["number"] = float(steps[-1]["number"])
            elif mutation == "job_run_id": jobs[0]["run_id"] = 99.0
            elif mutation == "job_head": jobs[0]["head_sha"] = "d" * 40
            elif mutation == "job_attempt": jobs[0]["run_attempt"] = True
            elif mutation == "metadata_id": metadata["id"] = 6.0
            elif mutation == "metadata_digest": metadata["digest"] = "sha256:" + "d" * 64
            elif mutation == "metadata_name": metadata["name"] = "krr-governance-source-12-attempt-1"
            elif mutation == "metadata_creator": metadata["workflow_run"]["id"] = 99.0
            elif mutation == "metadata_repo": metadata["workflow_run"]["repository_id"] = 101.0
            elif mutation == "metadata_head_repo": metadata["workflow_run"]["head_repository_id"] = 101.0
            elif mutation == "metadata_head": metadata["workflow_run"]["head_sha"] = "d" * 40
            else:
                read = fixture.read_json
                count = [0]
                def changed(endpoint, *, timeout):
                    value = read(endpoint, timeout=timeout)
                    if endpoint == "repos/owner/repository/actions/artifacts/6":
                        count[0] += 1
                        if count[0] == 2: value["workflow_run"]["repository_id"] = 101.0
                    return value
                fixture.read_json = changed
            with self.subTest(mutation=mutation), self.assertRaises(cohort.CohortError):
                fixture.load()
            self.assertNotIn("repos/owner/repository/actions/artifacts/6/zip", fixture.calls)

    def test_native_child_preserves_original_root_and_alias_validation(self):
        from unittest.mock import patch
        fixture, _, _ = self.native_child()
        fixture.data["repos/owner/repository/actions/runs/99/attempts/1/jobs?per_page=100&page=1"]["jobs"][1]["steps"][0]["completed_at"] = "1970-01-01T00:02:00Z"
        with self.assertRaises(cohort.CohortError): fixture.load()
        fixture, _, _ = self.native_child()
        context = fixture.load()
        with patch.object(cohort.time, "time", return_value=200):
            self.assertEqual(cohort.cohort_known_producer_ids(context, read_json=fixture.read_json,
                             read_archive=fixture.read_archive, budget=Budget(), deadline=300), {99: 11, 100: 11})
            context["header"]["alias_pages"] = context["header"]["member_pages"]
            with self.assertRaises(cohort.CohortError):
                cohort.cohort_known_producer_ids(context, read_json=fixture.read_json,
                                                read_archive=fixture.read_archive, budget=Budget(), deadline=300)

    def test_real_api_shapes_bind_header_pages_journal_owner_and_original_clock(self):
        fixture = RawArtifactFixture()
        result = fixture.load()
        self.assertEqual(result["members_by_id"], {11: fixture.member})
        self.assertEqual(cohort.validate_target_members(result, 1, [7]), {7: [fixture.member]})
        self.assertLessEqual(len(fixture.calls), 22)

    def test_sensor_compiled_blob_pin_avoids_contents_and_ref_requests(self):
        fixture = RawArtifactFixture()
        result = fixture.load(trusted_workflow_blob=fixture.blob)
        self.assertEqual(result["members_by_id"][11], fixture.member)
        self.assertFalse(any("/contents/" in endpoint or "/git/ref/" in endpoint for endpoint in fixture.calls))
        with self.assertRaises(cohort.CohortError):
            RawArtifactFixture().load(trusted_workflow_blob="f" * 40)

    def test_job_attempt_missing_uses_exact_attempt_endpoint(self):
        fixture = RawArtifactFixture()
        for job in fixture.jobs["jobs"]:
            job.pop("run_attempt")
        self.assertEqual(fixture.load()["members_by_id"], {11: fixture.member})
        self.assertTrue(any("/attempts/1/jobs?" in endpoint for endpoint in fixture.calls))

    def test_original_root_epoch_outside_preflight_stage_rejects(self):
        fixture = RawArtifactFixture()
        fixture.steps["Preflight workflow_run governance source"][0]["completed_at"] = "1970-01-01T00:02:00Z"
        with self.assertRaises(cohort.CohortError): fixture.load()

    def test_raw_stage_offset_is_normalized_without_clock_reset(self):
        self.assertEqual(cohort.canonical_timestamp("2019-08-08T08:00:00-07:00"), "2019-08-08T15:00:00Z")
        self.assertEqual(cohort.canonical_timestamp("1970-01-01T00:01:40.125000+00:00"), "1970-01-01T00:01:40.125Z")
        self.assertEqual(cohort.source_deadline("1970-01-01T00:01:40.125Z", now=101), 5500.125)

    def test_metadata_digest_expiry_and_foreign_creator_fail_closed(self):
        for field, value in (("expired", True), ("digest", "sha256:" + "f" * 64), ("id", True), ("size_in_bytes", 8193)):
            fixture = RawArtifactFixture()
            fixture.metadata[1][field] = value
            with self.subTest(field=field), self.assertRaises(cohort.CohortError):
                fixture.load()
        fixture = RawArtifactFixture()
        fixture.metadata[4]["workflow_run"]["id"] = 101
        with self.assertRaises(cohort.CohortError): fixture.load()

    def test_upload_and_binding_stage_missing_foreign_or_duplicate_reject(self):
        import copy
        for mutation in ("missing", "duplicate", "foreign", "upload_failure"):
            fixture = RawArtifactFixture()
            steps = fixture.steps[cohort.PRODUCER_JOB]
            if mutation == "missing": steps.pop(0)
            elif mutation == "duplicate": steps.append(copy.deepcopy(steps[0]))
            elif mutation == "foreign": steps[0]["name"] += "foreign"
            else: steps[1]["conclusion"] = "failure"
            with self.subTest(mutation=mutation), self.assertRaises(cohort.CohortError): fixture.load()

    def test_attempt_job_identity_and_incomplete_job_page_reject(self):
        for mutation in ("attempt", "run_id", "page", "duplicate"):
            fixture = RawArtifactFixture()
            if mutation == "attempt": fixture.jobs["jobs"][0]["run_attempt"] = 2
            elif mutation == "run_id": fixture.jobs["jobs"][0]["run_id"] = 101
            elif mutation == "page": fixture.jobs["total_count"] = 4
            else: fixture.jobs["jobs"][1]["id"] = fixture.jobs["jobs"][0]["id"]
            with self.subTest(mutation=mutation), self.assertRaises(cohort.CohortError): fixture.load()

    def test_budget_and_original_absolute_deadline_reject_before_extra_read(self):
        fixture = RawArtifactFixture()
        budget = Budget(1)
        with self.assertRaises(cohort.CohortError): fixture.load(budget)
        self.assertEqual(len(fixture.calls), 1)

    def test_current_default_ref_and_workflow_blob_drift_reject(self):
        fixture = RawArtifactFixture()
        fixture.data["repos/owner/repository/git/ref/heads/master"]["object"]["sha"] = "d" * 40
        with self.assertRaises(cohort.CohortError): fixture.load()
        fixture = RawArtifactFixture()
        fixture.data["repos/owner/repository/contents/.github/workflows/pr-governance.yml?ref=" + fixture.sha]["size"] = 0
        with self.assertRaises(cohort.CohortError): fixture.load()

    def test_foreign_selective_source_is_rejected(self):
        with self.assertRaises(cohort.CohortError): RawArtifactFixture().load(member_ids={12})



class CohortEarlyClockTests(unittest.TestCase):
    def fixture(self):
        context = {"header": {"repository": "owner/repository", "root_deadline_epoch": 21200},
                   "members_by_id": {11: {"source_deadline_epoch": 5500}, 12: {"source_deadline_epoch": 5300}}}
        run = {"id": 90, "head_sha": "a" * 40, "head_branch": "master"}
        job = {"id": 900, "run_id": 90, "head_sha": run["head_sha"], "head_branch": "master",
               "workflow_name": "PR governance status writer", "name": "Write authoritative governance Check Runs",
               "run_url": "https://api.github.com/repos/owner/repository/actions/runs/90",
               "url": "https://api.github.com/repos/owner/repository/actions/jobs/900",
               "started_at": "1970-01-01T00:15:00Z", "status": "in_progress", "conclusion": None,
               "steps": [{"name": "Revalidate every current open pull request and publish fenced states", "number": 6,
                          "started_at": "1970-01-01T00:16:40Z", "status": "in_progress", "conclusion": None}]}
        return context, run, job

    def test_actual_write_admission_clock_survives_queue_and_clamps_every_member(self):
        from unittest.mock import patch
        context, run, job = self.fixture()
        with patch.object(cohort.time, "time", return_value=1100):
            pin = cohort.writer_clock([job], run, context)
            self.assertEqual(pin, (900, 6, 1000, 2200))
        context["members_by_id"][12]["source_deadline_epoch"] = 2100
        with patch.object(cohort.time, "time", return_value=1200):
            self.assertEqual(cohort.writer_clock([job], run, context)[3], 2100)

    def test_admission_cannot_reset_after_repeated_poll_or_job_replacement(self):
        import copy
        from unittest.mock import patch
        context, run, job = self.fixture()
        with patch.object(cohort.time, "time", return_value=1100):
            pin = cohort.writer_clock([job], run, context)
            for mutated in ("id", "number", "started_at"):
                changed = copy.deepcopy(job)
                if mutated == "id": changed["id"] = 901; changed["url"] = changed["url"].replace("900", "901")
                elif mutated == "number": changed["steps"][0]["number"] = 7
                else: changed["steps"][0]["started_at"] = "1970-01-01T00:16:41Z"
                with self.subTest(mutated=mutated), self.assertRaises(cohort.CohortError):
                    cohort.writer_clock([changed], run, context, pin)
            self.assertEqual(cohort.writer_clock([job], run, context, pin), pin)

    def test_terminal_success_is_timely_and_other_conclusions_fail_closed(self):
        import copy
        from unittest.mock import patch
        context, run, job = self.fixture()
        for conclusion in ("success", "failure", "cancelled", None, "unknown"):
            changed = copy.deepcopy(job)
            for stage in (changed, changed["steps"][0]):
                stage.update(status="completed", conclusion=conclusion, completed_at="1970-01-01T00:30:00Z")
            with self.subTest(conclusion=conclusion), patch.object(cohort.time, "time", return_value=1900):
                if conclusion == "success": self.assertEqual(cohort.writer_clock([changed], run, context)[3], 2200)
                else:
                    with self.assertRaises(cohort.CohortError): cohort.writer_clock([changed], run, context)
        changed["conclusion"] = changed["steps"][0]["conclusion"] = "success"
        changed["steps"][0]["completed_at"] = "1970-01-01T00:36:41Z"
        with patch.object(cohort.time, "time", return_value=2202), self.assertRaises(cohort.CohortError):
            cohort.writer_clock([changed], run, context)

    def test_unstarted_hint_does_not_pin_clock_and_malformed_identity_rejects(self):
        import copy
        from unittest.mock import patch
        context, run, job = self.fixture()
        job["steps"][0]["started_at"] = None
        job["steps"][0]["status"] = "queued"
        with patch.object(cohort.time, "time", return_value=1100):
            self.assertIsNone(cohort.writer_clock([job], run, context))
            for field, value in (("run_id", 91), ("head_sha", "b" * 40), ("workflow_name", "foreign"), ("run_url", "https://evil.example")):
                changed = copy.deepcopy(job); changed[field] = value
                with self.subTest(field=field), self.assertRaises(cohort.CohortError):
                    cohort.writer_clock([changed], run, context)

    def test_source_unstarted_and_started_are_separate_admission_domains(self):
        source = {"id": 11, "head_sha": "c" * 40, "status": "queued"}
        self.assertIsNone(cohort.source_latch_clock(source, [], now=1000))
        job = {"run_id": 11, "head_sha": source["head_sha"], "name": "KRR / PR governance review latch",
               "steps": [{"name": "Await matching trusted governance Check Run", "number": 5,
                          "status": "in_progress", "started_at": "1970-01-01T00:16:40Z"}]}
        self.assertEqual(cohort.source_latch_clock(source, [job], now=1000), "1970-01-01T00:16:40Z")
        job["steps"][0]["started_at"] = None; job["steps"][0]["status"] = "queued"
        self.assertIsNone(cohort.source_latch_clock(source, [job], now=1000))
        source["status"] = "completed"
        with self.assertRaises(cohort.CohortError): cohort.source_latch_clock(source, [job], now=1000)


    def test_unmaterialized_active_latch_steps_are_only_an_unadmitted_hint(self):
        source = {"id": 11, "head_sha": "c" * 40, "status": "queued"}
        job = {"run_id": 11, "head_sha": source["head_sha"], "run_attempt": 1,
               "name": "KRR / PR governance review latch", "status": "queued",
               "conclusion": None, "steps": []}
        for source_state in cohort.ACTIVE:
            source["status"] = source_state
            for job_state in ("queued", "in_progress"):
                with self.subTest(source_state=source_state, job_state=job_state):
                    job["status"] = job_state
                    self.assertIsNone(cohort.source_latch_clock(source, [job], now=1000))
        for changes in ({"status": "completed"}, {"status": "unknown"}, {"status": "requested"},
                        {"status": "waiting"}, {"status": "pending"}, {"conclusion": "success"},
                        {"run_attempt": 2}, {"run_id": 12}, {"head_sha": "d" * 40},
                        {"steps": None}, {"steps": [{"name": "foreign", "status": "queued"}]}):
            invalid = dict(job, status="queued")
            invalid.update(changes)
            with self.subTest(changes=changes), self.assertRaises(cohort.CohortError):
                cohort.source_latch_clock(source, [invalid], now=1000)
        for state in ("requested", "waiting", "pending", "completed", "unknown"):
            step = {"name": "Await matching trusted governance Check Run", "number": 5,
                    "status": state, "started_at": None}
            with self.subTest(step_state=state), self.assertRaises(cohort.CohortError):
                cohort.source_latch_clock(source, [dict(job, status="queued", steps=[step])], now=1000)
        source["status"] = "completed"
        with self.assertRaises(cohort.CohortError):
            cohort.source_latch_clock(source, [dict(job, status="queued")], now=1000)


class CohortFollowerAndHandoffTests(unittest.TestCase):
    def follower(self):
        import copy
        fixture = RawArtifactFixture()
        context = fixture.load()
        journal = copy.deepcopy(fixture.journal); journal["producer_run_id"] = 99
        reference = fixture.artifact(6, journal)
        fixture.metadata[6]["workflow_run"]["id"] = 99
        run = copy.deepcopy(fixture.run); run.update(id=99, run_number=89, status="completed", conclusion="failure")
        source_steps = fixture.steps[cohort.PRODUCER_JOB][-2:]
        jobs = [{"id": 991, "run_id": 99, "head_sha": fixture.sha, "name": cohort.PRODUCER_JOB,
                 "status": "completed", "conclusion": "success", "steps": source_steps},
                {"id": 992, "run_id": 99, "head_sha": fixture.sha, "name": cohort.ELECTION_JOB,
                 "status": "completed", "conclusion": "cancelled", "steps": []},
                {"id": 993, "run_id": 99, "head_sha": fixture.sha, "name": cohort.ADMISSION_JOB,
                 "status": "completed", "conclusion": "failure", "steps": []},
                {"id": 994, "run_id": 99, "head_sha": fixture.sha, "name": "Preflight workflow_run governance source",
                 "status": "completed", "conclusion": "success", "steps": fixture.steps["Preflight workflow_run governance source"]}]
        jobs += [{"id": 1000 + index, "run_id": 99, "head_sha": fixture.sha, "name": name,
                  "status": "completed", "conclusion": "skipped", "steps": []} for index, name in enumerate(sorted(cohort.MUTATING_JOBS))]
        prefix = "repos/owner/repository/actions/runs/99"
        fixture.data[prefix] = run
        fixture.data[prefix + "/artifacts?per_page=100&page=1"] = {"total_count": 1, "artifacts": [fixture.metadata[6]]}
        fixture.data[prefix + "/attempts/1/jobs?per_page=100&page=1"] = {"total_count": len(jobs), "jobs": jobs}
        repo = {"id": 101, "full_name": fixture.repository}
        pull = {"number": 7, "state": "open", "draft": False, "body": "body", "base": {"ref": "master", "sha": "b" * 40, "repo": repo}, "head": {"sha": "c" * 40, "repo": repo}}
        source = {"id": 11, "run_attempt": 1, "workflow_id": 9, "run_number": 8, "name": "PR governance review sensor", "event": "pull_request_review", "head_sha": "c" * 40, "path": ".github/workflows/pr-governance-review-events.yml", "repository": repo, "head_repository": repo, "status": "completed", "conclusion": "success", "pull_requests": [pull]}
        fixture.data["repos/owner/repository/actions/runs/11"] = source
        fixture.data["repos/owner/repository/pulls/7"] = pull
        latch = {"id": 111, "run_id": 11, "head_sha": source["head_sha"], "name": "KRR / PR governance review latch", "steps": [{"number": 5, "name": "Await matching trusted governance Check Run", "started_at": "1970-01-01T00:01:40Z", "completed_at": "1970-01-01T00:03:20Z", "status": "completed", "conclusion": "success"}]}
        fixture.data["repos/owner/repository/actions/runs/11/attempts/1/jobs?per_page=100&page=1"] = {"total_count": 1, "jobs": [latch]}
        return fixture, context, run, jobs

    def test_cancelled_election_failure_observer_requires_every_mutation_skipped(self):
        from unittest.mock import patch
        fixture, context, run, jobs = self.follower()
        with patch.object(cohort.time, "time", return_value=200):
            self.assertTrue(cohort.consumer_is_covered(context, run, read_json=fixture.read_json,
                            read_archive=fixture.read_archive, budget=Budget(), deadline=300))
            jobs[-1].update(status="in_progress", conclusion=None)
            self.assertFalse(cohort.consumer_is_covered(context, run, read_json=fixture.read_json,
                             read_archive=fixture.read_archive, budget=Budget(), deadline=300))

    def test_selected_owner_missing_stage_or_foreign_member_never_becomes_follower(self):
        from unittest.mock import patch
        for mutation in ("selected", "missing_mutator", "wrong_source", "success_election"):
            fixture, context, run, jobs = self.follower()
            if mutation == "selected": jobs[1]["steps"] = [{"name": cohort.SELECTED_PREFIX + "owner=99 artifact=1 digest=" + fixture.header_ref["artifact_digest"], "status": "completed", "conclusion": "success"}]
            elif mutation == "missing_mutator": jobs.pop(); fixture.data["repos/owner/repository/actions/runs/99/attempts/1/jobs?per_page=100&page=1"]["total_count"] -= 1
            elif mutation == "wrong_source": context["header"]["member_ids"] = [12]
            else: jobs[1]["conclusion"] = "success"
            with self.subTest(mutation=mutation), patch.object(cohort.time, "time", return_value=200):
                self.assertFalse(cohort.consumer_is_covered(context, run, read_json=fixture.read_json,
                                 read_archive=fixture.read_archive, budget=Budget(), deadline=300))

    def test_handoff_waits_for_service_strict_wait_and_successful_backend_drain(self):
        from unittest.mock import patch
        import copy
        jobs = [{"name": name, "status": "completed", "conclusion": "success"} for name in
                ("Service bound cohort review sources", "Preserve pending review sensor before direct dispatch", "Preserve admitted sources before obsolete heavy preemption")]
        class Reader:
            deadline = 300
            cache = {}
            def jobs(self, *_): return copy.deepcopy(jobs)
        with patch.dict("os.environ", {"GITHUB_REPOSITORY": "owner/repository", "GITHUB_RUN_ID": "100", "COHORT_ARTIFACT_ID": "1", "COHORT_ARTIFACT_DIGEST": "sha256:" + "a" * 64}), patch.object(cohort.time, "time", return_value=200), patch.object(cohort, "_write_outputs") as outputs:
            cohort.await_handoff(Reader())
            self.assertEqual(outputs.call_args.args[0]["owner"], 100)
            for index in range(3):
                jobs[index]["conclusion"] = "failure"
                with self.subTest(index=index), self.assertRaises(cohort.CohortError): cohort.await_handoff(Reader())
                jobs[index]["conclusion"] = "success"
            jobs[-1]["conclusion"] = "skipped"
            with self.assertRaises(cohort.CohortError): cohort.await_handoff(Reader())


class CompletedSensorNoopTests(unittest.TestCase):
    def fixture(self):
        import base64
        fixture, context, _run, _jobs = CohortFollowerAndHandoffTests().follower()
        fixture.steps[cohort.PRODUCER_JOB][:] = fixture.steps[cohort.PRODUCER_JOB][:2]
        fixture.steps[cohort.PRODUCER_JOB].append({"number": 3, "name": cohort.JOURNAL_PREFIX + base64.b64encode(cohort.canonical(fixture.journal)).decode(), "status": "completed", "conclusion": "success"})
        fixture.jobs["jobs"][0]["steps"] = fixture.steps[cohort.PRODUCER_JOB]
        fixture.data["repos/owner/repository/actions/artifacts?name=krr-governance-source-11-attempt-1&per_page=100&page=1"] = {"total_count": 1, "artifacts": [fixture.metadata[4]]}
        fixture.data["repos/owner/repository/actions/runs/11/attempts/1/jobs?per_page=100&page=1"]["jobs"][0].update(status="completed",conclusion="success")
        return fixture

    def test_completed_success_original_await_journal_and_current_body_are_required(self):
        from unittest.mock import patch
        fixture = self.fixture()
        reader = cohort._Reader(fixture.read_json, fixture.read_archive, Budget(), 7000)
        with patch.dict("os.environ", {"GITHUB_REPOSITORY": fixture.repository}), patch.object(cohort.time, "time", return_value=200):
            self.assertTrue(cohort.completed_sensor_noop(fixture.data["repos/owner/repository/actions/runs/11"], fixture.data["repos/owner/repository/pulls/7"], reader=reader, workflow_sha=fixture.sha, workflow_blob=fixture.blob))

    def test_failure_cancelled_stale_head_body_unstarted_expired_never_noop(self):
        from unittest.mock import patch
        for mutation in ("failure", "cancelled", "head", "body", "unstarted", "expired", "late_completion", "missing_journal"):
            fixture = self.fixture(); source = fixture.data["repos/owner/repository/actions/runs/11"]; pull = fixture.data["repos/owner/repository/pulls/7"]
            now = 200
            if mutation in {"failure", "cancelled"}: source["conclusion"] = mutation
            elif mutation == "head": pull["head"]["sha"] = "d" * 40
            elif mutation == "body": pull["body"] = "edited"
            elif mutation == "unstarted": fixture.data["repos/owner/repository/actions/runs/11/attempts/1/jobs?per_page=100&page=1"]["jobs"][0]["steps"][0]["started_at"] = None
            elif mutation == "expired": now = 5500
            elif mutation == "late_completion": fixture.data["repos/owner/repository/actions/runs/11/attempts/1/jobs?per_page=100&page=1"]["jobs"][0]["steps"][0]["completed_at"] = "1970-01-01T01:31:41Z"
            else: fixture.data["repos/owner/repository/actions/artifacts?name=krr-governance-source-11-attempt-1&per_page=100&page=1"] = {"total_count": 0, "artifacts": []}
            with self.subTest(mutation=mutation), patch.dict("os.environ", {"GITHUB_REPOSITORY": fixture.repository}), patch.object(cohort.time, "time", return_value=now):
                with self.assertRaises(cohort.CohortError):
                    cohort.completed_sensor_noop(source, pull, reader=cohort._Reader(fixture.read_json, fixture.read_archive, Budget(), 7000), workflow_sha=fixture.sha, workflow_blob=fixture.blob)

    def test_pure_protection_contract_and_exact_map_retain_original_failclosed(self):
        import copy
        expected = {"url": "https://api.github.com/checks", "contexts_url": "https://api.github.com/contexts", "strict": True, "contexts": ["one"], "checks": [{"context": "one", "app_id": 4766933}]}
        self.assertEqual(cohort.protection_records({"required_status_checks": expected}, expected["url"], expected["contexts_url"]), (expected, [("one", 4766933)]))
        for mutation in ("strict", "context", "extra", "duplicate"):
            record=copy.deepcopy(expected)
            if mutation=="strict": record["strict"]=1
            elif mutation=="context": record["contexts"]=["other"]
            elif mutation=="extra": record["extra"]=True
            else: record["checks"]*=2;record["contexts"]*=2
            with self.subTest(mutation=mutation),self.assertRaises(cohort.CohortError): cohort.protection_records({"required_status_checks":record},expected["url"],expected["contexts_url"])
        self.assertEqual(cohort.canonical_preserved_map("[[7,90,900]]",[7],[[7,900]],"0"),{7:("90",900)})
        for raw,targets,manifest,scalar in (("[[7,90,900]]",[8],[[8,900]],"0"),("[[7,90,900],[7,91,901]]",[7,7],[[7,900],[7,901]],"0"),("[[7,90,900]]",[7],[[7,900]],"90")):
            with self.assertRaises(cohort.CohortError): cohort.canonical_preserved_map(raw,targets,manifest,scalar)


class NativeCohortPrepareTests(unittest.TestCase):
    def test_actual_trusted_loader_checks_git_blob_default_ref_and_private_file(self):
        import base64,os,re,stat,tempfile,textwrap
        from pathlib import Path
        from unittest.mock import patch
        from types import SimpleNamespace
        source=Path(__file__).resolve().parents[2]/".github/workflows/pr-governance.yml"
        text=source.read_text();start=text.index("- name: Prepare trusted immutable cohort fence reader")
        fragment=text[start:]; match=re.search(r"run: \|\n( +)python3 - <<'PY'\n(.*?)\n\1PY",fragment,re.S)
        self.assertIsNotNone(match)
        program=textwrap.dedent(match[2])
        code=Path(cohort.__file__).read_bytes()
        content={"type":"file","encoding":"base64","sha":hashlib.sha1(b"blob "+str(len(code)).encode()+b"\0"+code).hexdigest(),"content":base64.b64encode(code).decode()}
        with tempfile.TemporaryDirectory() as directory:
            env={"GITHUB_REPOSITORY":"owner/repository","DEFAULT_BRANCH":"master","WORKFLOW_REF":"owner/repository/.github/workflows/pr-governance.yml@refs/heads/master","WORKFLOW_SHA":"a"*40,"RUNNER_TEMP":directory}
            def response(args,**_): return SimpleNamespace(returncode=0,stdout=json.dumps({"default_branch":"master"} if args[-1]=="repos/owner/repository" else content))
            with patch.dict(os.environ,env),patch("subprocess.run",side_effect=response):
                exec(compile(program,"native-prepare","exec"),{})
            private=Path(directory)/"krr-cohort-fence-reader.py"
            self.assertEqual(private.read_bytes(),code)
            self.assertEqual(stat.S_IMODE(private.stat().st_mode),0o600)
            with patch.dict(os.environ,env),patch("subprocess.run",side_effect=response),self.assertRaises(FileExistsError): exec(compile(program,"native-prepare","exec"),{})
        with tempfile.TemporaryDirectory() as directory:
            env["RUNNER_TEMP"]=directory;content["sha"]="b"*40
            with patch.dict(os.environ,env),patch("subprocess.run",side_effect=response),self.assertRaises(SystemExit): exec(compile(program,"native-prepare","exec"),{})
            self.assertFalse((Path(directory)/"krr-cohort-fence-reader.py").exists())
        content["sha"]=hashlib.sha1(b"blob "+str(len(code)).encode()+b"\0"+code).hexdigest()
        env["WORKFLOW_REF"]="owner/repository/.github/workflows/pr-governance.yml@refs/heads/foreign"
        with patch.dict(os.environ,env),patch("subprocess.run",side_effect=response),self.assertRaises(SystemExit): exec(compile(program,"native-prepare","exec"),{})


class DispatcherInventoryBoundaryTests(unittest.TestCase):
    def fixture(self):
        import copy
        fixture,context,run,jobs=CohortFollowerAndHandoffTests().follower()
        fixture.steps[cohort.PRODUCER_JOB][:]=fixture.steps[cohort.PRODUCER_JOB][:3]
        fixture.run.update(status="in_progress",conclusion=None)
        binding="owner=100 artifact=1 digest="+fixture.header_ref["artifact_digest"]
        fixture.steps[cohort.ELECTION_JOB].append({"name":cohort.SELECTED_PREFIX+binding,"status":"completed","conclusion":"success"})
        fixture.jobs["jobs"].append({"id":999,"run_id":100,"head_sha":fixture.sha,"name":cohort.ADMISSION_JOB,
            "status":"completed","conclusion":"success","steps":[{"name":cohort.ADMITTED_PREFIX+binding,
            "status":"completed","conclusion":"success"}]})
        fixture.jobs["total_count"]=len(fixture.jobs["jobs"])
        unknown=dict(copy.deepcopy(fixture.run),id=1000,run_number=1000)
        rows=[run,fixture.run,unknown]
        def read(endpoint,*,timeout):
            if "/actions/workflows/2/runs?" in endpoint:
                import copy
                page=int(__import__("urllib.parse",fromlist=["parse_qs"]).parse_qs(endpoint.split("?",1)[1]).get("page",["1"])[0])
                return {"total_count":len(rows),"workflow_runs":copy.deepcopy(rows[(page-1)*100:page*100])}
            return fixture.read_json(endpoint,timeout=timeout)
        reader=cohort._Reader(read,fixture.read_archive,Budget(),300)
        adapter=cohort.DispatcherCohortFilter.__new__(cohort.DispatcherCohortFilter)
        adapter.reader,adapter.cohort,adapter.known=reader,context,{99:11}
        adapter.owner,adapter.repository,adapter.covered,adapter.phase=100,fixture.repository,{},"early"
        adapter.nonmutating={}
        endpoint="repos/owner/repository/actions/workflows/2/runs?per_page=100&page=1"
        return fixture,adapter,rows,endpoint,read

    def test_real_positive_follower_and_context_owner_removed_and_unknown_generation_retained(self):
        from unittest.mock import patch
        fixture,adapter,rows,endpoint,read=self.fixture()
        with patch.dict("os.environ",{"GITHUB_REPOSITORY":fixture.repository}),patch.object(cohort.time,"time",return_value=200):
            page=adapter.page(endpoint,read(endpoint,timeout=20))
            self.assertEqual(page["total_count"],1)
            self.assertEqual(page["workflow_runs"][0]["id"],1000)
            self.assertEqual(set(adapter.covered),{99})
            rows[0]["run_number"]+=1
            with self.assertRaises(cohort.CohortError):adapter.page(endpoint,read(endpoint,timeout=20))

    def test_page_duplicate_unknown_101_and_raw_truncation_fail_closed(self):
        from unittest.mock import patch
        for mutation in ("duplicate","unknown101","truncated"):
            fixture,adapter,rows,endpoint,read=self.fixture()
            if mutation=="duplicate": rows.append(rows[0])
            elif mutation=="unknown101": rows[:]=[{"id":1000+i,"event":"workflow_dispatch"} for i in range(101)]
            page=read(endpoint,timeout=20)
            if mutation=="truncated": page["total_count"]=4
            with self.subTest(mutation=mutation),patch.dict("os.environ",{"GITHUB_REPOSITORY":fixture.repository}),patch.object(cohort.time,"time",return_value=200),self.assertRaises(cohort.CohortError):adapter.page(endpoint,page)

if __name__ == "__main__":
    unittest.main()


class CohortRecoveryTests(unittest.TestCase):
    def select_and_admit(self, fixture):
        fixture.jobs['jobs'][1]['steps'].append({'name':cohort.SELECTED_PREFIX+'owner=100 artifact=1 digest='+fixture.header_ref['artifact_digest'],
                                               'status':'completed','conclusion':'success','number':20})
        fixture.jobs['jobs'].insert(3,{'id':999,'run_id':100,'head_sha':fixture.sha,'name':cohort.ADMISSION_JOB,
                                    'status':'completed','conclusion':'success','steps':[]})
        fixture.jobs['total_count']=len(fixture.jobs['jobs'])

    def failed_selected(self):
        fixture=RawArtifactFixture();fixture.run.update(status='completed',conclusion='failure',created_at='1970-01-01T00:01:40Z')
        fixture.jobs['jobs'][1].update(status='completed',conclusion='failure')
        fixture.jobs['jobs'] += [{'id':200+i,'run_id':100,'head_sha':fixture.sha,'name':name,'status':'completed','conclusion':'skipped','steps':[]} for i,name in enumerate(sorted(cohort.MUTATING_JOBS))]
        self.select_and_admit(fixture)
        fixture.data['repos/owner/repository/actions/runs/100/artifacts?per_page=100&page=1']={'total_count':len(fixture.metadata),'artifacts':list(fixture.metadata.values())}
        fixture.data['repos/owner/repository/actions/workflows/pr-governance-status-writer.yml/runs?created=%3E%3D1970-01-01T00%3A01%3A40Z&per_page=100&page=1']={'total_count':0,'workflow_runs':[]}
        return fixture

    def test_recovery_rejects_unselected_unadmitted_or_ambiguous_owner_proof(self):
        import copy
        from unittest.mock import patch
        for mutation in ('selected_missing','selected_failure','selected_duplicate','selected_foreign','admission_missing','admission_skipped','admission_failure','admission_duplicate'):
            fixture=self.failed_selected();election=fixture.jobs['jobs'][1];admission=fixture.jobs['jobs'][3]
            if mutation=='selected_missing':election['steps'].pop()
            elif mutation=='selected_failure':election['steps'][-1]['conclusion']='failure'
            elif mutation=='selected_duplicate':election['steps'].append(copy.deepcopy(election['steps'][-1]))
            elif mutation=='selected_foreign':election['steps'][-1]['name']=election['steps'][-1]['name'].replace('owner=100','owner=99')
            elif mutation=='admission_missing':fixture.jobs['jobs'].remove(admission)
            elif mutation=='admission_skipped':admission['conclusion']='skipped'
            elif mutation=='admission_failure':admission['conclusion']='failure'
            else:duplicate=copy.deepcopy(admission);duplicate['id']=1001;fixture.jobs['jobs'].append(duplicate)
            fixture.jobs['total_count']=len(fixture.jobs['jobs'])
            with self.subTest(mutation=mutation),patch.object(cohort.time,'time',return_value=200),self.assertRaises(cohort.CohortError):
                cohort.recover_failed_owner(cohort._Reader(fixture.read_json,fixture.read_archive,Budget(),300),fixture.repository,100,fixture.sha,fixture.blob)
    def test_failed_owner_without_pending_follower_requires_terminal_backend_and_original_root(self):
        from unittest.mock import patch
        fixture = RawArtifactFixture()
        fixture.run.update(status="completed", conclusion="failure", created_at="1970-01-01T00:01:40Z")
        fixture.jobs["jobs"][1].update(status="completed", conclusion="failure")
        fixture.jobs["jobs"] += [{"id": 200+i, "run_id": 100, "head_sha": fixture.sha, "name": name,
                                 "status": "completed", "conclusion": "skipped", "steps": []}
                                for i, name in enumerate(sorted(cohort.MUTATING_JOBS))]
        fixture.jobs["total_count"] = len(fixture.jobs["jobs"])
        self.select_and_admit(fixture)
        fixture.data["repos/owner/repository/actions/runs/100/artifacts?per_page=100&page=1"] = {"total_count": len(fixture.metadata), "artifacts": list(fixture.metadata.values())}
        fixture.data["repos/owner/repository/actions/workflows/pr-governance-status-writer.yml/runs?created=%3E%3D1970-01-01T00%3A01%3A40Z&per_page=100&page=1"] = {"total_count": 0, "workflow_runs": []}
        def recover():
            reader = cohort._Reader(fixture.read_json, fixture.read_archive, Budget(), 300)
            return cohort.recover_failed_owner(reader, fixture.repository, 100, fixture.sha, fixture.blob)
        with patch.object(cohort.time, "time", return_value=200):
            result = recover()
            self.assertEqual(result["root_deadline_epoch"], 21200)
            self.assertEqual(result["owner"], 100)
            for conclusion in ("success", "cancelled", "timed_out", None):
                fixture.run["conclusion"] = conclusion
                with self.subTest(conclusion=conclusion), self.assertRaises(cohort.CohortError): recover()
            fixture.run["conclusion"] = "failure"
            fixture.jobs["jobs"][-1].update(status="in_progress", conclusion=None)
            with self.assertRaises(cohort.CohortError): recover()
            fixture.jobs["jobs"][-1].update(status="completed", conclusion="skipped")
            fixture.data["repos/owner/repository/actions/workflows/pr-governance-status-writer.yml/runs?created=%3E%3D1970-01-01T00%3A01%3A40Z&per_page=100&page=1"] = {"total_count": 101, "workflow_runs": []}
            with self.assertRaises(cohort.CohortError): recover()


    def test_recovery_native_stage_inherits_clock_and_covers_only_drained_predecessor(self):
        import copy
        from unittest.mock import patch
        fixture=RawArtifactFixture();fixture.run.update(status="completed",conclusion="failure",created_at="1970-01-01T00:01:40Z",event="issues")
        fixture.jobs['jobs'][1].update(status='completed',conclusion='failure')
        fixture.jobs['jobs'] += [{'id':200+i,'run_id':100,'head_sha':fixture.sha,'name':name,'status':'completed','conclusion':'skipped','steps':[]} for i,name in enumerate(sorted(cohort.MUTATING_JOBS))]
        fixture.jobs['total_count']=len(fixture.jobs['jobs'])
        self.select_and_admit(fixture)
        fixture.data['repos/owner/repository/actions/runs/100/artifacts?per_page=100&page=1']={'total_count':len(fixture.metadata),'artifacts':list(fixture.metadata.values())}
        endpoint='repos/owner/repository/actions/workflows/pr-governance-status-writer.yml/runs?created=%3E%3D1970-01-01T00%3A01%3A40Z&per_page=100&page=1'
        fixture.data[endpoint]={'total_count':0,'workflow_runs':[]}
        marker=cohort.RECOVERY_PREFIX+'owner=100 journal=4 digest='+fixture.metadata[4]['digest']+' root=21200'
        scope={'name':'Exclude unavailable fork sources before dispatcher lock','status':'completed','conclusion':'success','started_at':'1970-01-01T00:08:20Z','completed_at':'1970-01-01T00:10:00Z'}
        stage={'name':marker,'status':'completed','conclusion':'success'}
        newrun=dict(fixture.run,id=101,run_number=91,event='workflow_dispatch',status='in_progress',conclusion=None)
        fixture.data['repos/owner/repository/actions/runs/101']=newrun
        fixture.data['repos/owner/repository/actions/runs/101/attempts/1/jobs?per_page=100&page=1']={'total_count':1,'jobs':[{'id':1011,'run_id':101,'head_sha':fixture.sha,'name':'Preflight workflow_run governance source','status':'completed','conclusion':'success','steps':[scope,stage]}]}
        journal=dict(fixture.journal,producer_run_id=101)
        context={'header':dict(fixture.header,owner_dispatcher_run_id=101),'recovered_predecessor_ids':[100]}
        def clock():
            reader=cohort._Reader(fixture.read_json,fixture.read_archive,Budget(),1000)
            cohort.validate_journal_clock(journal,reader)
            self.assertEqual(reader.cache['recovered_predecessor_ids'],{100})
        def covered(run):
            return cohort.consumer_is_covered(context,run,read_json=fixture.read_json,read_archive=fixture.read_archive,budget=Budget(),deadline=1000)
        with patch.object(cohort.time,'time',return_value=600):
            clock();self.assertTrue(covered(fixture.run));self.assertFalse(covered(newrun))
            stage['name']=marker.replace('root=21200','root=21201')
            with self.assertRaises(cohort.CohortError):clock()
            self.assertFalse(covered(fixture.run));stage['name']=marker
            fixture.jobs['jobs'][-1].update(status='in_progress',conclusion=None)
            self.assertFalse(covered(fixture.run))
            with self.assertRaises(cohort.CohortError):clock()

    def test_failed_owner_child_backend_double_read_and_wait_use_original_deadline(self):
        from unittest.mock import patch
        fixture=RawArtifactFixture();fixture.run.update(status='completed',conclusion='failure',created_at='1970-01-01T00:01:40Z')
        fixture.jobs['jobs'][1].update(status='completed',conclusion='failure')
        fixture.jobs['jobs'] += [{'id':200+i,'run_id':100,'head_sha':fixture.sha,'name':name,'status':'completed','conclusion':'skipped','steps':[]} for i,name in enumerate(sorted(cohort.MUTATING_JOBS))]
        fixture.jobs['total_count']=len(fixture.jobs['jobs'])
        self.select_and_admit(fixture)
        fixture.data['repos/owner/repository/actions/runs/100/artifacts?per_page=100&page=1']={'total_count':len(fixture.metadata),'artifacts':list(fixture.metadata.values())}
        endpoint='repos/owner/repository/actions/workflows/pr-governance-status-writer.yml/runs?created=%3E%3D1970-01-01T00%3A01%3A40Z&per_page=100&page=1'
        child={'id':500,'run_attempt':1,'head_sha':fixture.sha,'head_branch':'master','path':'.github/workflows/pr-governance-status-writer.yml','event':'workflow_dispatch','repository':{'id':101,'full_name':fixture.repository},'display_title':'source=100 scope=early segment=1','status':'in_progress','conclusion':None}
        fixture.data[endpoint]={'total_count':1,'workflow_runs':[child]};fixture.data['repos/owner/repository/actions/runs/500']=child
        clock=[200]
        def release(delay):
            clock[0]+=delay;child.update(status='completed',conclusion='failure')
        with patch.object(cohort.time,'time',side_effect=lambda:clock[0]),patch.object(cohort.time,'sleep',side_effect=release):
            reader=cohort._Reader(fixture.read_json,fixture.read_archive,Budget(),300)
            proof=cohort.await_failed_owner_recovery(reader,fixture.repository,100,fixture.sha,fixture.blob)
            self.assertEqual(proof['root_deadline_epoch'],21200);self.assertEqual(clock[0],230)
            self.assertGreaterEqual(fixture.calls.count('repos/owner/repository/actions/runs/500'),3)
            child['head_sha']='d'*40
            with self.assertRaises(cohort.CohortError):cohort.recover_failed_owner(cohort._Reader(fixture.read_json,fixture.read_archive,Budget(),300),fixture.repository,100,fixture.sha,fixture.blob)
        with patch.object(cohort.time,'time',return_value=21201):
            with self.assertRaises(cohort.CohortError):cohort.recover_failed_owner(cohort._Reader(fixture.read_json,fixture.read_archive,Budget(),22000),fixture.repository,100,fixture.sha,fixture.blob)

    def test_failed_handoff_holds_election_until_own_backend_terminal(self):
        from unittest.mock import patch
        fixture=RawArtifactFixture();fixture.data['repos/owner/repository'].update(id=101,full_name=fixture.repository)
        fixture.run.update(status='in_progress',conclusion=None,created_at='1970-01-01T00:01:40Z')
        for name in ('Service bound cohort review sources','Preserve pending review sensor before direct dispatch','Preserve admitted sources before obsolete heavy preemption'):
            fixture.jobs['jobs'].append({'id':200+len(fixture.jobs['jobs']),'run_id':100,'head_sha':fixture.sha,'name':name,'status':'completed','conclusion':'failure' if name.startswith('Service') else 'skipped','steps':[]})
        fixture.jobs['total_count']=len(fixture.jobs['jobs'])
        endpoint='repos/owner/repository/actions/workflows/pr-governance-status-writer.yml/runs?created=%3E%3D1970-01-01T00%3A01%3A40Z&per_page=100&page=1'
        child={'id':500,'run_attempt':1,'head_sha':fixture.sha,'head_branch':'master','path':'.github/workflows/pr-governance-status-writer.yml','event':'workflow_dispatch','repository':{'id':101,'full_name':fixture.repository},'display_title':'source=100 scope=early segment=1','status':'in_progress','conclusion':None}
        fixture.data[endpoint]={'total_count':1,'workflow_runs':[child]};fixture.data['repos/owner/repository/actions/runs/500']=child
        clock=[200]
        def release(delay):clock[0]+=delay;child.update(status='completed',conclusion='failure')
        env={'GITHUB_REPOSITORY':fixture.repository,'GITHUB_RUN_ID':'100','GITHUB_RUN_ATTEMPT':'1','WORKFLOW_SHA':fixture.sha,'WORKFLOW_REF':fixture.repository+'/.github/workflows/pr-governance.yml@refs/heads/master','DEFAULT_BRANCH':'master','COHORT_ARTIFACT_ID':'1','COHORT_ARTIFACT_DIGEST':fixture.header_ref['artifact_digest']}
        with patch.dict('os.environ',env),patch.object(cohort.time,'time',side_effect=lambda:clock[0]),patch.object(cohort.time,'sleep',side_effect=release) as sleep,patch.object(cohort,'_write_outputs') as output:
            reader=cohort._Reader(fixture.read_json,fixture.read_archive,Budget(),300)
            with self.assertRaisesRegex(cohort.CohortError,'after a verified terminal drain'):cohort.await_handoff(reader)
            self.assertEqual(clock[0],230);sleep.assert_called_once_with(30);output.assert_not_called()
            self.assertGreaterEqual(fixture.calls.count('repos/owner/repository/actions/runs/500'),3)

    def test_failure_recovery_dispatch_is_nonblocking_exact_and_positive_failure_only(self):
        from unittest.mock import patch
        from types import SimpleNamespace
        fixture=RawArtifactFixture();fixture.data['repos/owner/repository'].update(id=101,full_name=fixture.repository)
        fixture.run.update(status='in_progress',conclusion=None)
        fixture.jobs['jobs'][1].update(status='completed',conclusion='failure')
        fixture.jobs['jobs'][1]['steps'].append({'name':cohort.SELECTED_PREFIX+'owner=100 artifact=1 digest='+fixture.header_ref['artifact_digest'],'status':'completed','conclusion':'success'})
        fixture.jobs['jobs'] += [{'id':200+i,'run_id':100,'head_sha':fixture.sha,'name':name,'status':'completed','conclusion':'skipped','steps':[]} for i,name in enumerate(sorted(cohort.MUTATING_JOBS))]
        admission={'id':999,'run_id':100,'head_sha':fixture.sha,'name':cohort.ADMISSION_JOB,'status':'completed','conclusion':'success','steps':[]}
        fixture.jobs['jobs'].append(admission);fixture.jobs['total_count']=len(fixture.jobs['jobs'])
        env={'GITHUB_REPOSITORY':fixture.repository,'GITHUB_RUN_ID':'100','GITHUB_RUN_ATTEMPT':'1','WORKFLOW_SHA':fixture.sha,'WORKFLOW_REF':fixture.repository+'/.github/workflows/pr-governance.yml@refs/heads/master','DEFAULT_BRANCH':'master'}
        with patch.dict('os.environ',env),patch.object(cohort.time,'time',return_value=200),patch('subprocess.run',return_value=SimpleNamespace(returncode=0)) as dispatch,patch.object(cohort,'_write_outputs'):
            reader=cohort._Reader(fixture.read_json,fixture.read_archive,Budget(),300)
            cohort.dispatch_recovery(reader)
            self.assertEqual(dispatch.call_args.args[0][-4:],['-f','ref=master','-f','inputs[recovery_owner]=100'])
            self.assertIn('repos/owner/repository/actions/workflows/pr-governance.yml/dispatches',dispatch.call_args.args[0])
            admission['conclusion']='failure'
            with self.assertRaises(cohort.CohortError):cohort.dispatch_recovery(cohort._Reader(fixture.read_json,fixture.read_archive,Budget(),300))
            self.assertEqual(dispatch.call_count,1)


class CohortConditionalTransportTests(unittest.TestCase):
    def test_gh_exit_one_304_requires_exact_endpoint_etag_and_empty_body(self):
        from unittest.mock import patch
        from types import SimpleNamespace
        endpoint = "repos/owner/repository/actions/runs/100"
        for code in (0, 1):
            transport = cohort._Transport("secret", Budget())
            replies = [SimpleNamespace(returncode=0, stdout='HTTP/2.0 200 OK\nETag: "one"\n\n{"id":100}'),
                       SimpleNamespace(returncode=code, stdout='HTTP/2.0 304 Not Modified\nETag: "one"\n\n')]
            with patch("subprocess.run", side_effect=replies) as run:
                self.assertEqual(transport.json(endpoint, timeout=20), {"id": 100})
                self.assertEqual(transport.json(endpoint, timeout=20), {"id": 100})
                self.assertIn('If-None-Match: "one"', run.call_args.args[0])
                self.assertEqual(transport.primary_reads, 1)
        for code, etag, body, requested in ((2, 'ETag: "one"', '', endpoint), (1, '', '', endpoint),
                (1, 'ETag: "two"', '', endpoint), (1, 'ETag: "one"\nETag: "one"', '', endpoint),
                (1, 'ETag: "one"', ' ', endpoint), (1, 'ETag: "one"', '', endpoint+'1')):
            transport = cohort._Transport("secret", Budget()); transport.etags[endpoint] = ('"one"', {"id":100})
            with patch("subprocess.run", return_value=SimpleNamespace(returncode=code, stdout='HTTP/2.0 304 Not Modified\n'+etag+'\n\n'+body)):
                with self.subTest(code=code, etag=etag, body=body, endpoint=requested), self.assertRaises(cohort.CohortError):
                    transport.json(requested, timeout=20)

    def test_transport_rejects_unbounded_nonfinite_or_boolean_timeout_before_io(self):
        from unittest.mock import patch
        for timeout in (0, -1, 20.1, float('inf'), float('nan'), True, None, '20'):
            transport = cohort._Transport("secret", Budget())
            with patch("subprocess.run") as request, patch.object(cohort, 'read_artifact_archive') as archive:
                with self.subTest(timeout=timeout), self.assertRaises(cohort.CohortError): transport.json('repos/owner/repository', timeout=timeout)
                with self.subTest(timeout=timeout), self.assertRaises(cohort.CohortError): transport.archive('repos/owner/repository/actions/artifacts/1/zip', timeout=timeout)
                request.assert_not_called(); archive.assert_not_called()


class CohortCLIReplayTests(unittest.TestCase):
    def test_four_hundred_notifications_survive_real_producer_select_admit_handoff(self):
        self.replay_notifications()

    def test_bodyless_started_and_unstarted_sources_do_not_block_valid_cli_cohort(self):
        self.replay_notifications(bodyless=True)

    def test_six_hundred_started_sources_elect_with_native_transport_and_role_budget(self):
        self.replay_notifications(count=600,started=600,native=True,api_latency=0.25)

    def test_six_hundred_late_started_sources_resolve_original_clock_within_role_budget(self):
        self.replay_notifications(count=600,started=600,native=True,late_clock=True,api_latency=0.25)

    def test_unrelated_deleted_fork_does_not_block_native_valid_source_cohort(self):
        self.replay_notifications(count=2,started=2,native=True,unrelated=True)

    def replay_notifications(self, *, bodyless=False, count=200, started=2, native=False, late_clock=False, api_latency=0, unrelated=False):
        import base64, copy, os, tempfile, urllib.parse
        from contextlib import nullcontext
        from pathlib import Path
        from types import SimpleNamespace
        from unittest.mock import patch
        raw = RawArtifactFixture()
        prefix = "repos/" + raw.repository
        raw.data[prefix].update(id=101, full_name=raw.repository)
        metadata, archives, raw_runs, raw_jobs, sources, pulls = {}, {}, {}, {}, {}, {}
        repo = {"id": 101, "full_name": raw.repository}
        next_artifact = [1000]
        native_outputs, requests = [], []
        def upload(payload, producer, segment=None):
            identifier = next_artifact[0]; next_artifact[0] += 1
            kind = payload["kind"]
            member = payload.get("member")
            name = (f"krr-governance-source-{member['source_run_id']}-attempt-1" if member is not None
                    else f"krr-governance-source-driver-{producer}-attempt-1") if kind == "source" else f"krr-governance-{kind}-{producer}-attempt-1" + (f"-{segment}" if segment else "")
            packed = io.BytesIO()
            with zipfile.ZipFile(packed, "w", compression=zipfile.ZIP_DEFLATED) as archive: archive.writestr(zipfile.ZipInfo("binding.json", date_time=(2000,1,1,0,0,0)), cohort.canonical(payload))
            data = packed.getvalue(); digest = "sha256:" + hashlib.sha256(data).hexdigest()
            archives[identifier] = data
            metadata[identifier] = {"id": identifier, "name": name, "expired": False, "digest": digest,
                "size_in_bytes": len(data), "workflow_run": {"id": producer, "repository_id": 101, "head_repository_id": 101, "head_sha": raw.sha}}
            creator = next(job for job in raw_jobs[producer] if job["name"] == (cohort.PRODUCER_JOB if kind == "source" else cohort.ELECTION_JOB))
            stage = creator["steps"]
            stage.extend([{"name": cohort.PAYLOAD_PREFIX + kind + " sha256=" + hashlib.sha256(cohort.canonical(payload)).hexdigest(), "number": len(stage)+1, "status": "completed", "conclusion": "success"},
                          {"name": "Upload immutable " + name, "number": len(stage)+2, "status": "completed", "conclusion": "success"}])
            if kind == "source": stage.append({"name": cohort.JOURNAL_PREFIX+base64.b64encode(cohort.canonical(payload)).decode(), "number": len(stage)+1, "status": "completed", "conclusion": "success"})
            native_outputs.append(identifier)
            return {"artifact_id": identifier, "artifact_digest": digest}
        def producer(identifier):
            raw_runs[identifier] = dict(raw.run, id=identifier, run_number=identifier, status="in_progress", conclusion=None)
            raw_jobs[identifier] = [{"id": identifier*100+index, "run_id": identifier, "head_sha": raw.sha, "name": name,
                                    "status": "in_progress" if name == cohort.ELECTION_JOB else "completed", "conclusion": None if name == cohort.ELECTION_JOB else "success", "steps": []}
                                   for index, name in enumerate((cohort.PRODUCER_JOB, cohort.ELECTION_JOB, "Preflight workflow_run governance source"), 1)]
            raw_jobs[identifier][-1]["steps"] = copy.deepcopy(raw.steps["Preflight workflow_run governance source"])
        for offset in range(count):
            identifier, number = 1000+offset, offset+1
            member = dict(raw.member, source_run_id=identifier, source_run_number=offset+1, pr_number=number)
            pull = {"number": number, "state": "open", "draft": False, "body": "body", "base": {"ref": "master", "sha": "b"*40, "repo": repo}, "head": {"sha": "c"*40, "repo": repo}}
            source = {"id": identifier, "run_attempt": 1, "workflow_id": 9, "run_number": offset+1, "name": "PR governance review sensor", "event": "pull_request_review", "head_sha": "c"*40, "path": ".github/workflows/pr-governance-review-events.yml", "repository": repo, "head_repository": repo, "status": "in_progress" if offset < started else "queued", "conclusion": None, "pull_requests": [pull]}
            sources[identifier], pulls[number] = source, pull
            raw_jobs[identifier] = [{"id": identifier*100, "run_id": identifier, "head_sha": source["head_sha"], "name": "KRR / PR governance review latch", "status": "in_progress" if offset < started else "queued", "conclusion": None,
                "steps": [{"number": 5, "name": "Await matching trusted governance Check Run", "started_at": "1970-01-01T00:01:40Z", "status": "in_progress", "conclusion": None}] if offset < started and not late_clock else []}]
            if native and not late_clock and offset < started:
                raw_jobs[identifier][0]["steps"][0]["started_at"]=f"1970-01-01T00:01:40.{count-offset:06d}Z"
        def read(endpoint, *, timeout):
            requests.append(endpoint)
            self.assertGreater(timeout, 0); self.assertLessEqual(timeout, 20)
            if "/actions/artifacts/" in endpoint: return copy.deepcopy(metadata[int(endpoint.rsplit("/",1)[1])])
            if "/actions/artifacts?" in endpoint:
                name = urllib.parse.parse_qs(urllib.parse.urlsplit(endpoint).query)["name"][0]
                entries = [item for item in metadata.values() if item["name"] == name]
                return {"total_count": len(entries), "artifacts": copy.deepcopy(entries)}
            if "/actions/workflows/pr-governance-review-events.yml/runs?" in endpoint:
                query = urllib.parse.parse_qs(urllib.parse.urlsplit(endpoint).query); state=query["status"][0]; page=int(query["page"][0])
                entries = [source for source in sources.values() if source["status"] == state]
                return {"total_count": len(entries), "workflow_runs": copy.deepcopy(entries[(page-1)*100:page*100])}
            if "/attempts/1/jobs?" in endpoint:
                identifier=int(endpoint.split("/runs/")[1].split("/")[0]); jobs=raw_jobs[identifier]
                return {"total_count": len(jobs), "jobs": copy.deepcopy(jobs)}
            if "/actions/runs/" in endpoint: return copy.deepcopy({**raw_runs, **sources}[int(endpoint.rsplit("/",1)[1])])
            if "/pulls?" in endpoint:
                page=int(urllib.parse.parse_qs(urllib.parse.urlsplit(endpoint).query)["page"][0])
                return copy.deepcopy(list(pulls.values())[(page-1)*100:page*100])
            if "/pulls/" in endpoint: return copy.deepcopy(pulls[int(endpoint.rsplit("/",1)[1])])
            return raw.read_json(endpoint, timeout=timeout)
        class Transport:
            primary_reads = 0
            def __init__(self, token, budget): pass
            json = staticmethod(read)
            @staticmethod
            def archive(endpoint, *, timeout): return archives[int(endpoint.split("/")[-2])]
        env = {"GITHUB_REPOSITORY": raw.repository, "GITHUB_RUN_ATTEMPT": "1", "WORKFLOW_SHA": raw.sha,
               "WORKFLOW_REF": raw.repository+"/.github/workflows/pr-governance.yml@refs/heads/master", "DEFAULT_BRANCH": "master", "GH_TOKEN": "fixture", "COHORT_ORIGINAL_ROOT_DEADLINE": "21200", "COHORT_VALID": "true", "COHORT_SENSOR_SOURCE": "true"}
        initial_directory = os.getcwd()
        clock,selecting=[200.0],[False]
        def native_json(args, **options):
            value=read(args[5],timeout=options["timeout"])
            if selecting[0]:clock[0]+=api_latency
            return SimpleNamespace(returncode=0,stdout="HTTP/2.0 200 OK\n\n"+json.dumps(value))
        def native_archive(endpoint,token,timeout):
            if selecting[0]:clock[0]+=api_latency*2
            return archives[int(endpoint.split("/")[-2])]
        with tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ, env), patch.object(cohort.time, "time", side_effect=lambda:clock[0]), \
                (nullcontext() if native else patch.object(cohort, "_Transport", Transport)), \
                patch("subprocess.run",side_effect=native_json), \
                patch.object(cohort,"read_artifact_archive",side_effect=native_archive):
            def run(mode, owner, directory, extra=None):
                directory.mkdir(exist_ok=True); os.chdir(directory)
                output = directory/'outputs'; output.write_text('')
                selecting[0]=mode=="select"
                try:
                    with patch.dict(os.environ, dict({"COHORT_MODE": mode, "GITHUB_RUN_ID": str(owner), "GITHUB_OUTPUT": str(output)}, **(extra or {}))): cohort.main()
                finally:selecting[0]=False
                return dict(line.split('=',1) for line in output.read_text().splitlines())
            try:
                for offset, source in enumerate(sources.values()):
                    for notification in range(2):
                        identifier = 10000+offset*2+notification; producer(identifier)
                        hint = dict(raw.member, source_run_id=source["id"], source_run_number=source["run_number"], pr_number=offset+1, latch_started_at=None, source_deadline_epoch=None)
                        directory = Path(temporary)/str(identifier)
                        run('producer', identifier, directory, {"COHORT_MEMBER_HINT": json.dumps(hint)})
                        payload=json.loads((directory/'cohort-journal/binding.json').read_text()); upload(payload, identifier)
                self.assertEqual(len(native_outputs), count*2)
                self.assertEqual({item['name'] for item in metadata.values()}, {f'krr-governance-source-{identifier}-attempt-1' for identifier in sources})
                if late_clock:
                    for jobs in raw_jobs.values():
                        if jobs[0]["name"] == "KRR / PR governance review latch":
                            jobs[0]["steps"]=[{"number":5,"name":"Await matching trusted governance Check Run",
                                "started_at":"1970-01-01T00:03:20Z","status":"in_progress","conclusion":None}]
                if unrelated:
                    foreign=copy.deepcopy(pulls[1]);foreign.update(number=count+1,body=None)
                    foreign["head"]["repo"]=None
                    pulls[count+1]=foreign
                if bodyless:
                    for offset, body in enumerate((None,"",None,"")):
                        identifier,number=2000+offset,201+offset
                        source=copy.deepcopy(sources[1000 if offset<2 else 1002])
                        source.update(id=identifier,run_number=identifier)
                        source["pull_requests"][0].update(number=number,body=body)
                        sources[identifier],pulls[number]=source,copy.deepcopy(source["pull_requests"][0])
                        raw_jobs[identifier]=copy.deepcopy(raw_jobs[1000 if offset<2 else 1002])
                owner=90000; producer(owner); directory=Path(temporary)/str(owner)
                run('producer', owner, directory, {"COHORT_MEMBER_HINT": "null", "COHORT_SENSOR_SOURCE": "false"})
                own=upload(json.loads((directory/'cohort-journal/binding.json').read_text()), owner)
                requests.clear()
                outputs=run('select', owner, directory, {"COHORT_OWN_JOURNAL_ID": str(own['artifact_id']), "COHORT_OWN_JOURNAL_DIGEST": own['artifact_digest']})
                state=json.loads((directory/'cohort-selection/state.json').read_text())
                selected=sorted(sorted(sources,key=lambda identifier:(raw_jobs[identifier][0]["steps"][0]["started_at"],identifier))[:len(state['member_ids'])]) if native else [1000,1001]
                self.assertEqual(state['member_ids'], selected); self.assertEqual(outputs['batch_count'], '1')
                if native:
                    self.assertGreater(len(selected),0);self.assertLessEqual(len(selected),200)
                    self.assertLessEqual(int(outputs['cohort_read_attempts']),900)
                    self.assertEqual(int(outputs['cohort_primary_reads']),len(requests))
                    self.assertEqual(sum("/pulls/" in endpoint for endpoint in requests),len(selected))
                    self.assertEqual(sum("/pulls?" in endpoint for endpoint in requests),2*((count+int(unrelated))//100+1))
                    self.assertEqual(sum("/actions/artifacts?" in endpoint for endpoint in requests),len(selected))
                    self.native_selection_stats={"selected":len(selected),"role":int(outputs['cohort_read_attempts']),
                        "primary":int(outputs['cohort_primary_reads']),"jobs":sum("/jobs?" in endpoint for endpoint in requests),
                        "elapsed":clock[0]-200}
                    self.assertEqual(self.native_selection_stats["elapsed"],int(outputs['cohort_read_attempts'])*api_latency)
                    self.assertLess(clock[0],min(member["source_deadline_epoch"] for member in state["members"].values()))
                if bodyless:
                    self.assertFalse(any(f"krr-governance-source-{identifier}-attempt" in endpoint
                                         for identifier in range(2000,2004) for endpoint in requests))
                    self.assertFalse(any(f"/actions/runs/{identifier}/" in endpoint
                                         for identifier in range(2000,2004) for endpoint in requests))
                native={}
                for kind in ('members','aliases','batch'):
                    reference=upload(json.loads((directory/f'cohort-selection/{kind}-1/binding.json').read_text()), owner, 1)
                    native[f'COHORT_{kind.upper()}_1_ID']=str(reference['artifact_id']); native[f'COHORT_{kind.upper()}_1_DIGEST']=reference['artifact_digest'][7:]
                run('build-header', owner, directory, native)
                header=upload(json.loads((directory/'cohort-header/binding.json').read_text()), owner)
                raw_jobs[owner][1]['steps'].append({'name':cohort.SELECTED_PREFIX+f"owner={owner} artifact={header['artifact_id']} digest={header['artifact_digest']}", 'number':20,'status':'completed','conclusion':'success'})
                admitted=run('admission', owner, directory)
                self.assertEqual(admitted['source_ids'],json.dumps(selected,separators=(',',':'))); self.assertEqual(admitted['root_deadline_epoch'],'21200')
                for name in ('Service bound cohort review sources','Preserve pending review sensor before direct dispatch','Preserve admitted sources before obsolete heavy preemption'):
                    raw_jobs[owner].append({'id':9000010+len(raw_jobs[owner]),'run_id':owner,'head_sha':raw.sha,'name':name,'status':'completed','conclusion':'success','steps':[]})
                handoff=run('await-handoff', owner, directory, {'COHORT_ARTIFACT_ID':str(header['artifact_id']),'COHORT_ARTIFACT_DIGEST':header['artifact_digest']})
                self.assertEqual(handoff['owner'],str(owner))
                self.assertEqual(len([item for item in metadata.values() if item['name'].startswith('krr-governance-source-') and '-driver-' not in item['name']]),count*2)
                for item in metadata.values():
                    if item['name'].startswith('krr-governance-source-') and '-driver-' not in item['name']:
                        journal=json.loads(cohort.decode_archive(archives[item['id']]))
                        self.assertEqual(journal['root_deadline_epoch'],21200)
                        if journal['member']['source_run_id']>=1000+started: self.assertIsNone(journal['member']['source_deadline_epoch'])
                if late_clock:
                    self.assertTrue(all(member["latch_started_at"] == "1970-01-01T00:03:20Z"
                                        and member["source_deadline_epoch"] == 5600 for member in state["members"].values()))
            finally: os.chdir(initial_directory)


class CohortSensorPaginationTests(unittest.TestCase):
    def sensor(self, identifier):
        repo={"id":101,"full_name":"owner/repository"}
        pull={"number":7,"state":"open","draft":False,"body":"body","base":{"ref":"master","sha":"b"*40,"repo":repo},"head":{"sha":"c"*40,"repo":repo}}
        return {"id":identifier,"status":"queued","conclusion":None,"name":"PR governance review sensor",
                "event":"pull_request_review","run_attempt":1,"workflow_id":9,"run_number":identifier,
                "head_sha":"c"*40,"path":".github/workflows/pr-governance-review-events.yml",
                "repository":repo,"head_repository":repo,"pull_requests":[pull]}

    def snapshot(self, count, mutation=None):
        import copy, urllib.parse
        calls=[]
        entries=[self.sensor(identifier) for identifier in range(1,count+1)]
        def request(endpoint):
            if '/pulls/' in endpoint: return copy.deepcopy(entries[0]['pull_requests'][0])
            query=urllib.parse.parse_qs(urllib.parse.urlsplit(endpoint).query);state=query['status'][0];page=int(query['page'][0]);calls.append((state,page))
            selected=entries if state=='queued' else []
            result={'total_count':len(selected),'workflow_runs':copy.deepcopy(selected[(page-1)*100:page*100])}
            if mutation is not None: mutation(result, state, page, calls)
            return result
        class Reader:
            pass
        reader=Reader();reader.request=request
        return lambda:cohort.active_source_snapshot(reader,'owner/repository',101,'master'),calls

    def test_one_status_two_hundred_uses_page_two_and_stable_anchor(self):
        for count in (100,200,201,600):
            run,calls=self.snapshot(count)
            self.assertEqual(len(run()),count)
            self.assertEqual(calls.count(('queued',1)),2)
            self.assertEqual(calls.count(('queued',2)),int(count>100))
        run,_=self.snapshot(601)
        with self.assertRaises(cohort.CohortError):run()

    def test_duplicate_page_and_count_drift_fail_closed(self):
        def duplicate(result,state,page,calls):
            if state=='queued' and page==2: result['workflow_runs'][0]['id']=1
        def count(result,state,page,calls):
            if state=='queued' and page==2:result['total_count']=199
        for mutation in (duplicate,count):
            run,_=self.snapshot(200,mutation)
            with self.subTest(mutation=mutation),self.assertRaises(cohort.CohortError):run()

    def test_anchor_type_drift_is_not_stable_integer_identity(self):
        for changed in (True, 1.0):
            def mutate(result, state, page, calls):
                if state == "queued" and page == 1 and calls.count(("queued", 1)) % 2 == 0:
                    result["workflow_runs"][0]["id"] = changed
            run, calls = self.snapshot(1, mutate)
            with self.subTest(changed=changed), self.assertRaises(cohort.CohortError):
                run()
            self.assertEqual(calls.count(("queued", 1)), 8)

    def test_later_page_count_requires_exact_integer_type(self):
        for count, changed in ((101, 101.0), (1, True)):
            def mutate(result, state, page, calls):
                if state == "queued" and page == (2 if count > 100 else 1):
                    result["total_count"] = changed
            run, _ = self.snapshot(count, mutate)
            with self.subTest(count=count, changed=changed), self.assertRaises(cohort.CohortError):
                run()

    def test_first_page_transition_stabilizes_bounded_and_churn_rejects(self):
        def once(result,state,page,calls):
            if state=='queued' and page==1 and calls.count(('queued',1))==2:result['workflow_runs'][0]['status']='in_progress'
        run,calls=self.snapshot(200,once)
        self.assertEqual(len(run()),200);self.assertEqual(calls.count(('queued',1)),4)
        def churn(result,state,page,calls):
            if state=='queued' and page==1 and calls.count(('queued',1))%2==0:result['workflow_runs'][0]['status']='in_progress'
        run,calls=self.snapshot(200,churn)
        with self.assertRaises(cohort.CohortError):run()
        self.assertEqual(calls.count(('queued',1)),8)


class CohortEligibilityOverflowTests(unittest.TestCase):
    def fixture(self, count, *, unstarted=(), current_change=None):
        import copy
        from datetime import datetime, timezone
        sources, pulls, members, journals, requests, clocks = {}, {}, {}, {}, [], {}
        repo = {"id": 101, "full_name": "owner/repository"}
        for identifier in range(1, count + 1):
            source = CohortSensorPaginationTests().sensor(identifier)
            source["pull_requests"][0]["number"] = identifier
            source["status"] = "in_progress"
            pull = copy.deepcopy(source["pull_requests"][0])
            started = datetime.fromtimestamp(count + 101 - identifier, timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
            member = {"source_run_id":identifier,"source_run_attempt":1,"source_workflow_id":9,
                "source_run_number":identifier,"source_event":"pull_request_review","repository":"owner/repository",
                "repository_id":101,"pr_number":identifier,"base_ref":"master","base_sha":"b"*40,
                "head_sha":"c"*40,"pr_body_sha256":hashlib.sha256(b"body").hexdigest(),
                "latch_started_at":started,"source_deadline_epoch":count+101-identifier+5400}
            sources[identifier], pulls[identifier], members[identifier] = source, pull, member
            clocks[identifier] = [] if identifier in unstarted else [{"run_id":identifier,"head_sha":"c"*40,
                "name":"KRR / PR governance review latch","steps":[{"name":"Await matching trusted governance Check Run",
                "number":5,"status":"in_progress","started_at":started}]}]
            journals[identifier] = {"member":member,"root_deadline_epoch":21200}
        class Reader:
            cache = {}
            read_json = None
            budget = None
            deadline = 21200
            def request(self, endpoint):
                requests.append(endpoint)
                if "/pulls?" in endpoint:
                    import urllib.parse
                    page=int(urllib.parse.parse_qs(urllib.parse.urlsplit(endpoint).query)["page"][0])
                    rows=copy.deepcopy(list(pulls.values()))
                    if current_change:
                        for pull in rows:current_change(pull["number"],pull)
                    rows=[pull for pull in rows if pull.get("state") != "closed"]
                    return rows[(page-1)*100:page*100]
                if "/pulls/" in endpoint:
                    identifier = int(endpoint.rsplit("/", 1)[1])
                    pull = copy.deepcopy(pulls[identifier])
                    if current_change: current_change(identifier, pull)
                    return pull
                if "/actions/artifacts?" in endpoint:
                    identifier = int(endpoint.split("krr-governance-source-")[1].split("-attempt")[0])
                    return {"total_count":1,"artifacts":[{"id":identifier,"digest":"sha256:"+"a"*64}]}
                raise AssertionError(endpoint)
            def jobs(self, repository, identifier):
                requests.append("jobs:"+str(identifier))
                return copy.deepcopy(clocks[identifier])
            def artifact(self, *args, **kwargs): return ({"root_deadline_epoch":21200}, None)
        def journal(reader, metadata, identifier, *args):
            return copy.deepcopy(journals[identifier]), {"id":identifier+1000,"run_number":identifier}
        return sources, pulls, members, journals, requests, Reader(), journal

    def select(self, fixture):
        import os, tempfile
        from pathlib import Path
        from unittest.mock import patch
        sources, _, _, _, _, reader, journal = fixture
        initial = os.getcwd()
        with tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ, {"DEFAULT_BRANCH":"master",
                "COHORT_OWN_JOURNAL_ID":"9000","COHORT_OWN_JOURNAL_DIGEST":"sha256:"+"a"*64}), \
                patch.object(cohort, "_producer_context", return_value=("owner/repository",101,9000,"d"*40,"e"*40)), \
                patch.object(cohort, "active_source_snapshot", return_value=sources), \
                patch.object(cohort, "validate_journal_clock"), patch.object(cohort, "_journal_from_metadata", side_effect=journal), \
                patch.object(cohort, "_write_outputs"), patch.object(cohort.time, "time", return_value=1000):
            try:
                os.chdir(temporary)
                cohort.select_cohort(reader)
                return json.loads(Path("cohort-selection/state.json").read_text())
            finally: os.chdir(initial)

    def test_actual_snapshot_excludes_positive_closed_and_draft_without_journal(self):
        import copy, urllib.parse
        for change in ({"state":"closed"}, {"draft":True}):
            source = CohortSensorPaginationTests().sensor(1)
            pull = copy.deepcopy(source["pull_requests"][0]); pull.update(change)
            class Reader:
                def request(self, endpoint):
                    if "/pulls/" in endpoint: return pull
                    state = urllib.parse.parse_qs(urllib.parse.urlsplit(endpoint).query)["status"][0]
                    return {"total_count":int(state=="queued"),"workflow_runs":[source] if state=="queued" else []}
            with self.subTest(change=change):
                self.assertEqual(cohort.active_source_snapshot(Reader(),"owner/repository",101,"master"), {})

    def test_actual_selection_caps_201_and_600_by_original_deadline_and_preserves_successor(self):
        import copy
        for count in (201,600):
            fixture = self.fixture(count)
            original = copy.deepcopy(fixture[3])
            state = self.select(fixture)
            selected = list(range(count-199,count+1))
            self.assertEqual(state["member_ids"], selected)
            self.assertEqual(state["batch_count"],4)
            self.assertEqual(set(int(endpoint.split("krr-governance-source-")[1].split("-attempt")[0]) for endpoint in fixture[4] if "/actions/artifacts?" in endpoint),set(selected))
            self.assertEqual(fixture[3],original)
            for identifier in selected: fixture[0].pop(identifier)
            fixture[4].clear()
            successor = self.select(fixture)
            remaining = count-200
            expected = list(range(max(1,remaining-199),remaining+1))
            self.assertEqual(successor["member_ids"],expected)
            self.assertEqual(fixture[3],original)
            for identifier in expected:
                self.assertEqual(successor["members"][str(identifier)]["source_deadline_epoch"],original[identifier]["member"]["source_deadline_epoch"])
            for identifier in expected: fixture[0].pop(identifier)
            if fixture[0]:
                last = self.select(fixture)
                self.assertEqual(last["member_ids"],list(range(1,201)))
                self.assertEqual(fixture[3],original)

    def test_changed_current_pr_is_excluded_before_missing_journal(self):
        for change in ({"state":"closed"},{"draft":True},{"head":{"sha":"f"*40,"repo":{"id":101,"full_name":"owner/repository"}}}):
            fixture = self.fixture(2,current_change=lambda identifier,pull:pull.update(change) if identifier==1 else None)
            fixture[3].pop(1)
            with self.subTest(change=change):
                self.assertEqual(self.select(fixture)["member_ids"],[2])
                self.assertFalse(any("krr-governance-source-1-attempt" in endpoint for endpoint in fixture[4]))

    def test_current_pr_malformed_and_observation_failure_are_not_ineligibility(self):
        for change in ({"state":"unknown"},{"draft":None},{"number":True},{"head":None},
                       {"head":{"sha":"c"*40,"repo":None}},{"body":"\0"}):
            fixture=self.fixture(1,current_change=lambda identifier,pull:pull.update(change))
            with self.subTest(change=change),self.assertRaises(cohort.CohortError): self.select(fixture)
        fixture=self.fixture(1)
        fixture[5].request=lambda endpoint: (_ for _ in ()).throw(cohort.CohortError("API observation failed"))
        with self.assertRaises(cohort.CohortError):self.select(fixture)

    def test_unstarted_overflow_never_creates_clock_or_reads_journal(self):
        fixture=self.fixture(600,unstarted=range(1,401))
        state=self.select(fixture)
        self.assertEqual(state["member_ids"],list(range(401,601)))
        self.assertEqual(sum("/actions/artifacts?" in endpoint for endpoint in fixture[4]),200)


    def test_current_positive_foreign_and_default_changes_are_non_ack_exclusions(self):
        changes=({"base":{"ref":"other","sha":"b"*40,"repo":{"id":101,"full_name":"owner/repository"}}},
                 {"head":{"sha":"c"*40,"repo":{"id":102,"full_name":"owner/foreign"}}})
        for change in changes:
            fixture=self.fixture(1,current_change=lambda identifier,pull:pull.update(change))
            fixture[3].clear()
            with self.subTest(change=change):
                self.assertEqual(self.select(fixture)["member_ids"],[])
                self.assertFalse(any("/actions/artifacts?" in endpoint for endpoint in fixture[4]))

    def test_source_internal_head_drift_and_current_body_drift_fail_closed(self):
        fixture=self.fixture(1)
        fixture[0][1]["head_sha"]="f"*40
        with self.assertRaises(cohort.CohortError):self.select(fixture)
        fixture=self.fixture(1,current_change=lambda identifier,pull:pull.update(body="changed"))
        with self.assertRaises(cohort.CohortError):self.select(fixture)

    def test_mixed_status_union_uses_existing_six_hundred_bound(self):
        import copy,urllib.parse
        for count in (201,600,601):
            entries=[CohortSensorPaginationTests().sensor(identifier) for identifier in range(1,count+1)]
            for source in entries:
                source["status"]="queued" if source["id"]%2 else "in_progress"
            class Reader:
                def request(self,endpoint):
                    if "/pulls/" in endpoint:return copy.deepcopy(entries[0]["pull_requests"][0])
                    query=urllib.parse.parse_qs(urllib.parse.urlsplit(endpoint).query)
                    state,page=query["status"][0],int(query["page"][0])
                    chosen=[source for source in entries if source["status"]==state]
                    return {"total_count":len(chosen),"workflow_runs":copy.deepcopy(chosen[(page-1)*100:page*100])}
            with self.subTest(count=count):
                if count<=600:self.assertEqual(len(cohort.active_source_snapshot(Reader(),"owner/repository",101,"master")),count)
                else:
                    with self.assertRaises(cohort.CohortError):cohort.active_source_snapshot(Reader(),"owner/repository",101,"master")


    def test_existing_role_budget_elects_bounded_members_and_preserves_overflow_journals(self):
        import copy
        fixture=self.fixture(600)
        original=copy.deepcopy(fixture[3])
        budget=cohort.ReadBudget(900)
        request,jobs=fixture[5].request,fixture[5].jobs
        def bounded_request(endpoint):
            budget.charge()
            return request(endpoint)
        def bounded_jobs(repository,identifier):
            budget.charge()
            return jobs(repository,identifier)
        fixture[5].request=bounded_request;fixture[5].jobs=bounded_jobs
        fixture[5].budget=budget
        selected=self.select(fixture)["member_ids"]
        self.assertGreater(len(selected),0)
        self.assertLessEqual(budget.used,900)
        self.assertEqual(fixture[3],original)
        self.assertEqual(sum("/actions/artifacts?" in endpoint for endpoint in fixture[4]),len(selected))


class CohortDeferredFollowerTests(unittest.TestCase):
    def fixture(self, *, queued=False, retired=None):
        import copy
        fixture,context,run,jobs=CohortFollowerAndHandoffTests().follower()
        context["header"]["member_ids"]=[12]
        context["members_by_id"]={}
        run["display_title"]="sensor=11 action=completed"
        source=fixture.data["repos/owner/repository/actions/runs/11"]
        source.update(status="queued" if queued else "in_progress",conclusion=None)
        latch=fixture.data["repos/owner/repository/actions/runs/11/attempts/1/jobs?per_page=100&page=1"]["jobs"][0]
        latch.update(status=source["status"],conclusion=None)
        if queued:
            journal=copy.deepcopy(fixture.journal);journal["producer_run_id"]=99
            journal["member"].update(latch_started_at=None,source_deadline_epoch=None)
            fixture.artifact(6,journal)
            fixture.metadata[6]["workflow_run"]["id"]=99
            jobs[0]["steps"]=fixture.steps[cohort.PRODUCER_JOB][-2:]
            fixture.data["repos/owner/repository/actions/runs/99/artifacts?per_page=100&page=1"]={"total_count":1,"artifacts":[fixture.metadata[6]]}
            latch["steps"]=[]
        else:latch["steps"][0].update(status="in_progress",conclusion=None)
        pull=copy.deepcopy(fixture.data["repos/owner/repository/pulls/7"])
        if retired:pull.update(retired)
        fixture.data["repos/owner/repository/pulls/7"]=pull
        return fixture,context,run,jobs

    def covered(self,fixture,context,run):
        from unittest.mock import patch
        with patch.object(cohort.time,"time",return_value=200):
            return cohort.consumer_is_deferred(context,run,read_json=fixture.read_json,
                read_archive=fixture.read_archive,budget=Budget(),deadline=300)

    def test_original_started_and_nullable_queued_journals_are_deferred_without_ack_authority(self):
        import copy
        for queued in (False,True):
            fixture,context,run,jobs=self.fixture(queued=queued)
            before=copy.deepcopy(context)
            archive=fixture.archives[6]
            with self.subTest(queued=queued):
                self.assertTrue(self.covered(fixture,context,run))
                self.assertEqual(context,before);self.assertEqual(fixture.archives[6],archive)
                self.assertFalse(cohort.consumer_is_covered(context,run,read_json=fixture.read_json,
                    read_archive=fixture.read_archive,budget=Budget(),deadline=300))

    def test_positive_retired_closed_draft_and_stale_head_can_only_remove_nonmutation_follower(self):
        changes=({"state":"closed"},{"draft":True},
                 {"head":{"sha":"f"*40,"repo":{"id":101,"full_name":"owner/repository"}}})
        for change in changes:
            fixture,context,run,jobs=self.fixture(retired=change)
            with self.subTest(change=change):self.assertTrue(self.covered(fixture,context,run))

    def test_missing_or_privileged_or_ambiguous_proof_never_becomes_deferred(self):
        import copy
        for mutation in ("selected","mutation_active","mutation_missing","admitted","foreign_workflow","source_head","body","malformed","observation_drift","generation","title","publisher","shadowjournal","admission_missing"):
            fixture,context,run,jobs=self.fixture()
            if mutation=="selected":jobs[1]["steps"]=[{"name":cohort.SELECTED_PREFIX+"owner=99 artifact=1 digest="+fixture.header_ref["artifact_digest"],"status":"completed","conclusion":"success"}]
            elif mutation=="mutation_active":jobs[-1].update(status="in_progress",conclusion=None)
            elif mutation=="mutation_missing":jobs.pop();fixture.data["repos/owner/repository/actions/runs/99/attempts/1/jobs?per_page=100&page=1"]["total_count"]-=1
            elif mutation=="admitted":jobs[2]["conclusion"]="success"
            elif mutation=="foreign_workflow":context["header"]["workflow_sha"]="f"*40
            elif mutation=="source_head":fixture.data["repos/owner/repository/actions/runs/11"]["head_sha"]="f"*40
            elif mutation=="body":fixture.data["repos/owner/repository/pulls/7"]["body"]="changed"
            elif mutation=="malformed":fixture.data["repos/owner/repository/pulls/7"]["draft"]=None
            elif mutation=="generation":run=copy.deepcopy(run);run["run_number"]+=1
            elif mutation=="title":run["display_title"]="sensor=12 action=completed"
            elif mutation=="publisher":jobs[0]["conclusion"]="failure"
            elif mutation=="shadowjournal":
                listing=fixture.data["repos/owner/repository/actions/runs/99/artifacts?per_page=100&page=1"]
                listing["artifacts"].append(dict(listing["artifacts"][0],id=7));listing["total_count"]=2
            elif mutation=="admission_missing":
                jobs.pop(2);fixture.data["repos/owner/repository/actions/runs/99/attempts/1/jobs?per_page=100&page=1"]["total_count"]-=1
            else:
                original=fixture.read_json;reads=[0]
                def drifting(endpoint,**kwargs):
                    result=original(endpoint,**kwargs)
                    if "/pulls/" in endpoint:
                        reads[0]+=1
                        if reads[0]>1:result["draft"]=True
                    return result
                fixture.read_json=drifting
            with self.subTest(mutation=mutation):self.assertFalse(self.covered(fixture,context,run))


class CohortEmptyHeaderTests(unittest.TestCase):
    def fixture(self, mutation=None):
        import copy
        fixture=RawArtifactFixture()
        header=copy.deepcopy(fixture.header)
        header.update(member_ids=[],member_segments="",member_pages=[],alias_pages=[],batches=[])
        if mutation:mutation(header)
        content=dict(header);content.pop("cohort_digest")
        header["cohort_digest"]=hashlib.sha256(cohort.canonical(content)).hexdigest()
        election_steps=fixture.jobs["jobs"][1]["steps"]
        election_steps[:]=[step for step in election_steps if not step["name"].startswith(cohort.PAYLOAD_PREFIX+"cohort ") and step["name"]!="Upload immutable krr-governance-cohort-100-attempt-1"]
        reference=fixture.artifact(10,header)
        fixture.jobs["jobs"][1]["steps"].append({"name":cohort.SELECTED_PREFIX+"owner=100 artifact=10 digest="+reference["artifact_digest"],"status":"completed","conclusion":"success","number":20})
        fixture.jobs["jobs"].append({"id":998,"run_id":100,"head_sha":fixture.sha,"name":cohort.ADMISSION_JOB,"status":"completed","conclusion":"success","steps":[{"name":cohort.ADMITTED_PREFIX+"owner=100 artifact=10 digest="+reference["artifact_digest"],"status":"completed","conclusion":"success","number":1}]})
        fixture.jobs["total_count"]=len(fixture.jobs["jobs"])
        return fixture,reference

    def load(self,fixture,reference,**extra):
        from unittest.mock import patch
        with patch.dict("os.environ",{"GITHUB_REPOSITORY":fixture.repository}),patch.object(cohort.time,"time",return_value=200):
            return cohort.load_cohort(10,reference["artifact_digest"],expected_owner=100,expected_segment=extra.pop("expected_segment",None),
                read_json=fixture.read_json,read_archive=fixture.read_archive,budget=Budget(),deadline=300,**extra)

    def test_explicit_empty_context_requires_native_positive_owner_and_grants_no_member_or_target(self):
        fixture,reference=self.fixture()
        context=self.load(fixture,reference,allow_empty=True)
        self.assertEqual(context["members_by_id"],{});self.assertEqual(context["target_members_by_number"],{})
        self.assertEqual(context["header"]["member_ids"],[])
        with self.assertRaises(cohort.CohortError):self.load(fixture,reference)
        for extra in ({"expected_segment":1},{"member_ids":set()},{"member_ids":{11}}):
            with self.subTest(extra=extra),self.assertRaises(cohort.CohortError):self.load(fixture,reference,allow_empty=True,**extra)
        with self.assertRaises(cohort.CohortError):cohort.validate_target_members(context,1,[7])

    def test_empty_shadow_pages_and_unselected_or_failed_admission_fail_closed(self):
        for mutation in ("pages","segments","root","selected_missing","admission_missing","admission_skipped","duplicate","shadowselected"):
            change=(lambda header:header.update(member_pages=[header["root_journal"]])) if mutation=="pages" else (lambda header:header.update(member_segments="1")) if mutation=="segments" else (lambda header:header.update(root_deadline_epoch=21201)) if mutation=="root" else None
            fixture,reference=self.fixture(change)
            if mutation=="selected_missing":fixture.jobs["jobs"][1]["steps"].pop()
            elif mutation=="admission_missing":fixture.jobs["jobs"][-1]["steps"]=[]
            elif mutation=="admission_skipped":fixture.jobs["jobs"][-1]["conclusion"]="skipped"
            elif mutation=="duplicate":fixture.jobs["jobs"].append(dict(fixture.jobs["jobs"][-1],id=999));fixture.jobs["total_count"]+=1
            elif mutation=="shadowselected":fixture.jobs["jobs"][1]["steps"].append({"name":cohort.SELECTED_PREFIX+"owner=100 artifact=1 digest="+reference["artifact_digest"],"status":"completed","conclusion":"success"})
            with self.subTest(mutation=mutation),self.assertRaises(cohort.CohortError):self.load(fixture,reference,allow_empty=True)


class CohortSnapshotObservationTests(unittest.TestCase):
    def snapshot(self, *, transition=False, mutation=None, current_change=None):
        import copy, urllib.parse
        source=CohortSensorPaginationTests().sensor(1)
        other=copy.deepcopy(source);other["status"]="in_progress"
        if mutation:mutation(other)
        pull=copy.deepcopy(source["pull_requests"][0])
        if callable(current_change):current_change(pull)
        elif current_change:pull.update(current_change)
        class Reader:
            def request(self,endpoint):
                if "/pulls/" in endpoint:return copy.deepcopy(pull)
                state=urllib.parse.parse_qs(urllib.parse.urlsplit(endpoint).query)["status"][0]
                rows=[source] if state=="queued" else [other] if state=="in_progress" and transition else []
                return {"total_count":len(rows),"workflow_runs":copy.deepcopy(rows)}
        return cohort.active_source_snapshot(Reader(),"owner/repository",101,"master")

    def test_nullable_and_empty_current_body_filter_active_and_retired_source(self):
        for body in (None,""):
            for change in ({},{"state":"closed"},{"draft":True},
                           {"head":{"sha":"f"*40,"repo":{"id":101,"full_name":"owner/repository"}}}):
                with self.subTest(body=body,change=change):
                    self.assertEqual(self.snapshot(transition=True,current_change=dict(change,body=body)),{})

    def test_nonnullable_malformed_current_body_rejects_active_and_retired_source(self):
        for body in (False,0,1.0,[],{},"\0","\ud800"):
            for state in ("open","closed"):
                with self.subTest(body=body,state=state),self.assertRaisesRegex(cohort.CohortError,"body malformed"):
                    self.snapshot(current_change={"body":body,"state":state})
        with self.assertRaisesRegex(cohort.CohortError,"body malformed"):
            self.snapshot(current_change=lambda pull:pull.pop("body"))

    def test_native_partition_transition_retains_one_source_and_identity_drift_rejects(self):
        self.assertEqual(self.snapshot(transition=True)[1]["status"],"in_progress")
        def drift(source):
            source["head_sha"]="f"*40;source["pull_requests"][0]["head"]["sha"]="f"*40
        with self.assertRaisesRegex(cohort.CohortError,"Cross-partition"):self.snapshot(transition=True,mutation=drift)

    def test_malformed_api_observation_and_internal_source_binding_are_fail_closed(self):
        class Reader:
            def request(self,endpoint):return None
        with self.assertRaises(cohort.CohortError):cohort.active_source_snapshot(Reader(),"owner/repository",101,"master")
        for change in (lambda source:source["pull_requests"][0]["base"].update(ref=None),
                       lambda source:source.update(head_sha="f"*40),
                       lambda source:source.update(head_repository=None)):
            with self.subTest(change=change),self.assertRaises(cohort.CohortError):self.snapshot(transition=True,mutation=change)


class CohortOpenPRSnapshotTests(unittest.TestCase):
    def snapshot(self,count,mutation=None):
        import copy,urllib.parse
        pulls=[dict(CohortSensorPaginationTests().sensor(number)["pull_requests"][0],number=number)
               for number in range(1,count+1)]
        calls=[];scan=[0]
        class Reader:
            def request(self,endpoint):
                page=int(urllib.parse.parse_qs(urllib.parse.urlsplit(endpoint).query)["page"][0])
                if page==1:scan[0]+=1
                calls.append((scan[0],page))
                rows=copy.deepcopy(pulls[(page-1)*100:page*100])
                if mutation:mutation(rows,scan[0],page)
                return rows
        return cohort.open_pr_snapshot(Reader(),"owner/repository",101,"master"),calls

    def test_all_pages_and_terminal_are_read_twice_with_existing_six_hundred_bound(self):
        for count in (0,99,100,599,600):
            with self.subTest(count=count):
                pulls,calls=self.snapshot(count)
                self.assertEqual(len(pulls),count)
                self.assertEqual(calls,[(scan,page) for scan in (1,2) for page in range(1,count//100+2)])
        with self.assertRaisesRegex(cohort.CohortError,"page bound"):self.snapshot(601)

    def test_page_two_drift_is_rejected_even_with_stable_first_page(self):
        def drift(rows,scan,page):
            if scan==2 and page==2:rows[0]["body"]="changed"
        with self.assertRaisesRegex(cohort.CohortError,"did not stabilize"):self.snapshot(201,drift)
        def typed_drift(rows,scan,page):
            if page==2:rows[0]["unrelated_integer"]=1 if scan==1 else 1.0
        with self.assertRaisesRegex(cohort.CohortError,"did not stabilize"):self.snapshot(201,typed_drift)

    def test_duplicate_and_malformed_api_entries_are_not_normalized(self):
        for change in ({"number":True},{"state":"closed"}):
            with self.subTest(change=change),self.assertRaises(cohort.CohortError):
                self.snapshot(1,lambda rows,scan,page:rows[0].update(change) if rows else None)
        def duplicate(rows,scan,page):
            if page==2:rows[0]["number"]=1
        with self.assertRaisesRegex(cohort.CohortError,"identity malformed"):self.snapshot(101,duplicate)

    def test_absent_open_snapshot_source_requires_individual_positive_current_observation(self):
        import copy,urllib.parse
        source=CohortSensorPaginationTests().sensor(1)
        for observation in (None,{"state":"closed"},{"draft":True},{"body":None}):
            calls=[]
            class Reader:
                def request(self,endpoint):
                    calls.append(endpoint)
                    if "/pulls/" in endpoint:
                        if observation is None:return None
                        return dict(copy.deepcopy(source["pull_requests"][0]),**observation)
                    state=urllib.parse.parse_qs(urllib.parse.urlsplit(endpoint).query)["status"][0]
                    return {"total_count":int(state=="queued"),"workflow_runs":[source] if state=="queued" else []}
            with self.subTest(observation=observation):
                if observation is None:
                    with self.assertRaises(cohort.CohortError):
                        cohort.active_source_snapshot(Reader(),"owner/repository",101,"master",current_prs={})
                else:
                    self.assertEqual(cohort.active_source_snapshot(Reader(),"owner/repository",101,"master",current_prs={}),{})
                self.assertEqual(sum("/pulls/" in endpoint for endpoint in calls),1)

    def test_selected_current_body_and_native_clock_are_rechecked_before_authority(self):
        owner=CohortEligibilityOverflowTests()
        fixture=owner.fixture(1)
        request=fixture[5].request
        def drift(endpoint):
            value=request(endpoint)
            if "/pulls/" in endpoint:value["body"]="changed"
            return value
        fixture[5].request=drift
        with self.assertRaisesRegex(cohort.CohortError,"double-read drift"):owner.select(fixture)
        fixture=owner.fixture(1)
        fixture[3][1]["member"]["latch_started_at"]="1970-01-01T00:01:42Z"
        fixture[3][1]["member"]["source_deadline_epoch"]+=1
        with self.assertRaisesRegex(cohort.CohortError,"native clock drift"):owner.select(fixture)


class MemberGrantTests(unittest.TestCase):
    def fixture(self):
        import copy
        fixture, context, _, _ = CohortFollowerAndHandoffTests().follower()
        fixture.steps[cohort.PRODUCER_JOB][:] = fixture.steps[cohort.PRODUCER_JOB][:3]
        context["registered_writers"] = {1: 99}
        fixture.run.update(status="in_progress", conclusion=None, head_repository=copy.deepcopy(fixture.run["repository"]))
        binding = "owner=100 artifact=1 digest=" + fixture.header_ref["artifact_digest"]
        fixture.steps[cohort.ELECTION_JOB].append({"number": 50, "name": cohort.SELECTED_PREFIX + binding,
                                                 "status": "completed", "conclusion": "success"})
        for index, name, marker in ((999, cohort.ADMISSION_JOB, cohort.ADMITTED_PREFIX + binding),
                                   (1001, "Service bound cohort review sources", cohort.REGISTER_PREFIX +
                                    "segment=1 run=99 artifact=1 digest=" + fixture.header_ref["artifact_digest"])):
            fixture.jobs["jobs"].append({"id": index, "run_id": 100, "head_sha": fixture.sha, "run_attempt": 1,
                "name": name, "status": "completed" if index == 999 else "in_progress",
                "conclusion": "success" if index == 999 else None,
                "steps": [{"number": 1, "name": marker, "status": "completed", "conclusion": "success"}]})
        fixture.jobs["total_count"] = len(fixture.jobs["jobs"])
        source = fixture.data["repos/owner/repository/actions/runs/11"]
        source.update(status="in_progress", conclusion=None)
        writer = {"id": 99, "run_attempt": 1, "name": "PR governance status writer", "display_title": "source=100 scope=early segment=1",
                  "event": "workflow_dispatch", "path": ".github/workflows/pr-governance-status-writer.yml@master",
                  "head_sha": fixture.sha, "head_branch": "master", "repository": source["repository"], "head_repository": source["repository"],
                  "workflow_id": 8, "run_number": 10, "status": "in_progress", "conclusion": None}
        fixture.data["repos/owner/repository/actions/runs/99"] = writer
        fixture.data["repos/owner/repository"].update(id=101, full_name=fixture.repository)
        code = b"trusted immutable writer source"
        blob = hashlib.sha1(b"blob " + str(len(code)).encode() + b"\0" + code).hexdigest()
        fixture.data["repos/owner/repository/contents/scripts/review/pr_governance_status_writer.py?ref=" + fixture.sha] = {
            "path": "scripts/review/pr_governance_status_writer.py", "encoding": "base64", "sha": blob,
            "size": len(code), "content": base64.b64encode(code).decode()}
        from unittest.mock import patch
        with patch.object(cohort.time, "time", return_value=250):
            summary = cohort.encode_member_grant(context, 7, writer_id=99, segment=1, writer_workflow_blob=blob)
        query = {"source_run_id": "11", "cohort_artifact_id": "1", "cohort_artifact_digest": fixture.header_ref["artifact_digest"],
                 "cohort_segment": "1", "pr_base_sha": fixture.member["base_sha"], "pr_head_sha": fixture.member["head_sha"],
                 "pr_body_sha256": fixture.member["pr_body_sha256"]}
        for prefix in ("ci", "release"):
            query.update({prefix + "_workflow_id": "2", prefix + "_run_id": "3", prefix + "_run_number": "4",
                          prefix + "_run_attempt": "1", prefix + "_status": "completed", prefix + "_conclusion": "success"})
        check = {"id": 777, "name": "KRR / PR governance (trusted check)", "app": {"id": 4766933}, "head_sha": fixture.member["head_sha"],
                 "external_id": "krr-governance/v1/" + fixture.member["head_sha"] + "/writer-99", "status": "completed", "conclusion": "success",
                 "started_at": "1970-01-01T00:03:20Z", "completed_at": "1970-01-01T00:04:10Z",
                 "details_url": "https://github.com/owner/repository/actions/runs/99?" + cohort.urllib.parse.urlencode(query),
                 "output": {"summary": summary}}
        fixture.data["repos/owner/repository/check-runs/777"] = check
        fixture.check_endpoint = "repos/owner/repository/commits/" + fixture.member["head_sha"] + "/check-runs?" + cohort.urllib.parse.urlencode({"check_name": "KRR / PR governance (trusted check)", "app_id": 4766933, "filter": "all", "per_page": 100}) + "&page=1"
        fixture.data[fixture.check_endpoint] = {"total_count": 1, "check_runs": [check]}
        fixture.check_snapshot = {"repository": fixture.repository, "head_sha": check["head_sha"], "check_name": check["name"],
                                  "app_id": 4766933, "source_run_id": source["id"], "check_run_id": check["id"],
                                  "max_source_check_pages": 181, "endpoints": [fixture.check_endpoint],
                                  "pages": [copy.deepcopy(fixture.data[fixture.check_endpoint])]}
        return fixture, context, source, writer, check, blob

    def verify(self, fixture, source, writer, check, blob, *, budget=None, read_metadata=None):
        from unittest.mock import patch
        with patch.object(cohort.time, "time", return_value=250):
            return cohort.verify_sensor_member_grant(check, writer, source, fixture.data["repos/owner/repository/pulls/7"],
                read_json=fixture.read_json, budget=Budget() if budget is None else budget, deadline=300, workflow_blob=fixture.blob, writer_workflow_blob=blob,
                check_snapshot=fixture.check_snapshot, **({"read_metadata": read_metadata} if read_metadata is not None else {}))

    def metadata(self, fixture, blob):
        import copy
        fixture.calls.clear()
        contents = fixture.data["repos/owner/repository/contents/scripts/review/pr_governance_status_writer.py?ref=" + fixture.sha]
        code = base64.b64decode(contents["content"]).decode("utf-8")
        native = {"data": {"repository": {"id": "R_native_opaque", "databaseId": 101,
            "nameWithOwner": fixture.repository, "url": "https://github.com/" + fixture.repository,
            "defaultBranchRef": {"name": "master", "target": {"__typename": "Commit", "oid": fixture.sha}},
            "writer": {"__typename": "Blob", "oid": blob, "text": code, "byteSize": len(code.encode()),
                       "isBinary": False, "isTruncated": False}}, "rateLimit": {"cost": 1}}}
        calls = []
        def read(query, variables, *, timeout):
            self.assertEqual(query, cohort.MEMBER_GRANT_METADATA_QUERY)
            self.assertEqual(variables, {"owner": "owner", "name": "repository", "writerExpression": fixture.sha + ":scripts/review/pr_governance_status_writer.py"})
            self.assertLessEqual(timeout, 20)
            calls.append(len(fixture.calls))
            return copy.deepcopy(native)
        return native, calls, read

    def test_metadata_native_batch_preserves_independent_bytes_and_shared_budget(self):
        fixture, _, source, writer, check, blob = self.fixture()
        _, calls, read = self.metadata(fixture, blob)
        budget = cohort.ReadBudget(300)
        self.assertEqual(self.verify(fixture, source, writer, check, blob, budget=budget, read_metadata=read), 5500)
        self.assertEqual(len(calls), 2)
        self.assertEqual(budget.used, 10)
        self.assertEqual(len(fixture.calls), 8)
        self.assertFalse(any("/contents/" in path or path == "repos/owner/repository" or "/git/ref/" in path for path in fixture.calls))
        self.assertIn(fixture.check_endpoint, fixture.calls[calls[0]:calls[1]])
        self.assertIn("repos/owner/repository/pulls/7", fixture.calls)

    def test_metadata_native_errors_and_identity_types_never_fallback(self):
        changes = [lambda value: value.update(errors=[{"message": "native access denied"}]),
                   lambda value: value.update(data=None),
                   lambda value: value["data"]["rateLimit"].update(cost=True),
                   lambda value: value["data"].update(repository=None),
                   lambda value: value["data"]["repository"].update(databaseId=101.0),
                   lambda value: value["data"]["repository"].update(id=True),
                   lambda value: value["data"]["repository"].update(nameWithOwner="foreign/repository"),
                   lambda value: value["data"]["repository"].update(url="https://github.com/foreign/repository"),
                   lambda value: value["data"]["repository"]["defaultBranchRef"].update(name="other"),
                   lambda value: value["data"]["repository"]["defaultBranchRef"]["target"].update(__typename="Tag"),
                   lambda value: value["data"]["repository"]["defaultBranchRef"]["target"].update(oid="d" * 40),
                   lambda value: value["data"]["repository"]["writer"].update(isBinary=True),
                   lambda value: value["data"]["repository"]["writer"].update(isTruncated=True),
                   lambda value: value["data"]["repository"]["writer"].update(byteSize=True),
                   lambda value: value["data"]["repository"]["writer"].update(text="changed native bytes"),
                   lambda value: value["data"]["repository"]["writer"].update(text="bad\ud800"),
                   lambda value: value["data"]["repository"]["writer"].update(oid="d" * 40)]
        for index, change in enumerate(changes):
            fixture, _, source, writer, check, blob = self.fixture()
            value, calls, read = self.metadata(fixture, blob)
            change(value)
            with self.subTest(index=index), self.assertRaises(cohort.CohortError):
                self.verify(fixture, source, writer, check, blob, read_metadata=read)
            self.assertEqual(len(calls), 1)
            self.assertFalse(any("/contents/" in path or path == "repos/owner/repository" for path in fixture.calls))

    def test_metadata_native_closing_drift_and_callback_failure_fail_closed(self):
        fixture, _, source, writer, check, blob = self.fixture()
        _, calls, read = self.metadata(fixture, blob)
        def drift(query, variables, *, timeout):
            value = read(query, variables, timeout=timeout)
            if len(calls) == 2:
                value["data"]["repository"]["id"] += "changed"
            return value
        with self.assertRaisesRegex(cohort.CohortError, "metadata.*drift"):
            self.verify(fixture, source, writer, check, blob, read_metadata=drift)
        self.assertEqual(len(calls), 2)
        fixture, _, source, writer, check, blob = self.fixture()
        fixture.calls.clear()
        def failed(query, variables, *, timeout):
            raise cohort.CohortError("native GraphQL failed")
        with self.assertRaisesRegex(cohort.CohortError, "GraphQL failed"):
            self.verify(fixture, source, writer, check, blob, read_metadata=failed)
        self.assertFalse(any("/contents/" in path for path in fixture.calls))

    def test_metadata_native_request_obeys_original_role_budget(self):
        fixture, _, source, writer, check, blob = self.fixture()
        _, calls, read = self.metadata(fixture, blob)
        with self.assertRaises(cohort.CohortError):
            self.verify(fixture, source, writer, check, blob, budget=cohort.ReadBudget(9), read_metadata=read)
        self.assertLessEqual(len(calls), 2)

    def test_native_grant_reuses_membership_without_any_artifact_archive(self):
        fixture, _, source, writer, check, blob = self.fixture()
        fixture.calls.clear(); fixture.archives.clear()
        self.assertEqual(self.verify(fixture, source, writer, check, blob), 5500)
        self.assertFalse(any("/artifacts/" in endpoint for endpoint in fixture.calls))
        self.assertEqual(len(fixture.calls), 11)
        self.assertNotIn("repos/owner/repository/actions/runs/11/attempts/1/jobs?per_page=100&page=1", fixture.calls)

    def test_grant_original_clock_does_not_reopen_source_jobs(self):
        fixture, _, source, writer, check, blob = self.fixture()
        original = fixture.read_json
        def native(path, *, timeout):
            self.assertNotEqual(path, "repos/owner/repository/actions/runs/11/attempts/1/jobs?per_page=100&page=1")
            return original(path, timeout=timeout)
        fixture.read_json = native
        self.assertEqual(self.verify(fixture, source, writer, check, blob), 5500)

    def test_original_clock_producer_still_rejects_native_start_drift(self):
        from unittest.mock import patch
        fixture, context, _, _, _, _ = self.fixture()
        fixture.data["repos/owner/repository/actions/runs/11/attempts/1/jobs?per_page=100&page=1"]["jobs"][0]["steps"][0]["started_at"] = "1970-01-01T00:01:41Z"
        with patch.object(cohort.time, "time", return_value=250), self.assertRaisesRegex(cohort.CohortError, "clock drift"):
            cohort.resolve_member_clock(context["members_by_id"][11], read_json=fixture.read_json,
                                        budget=Budget(), deadline=300)

    def test_grant_original_clock_rejects_future_out_of_range_and_precision_loss(self):
        import copy
        for deadline in (5500.0000001, 8000, 253402311600, 1e308, True, float("inf")):
            fixture, _, source, writer, check, blob = self.fixture()
            grant = cohort.decode_member_grant(check["output"]["summary"], writer_workflow_blob=blob)
            grant["members"] = [[11, deadline]]
            check["output"]["summary"] = cohort.MEMBER_GRANT_PREFIX + json.dumps(grant, sort_keys=True, separators=(",", ":"))
            fixture.check_snapshot["pages"][0]["check_runs"][0]["output"] = copy.deepcopy(check["output"])
            with self.subTest(deadline=deadline), self.assertRaises(cohort.CohortError):
                self.verify(fixture, source, writer, check, blob)

    def test_grant_closing_output_cannot_reset_original_clock_to_now(self):
        import copy
        fixture, _, source, writer, check, blob = self.fixture()
        original = fixture.read_json
        def changed(path, *, timeout):
            value = original(path, timeout=timeout)
            if path == fixture.check_endpoint:
                value = copy.deepcopy(value)
                grant = cohort.decode_member_grant(value["check_runs"][0]["output"]["summary"], writer_workflow_blob=blob)
                grant["members"] = [[11, 250 + 5400]]
                value["check_runs"][0]["output"]["summary"] = cohort.MEMBER_GRANT_PREFIX + cohort.canonical(grant).decode()
            return value
        fixture.read_json = changed
        with self.assertRaisesRegex(cohort.CohortError, "snapshot drift"):
            self.verify(fixture, source, writer, check, blob)

    def test_complete_200_target_union_roundtrip_and_byte_limit(self):
        import copy
        from unittest.mock import patch
        _, context, _, _, _, blob = self.fixture()
        original = copy.deepcopy(context["members_by_id"][11])
        ids = list(range(110298349000, 110298349200))
        context["members_by_id"] = {identifier: dict(original, source_run_id=identifier) for identifier in ids}
        context["target_members_by_number"] = {7: ids}
        context["header"].update(member_ids=ids, member_segments="1" * 200)
        with patch.object(cohort.time, "time", return_value=250):
            summary = cohort.encode_member_grant(context, 7, writer_id=99, segment=1, writer_workflow_blob=blob)
        self.assertEqual(cohort.decode_member_grant(summary, writer_workflow_blob=blob)["members"], [[identifier, 5500] for identifier in ids])
        self.assertLessEqual(len(summary.encode()), 8192)
        oversized = cohort.decode_member_grant(summary, writer_workflow_blob=blob)
        oversized["members"] = [[9007199254740000 + index, 253402306199.99997] for index in range(200)]
        with self.assertRaisesRegex(cohort.CohortError, "byte bound"):
            cohort.decode_member_grant(cohort.MEMBER_GRANT_PREFIX + cohort.canonical(oversized).decode(), writer_workflow_blob=blob)

    def test_canonical_size_boundary_and_reserved_family(self):
        _, _, _, _, check, blob = self.fixture()
        grant = cohort.decode_member_grant(check["output"]["summary"], writer_workflow_blob=blob)
        base = cohort.MEMBER_GRANT_PREFIX + cohort.canonical(grant).decode()
        grant["base_ref"] += "x" * (8192 - len(base.encode()))
        exact = cohort.MEMBER_GRANT_PREFIX + cohort.canonical(grant).decode()
        self.assertEqual(len(exact.encode()), 8192)
        self.assertIsNotNone(cohort.decode_member_grant(exact, writer_workflow_blob=blob))
        for summary in (exact + " ", "KRR cohort member grant", "KRR cohort member grant v", "KRR cohort member grant v2\n{}"):
            with self.subTest(summary=summary[:30]), self.assertRaises(cohort.CohortError):
                cohort.decode_member_grant(summary, writer_workflow_blob=blob)
        for summary in (None, 1, {}, "legacy governance decision"):
            self.assertIsNone(cohort.decode_member_grant(summary, writer_workflow_blob=blob))

    def test_builder_rejects_incomplete_or_untyped_target(self):
        import copy
        from unittest.mock import patch
        _, original, _, _, _, blob = self.fixture()
        variants = [lambda value: value["target_members_by_number"].update({7: []}),
                    lambda value: value.update(loaded_segments=[True]),
                    lambda value: value["registered_writers"].update({1: 99.0}),
                    lambda value: value["members_by_id"][11].update(repository_id=101.0),
                    lambda value: value["members_by_id"][11].update(pr_number=7.0),
                    lambda value: value["members_by_id"].update({12: dict(value["members_by_id"][11], source_run_id=12)})]
        for change in variants:
            context = copy.deepcopy(original); change(context)
            with self.subTest(change=change), patch.object(cohort.time, "time", return_value=250), self.assertRaises(cohort.CohortError):
                cohort.encode_member_grant(context, 7, writer_id=99, segment=1, writer_workflow_blob=blob)

    def test_parser_rejects_noncanonical_or_untyped_union(self):
        import copy
        _, _, _, _, check, blob = self.fixture()
        grant = cohort.decode_member_grant(check["output"]["summary"], writer_workflow_blob=blob)
        variants = [("repository_id", True), ("pr_number", 7.0), ("owner_run_attempt", True), ("segment", 1.0),
                    ("writer_workflow_blob_sha", "d" * 40), ("members", [[11, True]]), ("members", [[11.0, 5500]]),
                    ("members", [[11, 5500], [11, 5500]]), ("members", [[12, 5500], [11, 5500]]),
                    ("base_ref", "bad\ud800")]
        for key, value in variants:
            changed = copy.deepcopy(grant); changed[key] = value
            summary = cohort.MEMBER_GRANT_PREFIX + json.dumps(changed, sort_keys=True, separators=(",", ":"))
            with self.subTest(key=key, value=repr(value)), self.assertRaises(cohort.CohortError):
                cohort.decode_member_grant(summary, writer_workflow_blob=blob)
        for raw in ('{"v":1,"v":1}', check["output"]["summary"].split("\n", 1)[1] + "\n",
                    check["output"]["summary"].split("\n", 1)[1].replace("5500", "NaN")):
            with self.assertRaises(cohort.CohortError):
                cohort.decode_member_grant(cohort.MEMBER_GRANT_PREFIX + raw, writer_workflow_blob=blob)

    def test_native_identity_clock_and_replay_negatives(self):
        import copy
        changes = [lambda f, s, w, c: s.update(run_attempt=True),
                   lambda f, s, w, c: w.update(id=99.0),
                   lambda f, s, w, c: w.update(display_title="source=100 scope=all segment=1"),
                   lambda f, s, w, c: f.data["repos/owner/repository/pulls/7"].update(body="new body"),
                   lambda f, s, w, c: f.data["repos/owner/repository/git/ref/heads/master"]["object"].update(sha="d" * 40),
                   lambda f, s, w, c: f.run.update(status="completed", conclusion="cancelled"),
                   lambda f, s, w, c: f.jobs["jobs"][0].update(run_id=100.0),
                   lambda f, s, w, c: f.steps[cohort.ELECTION_JOB][-1].update(number=True),
                   lambda f, s, w, c: f.steps[cohort.ELECTION_JOB][-1].update(name=cohort.SELECTED_PREFIX + "owner=100 artifact=2 digest=" + f.header_ref["artifact_digest"]),
                   lambda f, s, w, c: f.jobs["jobs"][-1]["steps"][0].update(name=cohort.REGISTER_PREFIX + "segment=1 run=98 artifact=1 digest=" + f.header_ref["artifact_digest"]),
                   lambda f, s, w, c: f.steps[cohort.ELECTION_JOB].append({"number": 51, "name": cohort.HANDOFF_PREFIX + "owner=100", "status": "completed", "conclusion": "success"}),
                   lambda f, s, w, c: c.update(id=True),
                   lambda f, s, w, c: c["app"].update(id=4766933.0),
                   lambda f, s, w, c: c.update(details_url=c["details_url"] + "&pr_body_sha256=" + "a" * 64),
                   lambda f, s, w, c: f.data[f.check_endpoint].update(check_runs=[dict(c, id=778)]),
                   lambda f, s, w, c: f.data[f.check_endpoint].update(check_runs=[dict(c, output={"summary": c["output"]["summary"] + " "})])]
        for index, change in enumerate(changes):
            fixture, _, source, writer, check, blob = self.fixture()
            change(fixture, source, writer, check)
            with self.subTest(index=index), self.assertRaises(cohort.CohortError):
                self.verify(fixture, source, writer, check, blob)

    def test_closing_native_lifecycle_and_output_drift(self):
        import copy
        for endpoint_kind in ("owner", "jobs", "writer", "source", "pr"):
            fixture, _, source, writer, check, blob = self.fixture()
            original = fixture.read_json
            endpoint = ("repos/owner/repository/pulls/7" if endpoint_kind == "pr" else
                        "repos/owner/repository/actions/runs/" + {"owner": "100", "jobs": "100/attempts/1/jobs?per_page=100&page=1", "writer": "99", "source": "11"}[endpoint_kind])
            calls = 0
            def drift(path, *, timeout):
                nonlocal calls
                value = original(path, timeout=timeout)
                if path == endpoint:
                    calls += 1
                    if calls == (2 if endpoint_kind in {"owner", "jobs"} else 1):
                        if endpoint_kind == "jobs":
                            value["jobs"][-1]["steps"][0]["name"] += " stale"
                        elif endpoint_kind == "source":
                            value["workflow_id"] = 9.0
                        elif endpoint_kind == "pr":
                            value["body"] = "changed during native proof"
                        else:
                            value.update(status="completed", conclusion="cancelled")
                return value
            fixture.read_json = drift
            with self.subTest(kind=endpoint_kind), self.assertRaises(cohort.CohortError):
                self.verify(fixture, source, writer, check, blob)

    def test_fractional_original_source_clock_is_preserved(self):
        from unittest.mock import patch
        fixture, context, source, writer, check, blob = self.fixture()
        context["members_by_id"][11].update(latch_started_at="1970-01-01T00:01:40.125Z", source_deadline_epoch=5500.125)
        fixture.data["repos/owner/repository/actions/runs/11/attempts/1/jobs?per_page=100&page=1"]["jobs"][0]["steps"][0]["started_at"] = "1970-01-01T00:01:40.125Z"
        with patch.object(cohort.time, "time", return_value=250):
            check["output"]["summary"] = cohort.encode_member_grant(context, 7, writer_id=99, segment=1, writer_workflow_blob=blob)
        fixture.check_snapshot["pages"][0]["check_runs"][0]["output"]["summary"] = check["output"]["summary"]
        self.assertEqual(self.verify(fixture, source, writer, check, blob), 5500.125)

    def test_native_api_size_is_independent_of_marker_byte_bound(self):
        fixture, _, source, writer, check, blob = self.fixture()
        check["output"]["text"] = "native Check Run text " * 1000
        source["repository"]["description"] = "native repository metadata " * 1000
        fixture.run["repository"]["description"] = "native owner repository metadata " * 1000
        fixture.check_snapshot["pages"][0]["check_runs"][0]["output"] = dict(check["output"])
        self.assertEqual(self.verify(fixture, source, writer, check, blob), 5500)

    def test_grant_native_artifact_owner_pin_and_expiry_negatives(self):
        for key, value in (("root_deadline_epoch", 249), ("owner_dispatcher_run_id", 101),
                           ("cohort_artifact_id", 2), ("cohort_artifact_digest", "sha256:" + "d" * 64),
                           ("workflow_blob_sha", "d" * 40), ("writer_workflow_blob_sha", "d" * 40),
                           ("members", [[12, 5500]])):
            fixture, _, source, writer, check, blob = self.fixture()
            grant = cohort.decode_member_grant(check["output"]["summary"], writer_workflow_blob=blob)
            grant[key] = value
            check["output"]["summary"] = cohort.MEMBER_GRANT_PREFIX + cohort.canonical(grant).decode()
            with self.subTest(key=key), self.assertRaises((cohort.CohortError, KeyError)):
                self.verify(fixture, source, writer, check, blob)

    def test_owner_event_and_title_match_existing_writer_contract(self):
        import os
        import pr_governance_status_writer as writer_contract
        from unittest.mock import patch
        cases = [(event, "PR governance dispatcher", None) for event in sorted(writer_contract.DISPATCHER_EVENTS)]
        cases += [("workflow_run", "sensor=11 action=in_progress", "sensor=11 action=in_progress"),
                  ("workflow_dispatch", "sensor=11 action=in_progress", "sensor=11 action=in_progress"),
                  ("workflow_run", "sensor=11 action=other", "sensor=11 action=other"),
                  ("push", "PR governance dispatcher", None), ("workflow_run", "foreign", "foreign")]
        for event, name, title in cases:
            fixture, _, source, writer, check, blob = self.fixture()
            fixture.run.update(event=event, name=name, display_title=title)
            with patch.dict(os.environ, {"GOVERNANCE_COHORT_ARTIFACT_ID": "1", "GOVERNANCE_COHORT_ARTIFACT_DIGEST": fixture.header_ref["artifact_digest"]}):
                expected = event in writer_contract.DISPATCHER_EVENTS and writer_contract.dispatcher_name_matches(fixture.run)
            with self.subTest(event=event, name=name):
                if expected:
                    self.assertEqual(self.verify(fixture, source, writer, check, blob), 5500)
                else:
                    with self.assertRaises(cohort.CohortError):
                        self.verify(fixture, source, writer, check, blob)

    def test_skipped_handoff_keeps_original_native_lease(self):
        for status, conclusion, accepted in (("queued", None, True), ("completed", "skipped", True),
                                              ("pending", None, False), ("in_progress", None, False),
                                              ("completed", "success", False), ("completed", "failure", False)):
            fixture, _, source, writer, check, blob = self.fixture()
            fixture.steps[cohort.ELECTION_JOB].append({"number": 51, "name": cohort.HANDOFF_PREFIX +
                "owner=100 artifact=1 digest=" + fixture.header_ref["artifact_digest"], "status": status, "conclusion": conclusion})
            with self.subTest(status=status, conclusion=conclusion):
                if accepted:
                    self.assertEqual(self.verify(fixture, source, writer, check, blob), 5500)
                else:
                    with self.assertRaises(cohort.CohortError):
                        self.verify(fixture, source, writer, check, blob)

    def test_higher_immutable_id_pending_on_page_two_defeats_timestamp_latest(self):
        import copy
        fixture, _, source, writer, check, blob = self.fixture()
        first = []
        for index in range(100):
            old = copy.deepcopy(check)
            old.update(id=index + 1, external_id="krr-governance/v1/" + check["head_sha"] + "/writer-" + str(index + 1000))
            first.append(old)
        newer = copy.deepcopy(check)
        newer.update(id=778, external_id="krr-governance/v1/" + check["head_sha"] + "/writer-9999",
                     status="in_progress", conclusion=None, started_at="1970-01-01T00:01:00Z", completed_at=None)
        endpoint = fixture.check_endpoint.rsplit("&page=", 1)[0]
        fixture.data[endpoint + "&page=1"] = {"total_count": 102, "check_runs": first}
        fixture.data[endpoint + "&page=2"] = {"total_count": 102, "check_runs": [check, newer]}
        fixture.check_snapshot["endpoints"] = [endpoint + "&page=1", endpoint + "&page=2"]
        fixture.check_snapshot["pages"] = [copy.deepcopy(fixture.data[path]) for path in fixture.check_snapshot["endpoints"]]
        with self.assertRaises(cohort.CohortError):
            self.verify(fixture, source, writer, check, blob)

    def complete_check_pages(self, fixture, check, total=102):
        import copy
        entries = []
        for index in range(total - 1):
            older = copy.deepcopy(check)
            older.update(id=index + 1, external_id="krr-governance/v1/" + check["head_sha"] + "/writer-" + str(index + 10000),
                         status="queued", conclusion=None, started_at=None, completed_at=None, output={"summary": "legacy pending"})
            entries.append(older)
        entries.append(check)
        base = fixture.check_endpoint.rsplit("&page=", 1)[0]
        endpoints = [base + "&page=" + str(number) for number in range(1, (total + 99) // 100 + 1)]
        for number, endpoint in enumerate(endpoints):
            fixture.data[endpoint] = {"total_count": total, "check_runs": entries[number * 100:(number + 1) * 100]}
        fixture.check_snapshot.update(endpoints=endpoints, pages=[fixture.read_json(endpoint, timeout=20) for endpoint in endpoints], check_run_id=check["id"])
        fixture.calls.clear()
        return endpoints

    def test_complete_initial_native_snapshot_and_closing_every_page(self):
        fixture, _, source, writer, check, blob = self.fixture()
        endpoints = self.complete_check_pages(fixture, check)
        self.assertEqual(self.verify(fixture, source, writer, check, blob), 5500)
        self.assertEqual([path for path in fixture.calls if path in endpoints], endpoints)
        self.assertEqual(len(fixture.calls), 12)

    def test_original_181_page_history_fits_existing_role_300(self):
        fixture, _, source, writer, check, blob = self.fixture()
        check["id"] = 20000
        endpoints = self.complete_check_pages(fixture, check, total=18001)
        budget = cohort.ReadBudget(300)
        self.assertEqual(len(endpoints), 181)
        self.assertEqual(self.verify(fixture, source, writer, check, blob, budget=budget), 5500)
        self.assertEqual(budget.used, 191)
        self.assertEqual([path for path in fixture.calls if path in endpoints], endpoints)

    def test_native_page_two_same_count_replacement_and_raw_type_drift(self):
        import copy
        changes = [lambda row: row.update(id=778, external_id="krr-governance/v1/" + "c" * 40 + "/writer-9999"),
                   lambda row: row["output"].update(summary="different native output"),
                   lambda row: row.update(id=777.0), lambda row: row["app"].update(id=4766933.0)]
        for index, change in enumerate(changes):
            fixture, _, source, writer, check, blob = self.fixture()
            endpoints = self.complete_check_pages(fixture, check)
            fixture.data[endpoints[-1]] = copy.deepcopy(fixture.data[endpoints[-1]])
            change(fixture.data[endpoints[-1]]["check_runs"][-1])
            with self.subTest(index=index), self.assertRaises(cohort.CohortError):
                self.verify(fixture, source, writer, check, blob)
            self.assertIn(endpoints[-1], fixture.calls)

    def test_snapshot_missing_incomplete_context_and_duplicates_fail_closed(self):
        import copy
        changes = [lambda f: setattr(f, "check_snapshot", None),
                   lambda f: f.check_snapshot.update(max_source_check_pages=True),
                   lambda f: f.check_snapshot.update(source_run_id=11.0),
                   lambda f: f.check_snapshot["endpoints"].__setitem__(0, f.check_snapshot["endpoints"][0].replace("filter=all", "filter=latest")),
                   lambda f: f.check_snapshot["pages"].pop(),
                   lambda f: f.check_snapshot["pages"][1]["check_runs"][0].update(id=1),
                   lambda f: f.check_snapshot["pages"][1]["check_runs"][0].update(external_id=f.check_snapshot["pages"][0]["check_runs"][0]["external_id"]),
                   lambda f: f.check_snapshot["pages"][1].update(total_count=102.0),
                   lambda f: f.check_snapshot["pages"][1]["check_runs"][0]["app"].update(id=True)]
        for index, change in enumerate(changes):
            fixture, _, source, writer, check, blob = self.fixture()
            self.complete_check_pages(fixture, check)
            change(fixture)
            with self.subTest(index=index), self.assertRaises(cohort.CohortError):
                self.verify(fixture, source, writer, check, blob)

    def test_existing_dispatcher_history_coexists_with_exact_writer_grant(self):
        fixture, _, source, writer, check, blob = self.fixture()
        self.complete_check_pages(fixture, check, total=2)
        old = fixture.check_snapshot["pages"][0]["check_runs"][0]
        old["external_id"] = "krr-governance/v1/" + check["head_sha"] + "/dispatcher-100"
        fixture.data[fixture.check_endpoint]["check_runs"][0]["external_id"] = old["external_id"]
        self.assertEqual(self.verify(fixture, source, writer, check, blob), 5500)

    def test_history_generation_grammar_never_promotes_selected_dispatcher(self):
        for suffix in ("dispatcher-0", "dispatcher-01", "other-100", "Dispatcher-100", "writer-0"):
            fixture, _, source, writer, check, blob = self.fixture()
            self.complete_check_pages(fixture, check, total=2)
            fixture.check_snapshot["pages"][0]["check_runs"][0]["external_id"] = "krr-governance/v1/" + check["head_sha"] + "/" + suffix
            with self.subTest(suffix=suffix), self.assertRaises(cohort.CohortError):
                self.verify(fixture, source, writer, check, blob)
        fixture, _, source, writer, check, blob = self.fixture()
        check["external_id"] = "krr-governance/v1/" + check["head_sha"] + "/dispatcher-99"
        with self.assertRaises(cohort.CohortError):
            self.verify(fixture, source, writer, check, blob)
