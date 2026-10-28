"""Strict Cargo.lock closure comparison regressions for HTML performance arms."""
from __future__ import annotations

import hashlib
import unittest

from html_performance_harness_dependency_closure import verify_harness_closure


def _lock(shared_version: str = "1.0.0", sdk_version: str = "0.4.23",
          sdk_child_version: str = "1.0.0", harness_edges: str = '"serde 1.0.0"',
          serde_edges: str = '"shared 1.0.0"') -> bytes:
    packages = [
        f'''[[package]]\nname = "krr-html-performance-harness"\nversion = "0.1.0"\ndependencies = [{harness_edges}, "katana-render-runtime {sdk_version}"]''',
        f'''[[package]]\nname = "serde"\nversion = "1.0.0"\nsource = "registry+https://example.invalid/index"\nchecksum = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"\ndependencies = [{serde_edges.replace("1.0.0", shared_version)}]''',
        f'''[[package]]\nname = "shared"\nversion = "{shared_version}"\nsource = "registry+https://example.invalid/index"\nchecksum = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"''',
        f'''[[package]]\nname = "katana-render-runtime"\nversion = "{sdk_version}"\ndependencies = ["sdk-child {sdk_child_version}"]''',
        f'''[[package]]\nname = "sdk-child"\nversion = "{sdk_child_version}"\nsource = "registry+https://example.invalid/index"\nchecksum = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"''',
    ]
    return ("version = 3\n\n" + "\n\n".join(packages) + "\n").encode()


class HarnessDependencyClosureTest(unittest.TestCase):
    def test_sdk_only_lock_changes_are_accepted(self) -> None:
        parent = _lock("1.0.0", "0.4.23", "1.0.0")
        current = _lock("1.0.0", "0.4.24", "2.0.0")
        verify_harness_closure(parent, current, hashlib.sha256(parent).hexdigest())

    def test_changed_shared_dependency_is_rejected(self) -> None:
        parent = _lock("1.0.0", "0.4.23", "1.0.0")
        current = _lock("1.1.0", "0.4.24", "2.0.0")
        with self.assertRaisesRegex(ValueError, "closure differs"):
            verify_harness_closure(parent, current, hashlib.sha256(parent).hexdigest())

    def test_changed_checksum_on_same_identity_is_rejected(self) -> None:
        parent = _lock()
        current = parent.replace(b'checksum = "aaaa', b'checksum = "dddd', 1)
        with self.assertRaisesRegex(ValueError, "closure differs"):
            verify_harness_closure(parent, current, hashlib.sha256(parent).hexdigest())

    def test_harness_edge_change_is_rejected(self) -> None:
        parent = _lock()
        current = _lock(harness_edges='"serde 1.0.0", "shared 1.0.0"')
        with self.assertRaises(ValueError):
            verify_harness_closure(parent, current, hashlib.sha256(parent).hexdigest())

    def test_non_sdk_dependency_edge_to_sdk_is_rejected(self) -> None:
        parent = _lock()
        current = _lock("1.0.0", "0.4.24", "2.0.0",
                        serde_edges='"shared 1.0.0", "katana-render-runtime 0.4.24"')
        with self.assertRaisesRegex(ValueError, "edge reaches the SDK"):
            verify_harness_closure(parent, current, hashlib.sha256(parent).hexdigest())

    def test_parent_pin_and_malformed_lock_fail_closed(self) -> None:
        parent = _lock()
        with self.assertRaisesRegex(ValueError, "immutable pin"):
            verify_harness_closure(parent, parent, "0" * 64)
        with self.assertRaisesRegex(ValueError, "valid UTF-8 TOML"):
            verify_harness_closure(b"[[package]\n", parent, hashlib.sha256(b"[[package]\n").hexdigest())

    def test_ambiguous_bare_dependency_is_rejected(self) -> None:
        ambiguous = _lock(harness_edges='"serde"') + b'''\n[[package]]\nname = "serde"\nversion = "2.0.0"\nsource = "registry+https://example.invalid/index"\nchecksum = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"\n'''
        with self.assertRaisesRegex(ValueError, "does not resolve uniquely"):
            verify_harness_closure(ambiguous, ambiguous, hashlib.sha256(ambiguous).hexdigest())

    def test_missing_dependency_and_multiple_source_identity_fail_closed(self) -> None:
        missing = _lock(harness_edges='"absent 1.0.0"')
        with self.assertRaisesRegex(ValueError, "does not resolve uniquely"):
            verify_harness_closure(missing, missing, hashlib.sha256(missing).hexdigest())
        duplicate = _lock() + b'''\n[[package]]\nname = "serde"\nversion = "1.0.0"\nsource = "registry+https://other.invalid/index"\nchecksum = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"\n'''
        with self.assertRaisesRegex(ValueError, "multiple identities"):
            verify_harness_closure(duplicate, duplicate, hashlib.sha256(duplicate).hexdigest())


if __name__ == "__main__":
    unittest.main()
