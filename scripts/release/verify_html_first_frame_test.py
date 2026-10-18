"""初期描画・closeの性能証跡が不足している場合は合格にしない。"""

import copy
import unittest

from verify_html_first_frame import process_passed, valid_evidence


class FirstFrameEvidenceTests(unittest.TestCase):
    def evidence(self) -> dict[str, object]:
        return {"closed": True, "first_frame_ms": 2500.0, "close_ms": 1.0,
                "frame": {"generation": 1, "width": 1280, "height": 900,
                          "scale": 1.0, "pixel_format": "Rgba8", "pixel_bytes": 4608000}}

    def test_actual_frame_and_successful_close_are_required(self) -> None:
        evidence = self.evidence()
        self.assertTrue(valid_evidence(evidence))
        for key, value in (("closed", False), ("frame", None),
                           ("first_frame_ms", 60000.0), ("close_ms", 60000.0),
                           ("first_frame_ms", float("nan")), ("close_ms", -1),
                           ("first_frame_ms", True)):
            with self.subTest(key=key, value=value):
                changed = copy.deepcopy(evidence)
                changed[key] = value
                self.assertFalse(valid_evidence(changed))

    def test_first_frame_has_strict_three_second_limit(self) -> None:
        evidence = self.evidence()
        evidence["first_frame_ms"] = 2999.999
        self.assertTrue(valid_evidence(evidence))
        evidence["first_frame_ms"] = 3000
        self.assertFalse(valid_evidence(evidence))

    def test_viewport_buffer_and_generation_must_match(self) -> None:
        for key, value in (("width", 800), ("height", 800), ("scale", True),
                           ("pixel_bytes", 0), ("pixel_format", "Rgb8"),
                           ("generation", 0), ("generation", True)):
            with self.subTest(key=key, value=value):
                changed = self.evidence()
                self.set_frame_value(changed, key, value)
                self.assertFalse(valid_evidence(changed))

    def set_frame_value(self, evidence: dict[str, object], key: str, value: object) -> None:
        frame = evidence.get("frame")
        if not isinstance(frame, dict):
            raise AssertionError("fixture frame must be a mapping")
        typed_frame: dict[str, object] = {}
        for frame_key, frame_value in frame.items():
            if not isinstance(frame_key, str):
                raise AssertionError("fixture frame keys must be strings")
            typed_frame[frame_key] = frame_value
        typed_frame[key] = value
        evidence["frame"] = typed_frame

    def test_missing_or_untyped_evidence_is_rejected(self) -> None:
        values: tuple[object, ...] = (None, [], {}, "success")
        for value in values:
            self.assertFalse(valid_evidence(value))
        for key in ("closed", "first_frame_ms", "close_ms", "frame"):
            changed = self.evidence()
            del changed[key]
            self.assertFalse(valid_evidence(changed))

    def test_terminal_process_and_whole_runtime_deadline_are_required(self) -> None:
        result = {"timed_out": False, "exit_code": 0, "elapsed_seconds": 40.0,
                  "remaining_process_group": False}
        self.assertTrue(process_passed(result, self.evidence()))
        for key, value in (("timed_out", True), ("exit_code", 1), ("exit_code", False),
                           ("elapsed_seconds", 60.0), ("elapsed_seconds", float("nan")),
                           ("remaining_process_group", True)):
            with self.subTest(key=key, value=value):
                changed = dict(result)
                changed[key] = value
                self.assertFalse(process_passed(changed, self.evidence()))
        self.assertFalse(process_passed({}, self.evidence()))
        self.assertFalse(process_passed(result, None))


if __name__ == "__main__":
    unittest.main()
