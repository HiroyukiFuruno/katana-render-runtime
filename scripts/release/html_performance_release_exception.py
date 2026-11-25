"""v0.4.24 の #126 初回RSS例外を狭く判定する。"""
from __future__ import annotations

from collections.abc import Mapping, Sequence
from pathlib import Path

import html_performance_memory_evidence as evidence
import html_performance_memory_policy as memory
import verify_html_performance_evidence as gate


ISSUE_NUMBER = 126
RELEASE_VERSION = "0.4.24"
CANDIDATE_HEAD = "d40d53daf9287f668e0f93ce1c11124658cc234a"
BUNDLE_MANIFEST_SHA256 = "ab53e45d18e00cd4143f4e51bed02f8bdef979ebb27de1d0b466dcbc630bba8f"
MEMORY_POLICY_VERSION = "html-memory-v7"
CONDITIONS = frozenset({
    "events/expected-state/620x900@1",
    "events/expected-state/960x720@1",
    "input-state/focus-visible/960x720@1",
})
PHASES = ("cold_open", "warm_open", "full_y", "post_operation", "re_display", "same_session_resize")


def post_cold_increment_projection(report_paths: Sequence[Path], expected: gate.JsonObject,
                                   retained_rss: Sequence[float]) -> Mapping[str, object] | None:
    """認証済みreportから、cold由来を除いた各phaseのRSSと時間を再計算する。"""
    try:
        grouped = gate._group_reports(list(report_paths), expected)
        baseline = evidence._validated_samples(grouped["baseline"])
        candidate = evidence._validated_samples(grouped["candidate"])
        if len(baseline) != memory.SAMPLE_COUNT or len(candidate) != memory.SAMPLE_COUNT:
            return None
        no_scroll = (evidence._proven_no_scroll(baseline) and evidence._proven_no_scroll(candidate))
        if no_scroll and not (evidence._full_y_rss_matches_re_display(baseline)
                              and evidence._full_y_rss_matches_re_display(candidate)):
            return None
        _, base_cold = evidence._phase_values(baseline, "cold_open", no_scroll=False)
        _, candidate_cold = evidence._phase_values(candidate, "cold_open", no_scroll=False)
        retention = memory.evaluate_retention(retained_rss, policy_version=MEMORY_POLICY_VERSION)
        phases: dict[str, object] = {}
        base_previous, candidate_previous = base_cold, candidate_cold
        for phase_name in PHASES:
            no_scroll_phase = no_scroll and phase_name == "full_y"
            base_time, base_peak = evidence._phase_values(baseline, phase_name, no_scroll=no_scroll_phase)
            candidate_time, candidate_peak = evidence._phase_values(candidate, phase_name, no_scroll=no_scroll_phase)
            timing = memory.evaluate_peak_phase(base_peak, candidate_peak, base_time, candidate_time,
                                                policy_version=MEMORY_POLICY_VERSION).time_regression
            if phase_name == "cold_open":
                increment_allowed = False
            else:
                if not all(base_value >= base_prior and candidate_value >= candidate_prior
                           for base_value, base_prior, candidate_value, candidate_prior in zip(
                               base_peak, base_previous, candidate_peak, candidate_previous, strict=True)):
                    return None
                differences = [
                    (candidate_value - candidate_initial) - (base_value - base_initial)
                    for base_value, base_initial, candidate_value, candidate_initial in zip(
                        base_peak, base_cold, candidate_peak, candidate_cold, strict=True)
                ]
                increment_allowed = all(
                    difference <= min(memory.SMALL_PEAK_BYTES, base_value * memory.SMALL_PEAK_RATIO)
                    for base_value, difference in zip(base_peak, differences, strict=True))
                base_previous, candidate_previous = base_peak, candidate_peak
            phases[phase_name] = {
                "timeRegression": timing.detected_bad,
                "incrementAllowed": increment_allowed,
            }
        return {"retentionAllowed": retention.retention_allowed, "noScrollFullY": no_scroll,
                "phases": phases}
    except (KeyError, TypeError, ValueError):
        return None


def allows_issue_126_initial_rss(*, release_version: str, issue_number: int,
                                 candidate_head: str, bundle_manifest_sha256: str,
                                 condition_id: str, policy_version: str,
                                 canonical_policy: Mapping[str, object],
                                 projection: Mapping[str, object] | None) -> bool:
    """初回RSSだけを許容し、時間・保持量・他phaseの不合格は残す。"""
    if (release_version, issue_number, candidate_head, bundle_manifest_sha256, policy_version) != (
        RELEASE_VERSION, ISSUE_NUMBER, CANDIDATE_HEAD, BUNDLE_MANIFEST_SHA256, MEMORY_POLICY_VERSION,
    ) or condition_id not in CONDITIONS:
        return False
    phases = canonical_policy.get("phases")
    retention = canonical_policy.get("retention")
    if projection is None:
        return False
    projection_phases = projection.get("phases")
    if not isinstance(phases, Mapping) or not isinstance(retention, Mapping) or not isinstance(projection_phases, Mapping):
        return False
    if (canonical_policy.get("policyPass") is not False or retention.get("retention_allowed") is not True
            or projection.get("retentionAllowed") is not True):
        return False
    if set(phases) != set(PHASES):
        return False
    for phase_name in PHASES:
        phase = phases.get(phase_name)
        if not isinstance(phase, Mapping):
            return False
        projection_phase = projection_phases.get(phase_name)
        if not isinstance(projection_phase, Mapping) or projection_phase.get("timeRegression") is not False:
            return False
        if phase_name == "cold_open":
            if phase.get("peak_allowed") is not False:
                return False
        elif projection_phase.get("incrementAllowed") is not True:
            return False
    return True
