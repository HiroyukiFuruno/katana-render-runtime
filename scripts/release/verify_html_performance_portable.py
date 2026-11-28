#!/usr/bin/env python3
"""Verify a transported performance bundle against an externally trusted manifest hash.

This checks transported observations and recalculates their measurement policy.
The separate formal release gate additionally requires Git and source admission.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess
import sys
import zipfile

import verify_html_performance_evidence as gate


def verify_portable(directory: Path, trusted_manifest_sha256: str) -> gate.JsonObject:
    """Recompute supported measurement policies without claiming release admission."""
    gate.digest(trusted_manifest_sha256)
    manifest = directory / "bundle.json"
    gate.require(gate.sha256(manifest) == trusted_manifest_sha256, "trusted manifest SHA256 differs")
    bundle = gate.Bundle(directory)
    attempts = gate._objects(bundle.manifest.get("attempts"), "viewport attempts")
    gate.require(len(attempts) == 84, "all 84 catalog conditions required")
    identities: set[str] = set()
    actual_conditions: list[gate.JsonObject] = []
    catalog_ref: gate.JsonObject | None = None
    noncomparable_ids: set[str] = set()
    paired = 0
    noncomparable = 0
    for attempt in attempts:
        expected = bundle.json(attempt["expected"])
        result = bundle.json(attempt["result"])
        manifest_value = bundle.json(attempt["pairManifest"])
        catalog = gate._object(expected.get("catalogCondition"), "catalog condition")
        identity = gate._text(catalog.get("condition_id"), "condition id")
        gate.require(expected["viewport"] == catalog["viewport"]
                     and expected["fixture_sha256"] == catalog["fixture_sha256"],
                     "measurement viewport or fixture differs from catalog")
        gate.require(identity not in identities, "duplicate catalog condition")
        identities.add(identity)
        actual_conditions.append(catalog)
        current_ref = gate._object(expected.get("catalogPlanRef"), "catalog plan reference")
        if catalog_ref is None:
            catalog_ref = current_ref
        gate.require(current_ref == catalog_ref, "catalog plan references differ")
        plan = bundle.json(expected["operationPlanRef"])
        order = gate._objects(manifest_value.get("order"), "pair order")
        pins = gate._objects(expected.get("reportPins"), "report pins")
        gate.require(len(order) == len(pins) == 10 and manifest_value.get("measurement_executed") is True,
                     "incomplete measurement order")
        non_comparable = manifest_value.get("measurement_status") == "nonComparable"
        fragment = gate.non_comparable_baseline_fragment(expected) if non_comparable else None
        failures = gate._objects(manifest_value.get("failures"), "baseline failures") if non_comparable else []
        if non_comparable:
            gate.require(fragment is not None and len(failures) == 5, "baseline failure slots differ")
            gate.require(failures[1:] == [{"status": "missing",
                                          "reason": "baseline skipped after known fragment refusal",
                                          "afterFailure": failures[0]} for _ in range(4)],
                         "baseline skipped failures differ")
        paths: list[Path] = []
        for index, (row, pin) in enumerate(zip(order, pins, strict=True)):
            pair = index // 2
            arm = ("baseline", "candidate")[index % 2] if pair % 2 == 0 else ("candidate", "baseline")[index % 2]
            gate.require(row.get("arm") == arm and row.get("pair_index") == pair
                         and row.get("output") == pin.get("path"), "pair order differs")
            path = bundle.file(pin)
            gate.require(path.name == f"{arm}-{pair}.json", "report name differs")
            report = gate.read_json(path)
            if non_comparable and arm == "baseline":
                assert fragment is not None
                gate._validate_noncomparable_baseline_order(bundle, expected, row, report, pair,
                                                             fragment, failures[0])
                continue
            report_arm, samples = gate._validate_report(report, expected)
            gate.require(report_arm == (gate.baseline_arm_id(expected) if arm == "baseline" else arm)
                         and report.get("measurement_executed") is True
                         and report.get("pair_index") == pair
                         and [sample.get("sample") for sample in samples] == list(range(5)),
                         "measurement report differs")
            for sample in samples:
                gate.validate_measurement_sample(sample, expected, plan, candidate=arm == "candidate")
            paths.append(path)
        if non_comparable:
            gate.require(gate.non_comparable_baseline_fragment(expected) is not None
                         and result.get("comparisonStatus") == "nonComparable",
                         "unsupported non-comparable baseline")
            gate.require(result.get("policy") == {
                "status": "nonComparable", "policyPass": False,
                "reason": "known baseline fragment navigation refusal", "conditionId": identity,
                "fragment": fragment, "candidateOperationalPass": True, "candidateSamples": 25,
                "phaseRecords": len(gate.PHASES) * 25,
            }, "non-comparable policy differs")
            noncomparable += 1
            noncomparable_ids.add(identity)
        else:
            policy = gate.evaluate_measurement_policy(paths, expected)
            gate.require(result.get("comparisonStatus") == "paired" and result.get("policy") == policy,
                         "paired measurement policy differs")
            if "memoryPolicy" not in result:
                gate.require(policy["policyPass"] is True, "paired time/RSS regression")
            paired += 1
        if "memoryPolicy" in result:
            gate._require_v2_memory_policy(bundle, expected, result, paths, non_comparable=non_comparable)
    gate.require((paired == 84 and noncomparable == 0)
                 or (paired == 72 and noncomparable_ids == set(gate.NON_COMPARABLE_BASELINE_FRAGMENT_FAILURES)),
                 "catalog comparison set differs")
    assert catalog_ref is not None
    catalog_plan = bundle.json(catalog_ref)
    gate.require(catalog_plan.get("profile") == "krr-html84-condition-plan-v1"
                 and catalog_plan.get("acceptance") is False, "catalog plan differs")
    planned = gate._objects(catalog_plan.get("conditions"), "planned catalog conditions")
    gate.require(len(planned) == 84, "catalog plan coverage differs")
    for actual, plan_item in zip(actual_conditions, planned, strict=True):
        for plan_key, actual_key in (("conditionId", "condition_id"), ("caseId", "case_id"),
                                     ("state", "state"), ("operationKind", "operation_kind"),
                                     ("targetSelector", "target_selector"), ("sourceSha256", "fixture_sha256"),
                                     ("viewport", "viewport")):
            gate.require(plan_item.get(plan_key) == actual.get(actual_key), "catalog condition differs")
    gate.require(gate.sha256(manifest) == trusted_manifest_sha256, "manifest changed during verification")
    return {"samplePolicyChecks": "passed", "pairedConditions": paired,
            "nonComparableConditions": noncomparable, "formalReleaseAcceptance": False,
            "limitation": "official catalog source, Git/source admission, "
                          "full execution provenance and visual acceptance not verified"}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence-dir", type=Path, required=True)
    parser.add_argument("--trusted-manifest-sha256", required=True,
                        help="bundle.json SHA256 obtained independently of the bundle")
    args = parser.parse_args()
    try:
        result = verify_portable(args.evidence_dir.resolve(), args.trusted_manifest_sha256)
        print(json.dumps(result, sort_keys=True))
        return 0
    except (ValueError, KeyError, TypeError, OSError, subprocess.CalledProcessError, zipfile.BadZipFile) as error:
        print(f"HTML performance portable verification failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
