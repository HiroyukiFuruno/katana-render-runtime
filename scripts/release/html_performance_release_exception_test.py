from __future__ import annotations

import unittest
from collections.abc import Callable
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

import html_performance_memory_evidence as evidence
import html_performance_memory_policy as memory
import html_performance_release_exception as exception
import verify_html_performance_evidence as gate


def policy() -> dict[str, object]:
    return {
        "policyPass": False,
        "retention": {"retention_allowed": True},
        "phases": {
            phase: {
                "peak_allowed": phase != "cold_open",
                "time_regression": {"detected_bad": False},
            }
            for phase in exception.PHASES
        },
    }


def projection() -> dict[str, object]:
    return {
        "retentionAllowed": True,
        "phases": {
            phase: {"timeRegression": False, "incrementAllowed": phase != "cold_open"}
            for phase in exception.PHASES
        },
    }


def call(value: dict[str, object], *, release_version: str = exception.RELEASE_VERSION,
         issue_number: int = exception.ISSUE_NUMBER, candidate_head: str = exception.CANDIDATE_HEAD,
         bundle_manifest_sha256: str = exception.BUNDLE_MANIFEST_SHA256,
         condition_id: str = "events/expected-state/620x900@1",
         policy_version: str = exception.MEMORY_POLICY_VERSION,
         result_projection: dict[str, object] | None = None) -> bool:
    return exception.allows_issue_126_initial_rss(
        release_version=release_version, issue_number=issue_number, candidate_head=candidate_head,
        bundle_manifest_sha256=bundle_manifest_sha256, condition_id=condition_id,
        policy_version=policy_version, canonical_policy=value,
        projection=projection() if result_projection is None else result_projection,
    )


class ReleaseExceptionTest(unittest.TestCase):
    def test_projection_uses_v7_noop_and_rejects_rss_mismatch(self) -> None:
        samples: list[gate.JsonObject] = [{} for _ in range(memory.SAMPLE_COUNT)]
        reports: dict[int, gate.JsonObject] = {index: {} for index in range(5)}
        phase_values = ([1.0] * memory.SAMPLE_COUNT, [100.0] * memory.SAMPLE_COUNT)
        with (patch.object(gate, "_group_reports", return_value={"baseline": reports, "candidate": reports}),
              patch.object(evidence, "_validated_samples", side_effect=[samples, samples]),
              patch.object(evidence, "_proven_no_scroll", return_value=True),
              patch.object(evidence, "_full_y_rss_matches_re_display", return_value=False),
              patch.object(evidence, "_phase_values", return_value=phase_values),
              patch.object(memory, "evaluate_retention")):
            self.assertIsNone(exception.post_cold_increment_projection([Path("report")], {}, [1.0]))

        calls: list[bool] = []
        def values(*_args: object, no_scroll: bool) -> tuple[list[float], list[float]]:
            calls.append(no_scroll)
            return phase_values
        with (patch.object(gate, "_group_reports", return_value={"baseline": reports, "candidate": reports}),
              patch.object(evidence, "_validated_samples", side_effect=[samples, samples]),
              patch.object(evidence, "_proven_no_scroll", return_value=True),
              patch.object(evidence, "_full_y_rss_matches_re_display", return_value=True),
              patch.object(evidence, "_phase_values", side_effect=values),
              patch.object(memory, "evaluate_peak_phase", return_value=SimpleNamespace(time_regression=SimpleNamespace(detected_bad=False))),
              patch.object(memory, "evaluate_retention", return_value=SimpleNamespace(retention_allowed=True))):
            result = exception.post_cold_increment_projection([Path("report")], {}, [1.0])
        self.assertIsNotNone(result)
        self.assertIn(True, calls)

    def test_projection_rejects_nonmonotonic_cumulative_rss(self) -> None:
        samples: list[gate.JsonObject] = [{} for _ in range(memory.SAMPLE_COUNT)]
        reports: dict[int, gate.JsonObject] = {index: {} for index in range(5)}
        stable = ([1.0] * memory.SAMPLE_COUNT, [100.0] * memory.SAMPLE_COUNT)
        decreasing = ([1.0] * memory.SAMPLE_COUNT, [99.0] * memory.SAMPLE_COUNT)
        values = iter([stable, stable, stable, stable, decreasing, decreasing])
        with (patch.object(gate, "_group_reports", return_value={"baseline": reports, "candidate": reports}),
              patch.object(evidence, "_validated_samples", side_effect=[samples, samples]),
              patch.object(evidence, "_proven_no_scroll", return_value=False),
              patch.object(evidence, "_phase_values", side_effect=lambda *_args, **_kwargs: next(values)),
              patch.object(memory, "evaluate_peak_phase", return_value=SimpleNamespace(time_regression=SimpleNamespace(detected_bad=False))),
              patch.object(memory, "evaluate_retention", return_value=SimpleNamespace(retention_allowed=True))):
            self.assertIsNone(exception.post_cold_increment_projection([Path("report")], {}, [1.0]))

    def test_allows_only_the_pinned_initial_rss_failure(self) -> None:
        self.assertTrue(call(policy()))

    def test_rejects_epoch_and_scope_drift(self) -> None:
        cases: tuple[tuple[str, Callable[[], bool]], ...] = (
            ("issue", lambda: call(policy(), issue_number=125)),
            ("release", lambda: call(policy(), release_version="0.4.25")),
            ("head", lambda: call(policy(), candidate_head="0" * 40)),
            ("manifest", lambda: call(policy(), bundle_manifest_sha256="0" * 64)),
            ("policy", lambda: call(policy(), policy_version="html-memory-v6")),
            ("condition", lambda: call(policy(), condition_id="events/expected-state/1280x900@1")),
        )
        for label, evaluate in cases:
            with self.subTest(label=label):
                self.assertFalse(evaluate())

    def test_rejects_redisplay_time_regression(self) -> None:
        value = policy()
        phases = value["phases"]
        assert isinstance(phases, dict)
        redisplay = phases["re_display"]
        assert isinstance(redisplay, dict)
        redisplay["time_regression"] = {"detected_bad": True}
        result = projection()
        result_phases = result["phases"]
        assert isinstance(result_phases, dict)
        result_phases["re_display"] = {"timeRegression": True, "incrementAllowed": True}
        self.assertFalse(call(value, condition_id="events/expected-state/960x720@1", result_projection=result))

    def test_rejects_non_memory_failures_and_retention_failure(self) -> None:
        peak_failure = policy()
        phases = peak_failure["phases"]
        assert isinstance(phases, dict)
        phases["warm_open"] = {"peak_allowed": False, "time_regression": {"detected_bad": False}}
        bad_projection = projection()
        projected = bad_projection["phases"]
        assert isinstance(projected, dict)
        projected["warm_open"] = {"timeRegression": False, "incrementAllowed": False}
        self.assertFalse(call(peak_failure, result_projection=bad_projection))
        retention_failure = policy()
        retention = retention_failure["retention"]
        assert isinstance(retention, dict)
        retention["retention_allowed"] = False
        self.assertFalse(call(retention_failure))

    def test_rejects_missing_independent_projection(self) -> None:
        self.assertFalse(exception.allows_issue_126_initial_rss(
            release_version=exception.RELEASE_VERSION, issue_number=exception.ISSUE_NUMBER,
            candidate_head=exception.CANDIDATE_HEAD,
            bundle_manifest_sha256=exception.BUNDLE_MANIFEST_SHA256,
            condition_id="events/expected-state/620x900@1",
            policy_version=exception.MEMORY_POLICY_VERSION,
            canonical_policy=policy(), projection=None,
        ))
