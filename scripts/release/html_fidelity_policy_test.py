from __future__ import annotations
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location("html_fidelity_policy", Path(__file__).with_name("html_fidelity_policy.py"))
assert SPEC and SPEC.loader
POLICY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(POLICY)


class HtmlPolicyTests(unittest.TestCase):
    def test_only_authorized_version_and_exact_refs_select_interim(self) -> None:
        for ref in ("release/v0.4.24", "refs/heads/release/v0.4.24", "master", "refs/heads/master", "refs/tags/v0.4.24"):
            value = POLICY.select_html_policy("0.4.24", ref)
            self.assertEqual((value["phase"], value["threshold"]), ("interim", 90))
            self.assertTrue(str(value["authorization"]).endswith("/issues/109"))
        for version, ref in (("0.4.23", "master"), ("0.4.25", "master"), ("0.4.25", "release/v0.4.25"), ("0.4.24", "feature/v0.4.24")):
            value = POLICY.select_html_policy(version, ref)
            self.assertEqual((value["phase"], value["threshold"]), ("final", 99))

    def test_mismatched_release_version_is_rejected(self) -> None:
        for ref in ("release/v0.4.24", "refs/tags/v0.4.24"):
            with self.assertRaisesRegex(ValueError, "version differ"):
                POLICY.select_html_policy("0.4.23", ref)

    def test_policy_threshold_version_and_authorization_cannot_be_overridden(self) -> None:
        original = json.loads((POLICY.ROOT / POLICY.POLICY_PATH).read_text())
        for field, value in (("threshold", 89), ("version", "0.4.25"), ("authorization", "https://example.com/109")):
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                target = root / POLICY.POLICY_PATH
                target.parent.mkdir(parents=True)
                mutated = json.loads(json.dumps(original))
                mutated["interimRelease"][field] = value
                target.write_text(json.dumps(mutated))
                with self.assertRaisesRegex(ValueError, "unauthorized"):
                    POLICY.select_html_policy("0.4.24", "master", root)

    def test_source_digest_changes_when_current_contract_changes(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for name in POLICY.SOURCE_PATHS:
                target = root / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes((POLICY.ROOT / name).read_bytes())
            before = POLICY.select_html_policy("0.4.24", "master", root)
            (root / "scripts/html/fidelity-phase.ts").write_text("changed source")
            after = POLICY.select_html_policy("0.4.24", "master", root)
            self.assertNotEqual(before["source_sha256"], after["source_sha256"])


if __name__ == "__main__":
    unittest.main()
