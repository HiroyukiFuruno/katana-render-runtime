"""The historical harness accepts only the proven RSS extraction."""
from __future__ import annotations

import hashlib
import subprocess
import unittest
from pathlib import Path

import html_performance_harness_equivalence as equivalence


class HarnessEquivalenceTest(unittest.TestCase):
    def setUp(self) -> None:
        old_main = ("mod provenance;\nuse provenance::{arm_name, hex_digest, provenance};\n"
                    + equivalence._CONSTANTS
                    + "fn phase() { let peak_rss_bytes = peak_rss_bytes()?; }\n"
                    + equivalence._FUNCTION + "\n")
        current_main = ("mod provenance;\nmod rss;\n"
                        "use provenance::{arm_name, hex_digest, provenance};\nuse rss::ProcessRss;\n"
                        "fn phase() { let peak_rss_bytes = ProcessRss::peak_bytes()?; }\n")
        rss = (equivalence._CONSTANTS + "\npub(crate) struct ProcessRss;\n\nimpl ProcessRss {\n"
               "    pub(crate) fn peak_bytes() -> Result<u64, Box<dyn std::error::Error>> {\n"
               "        /* WHY: 同じgetrusageのピークRSSを保ち、ABIとOSエラー処理は既存ライブラリへ委譲する。 */\n"
               "        let usage = nix::sys::resource::getrusage(nix::sys::resource::UsageWho::RUSAGE_SELF)?;\n"
               "        let value = u64::try_from(usage.max_rss())?;\n"
               "        Ok(value.saturating_mul(BYTES_PER_RSS_UNIT))\n    }\n}\n")
        manifest = b'[package]\nname = "krr-html-performance-harness"\nversion = "0.1.0"\n'
        new_manifest = (manifest + b'default-run = "krr-html-performance-harness"\n'
                        b'[[bin]]\nname = "diagram_resource_probe"\npath = "src/bin/diagram_resource_probe.rs"\n')
        other = equivalence.ROOT + "src/measurement.rs"
        self.old = {equivalence.MANIFEST: manifest, equivalence.MAIN: old_main.encode(), other: b"timing"}
        probe = (Path(__file__).resolve().parents[2] / equivalence.PROBE).read_bytes()
        self.current = {equivalence.MANIFEST: new_manifest, equivalence.MAIN: current_main.encode(),
                        equivalence.RSS: rss.encode(), equivalence.PROBE: probe,
                        equivalence.PROBE_TEST: (Path(__file__).resolve().parents[2]
                                                 / equivalence.PROBE_TEST).read_bytes(),
                        other: b"timing"}

    def verify(self) -> None:
        def records(files: dict[str, bytes]) -> dict[str, tuple[str, int]]:
            return {path: (hashlib.sha256(data).hexdigest(), len(data)) for path, data in files.items()}
        def read(revision: str, path: str) -> bytes:
            return (self.current if revision == "HEAD" else self.old)[path]
        equivalence.verify_harness_sources(records(self.old), records(self.current), "old", read)

    def test_only_rss_extraction_and_independent_diagnostic_are_accepted(self) -> None:
        self.verify()

    def test_historical_two_file_diagnostic_remains_accepted(self) -> None:
        del self.current[equivalence.PROBE_TEST]
        self.current[equivalence.PROBE] = subprocess.check_output(
            ["git", "--no-replace-objects", "show", "3ac29dd6:" + equivalence.PROBE],
            cwd=Path(__file__).resolve().parents[2],
        )
        self.verify()

    def test_changed_driver_settings_timing_rss_and_unknown_modules_are_rejected(self) -> None:
        mutations = {
            "cargo_feature": (equivalence.MANIFEST, b'\n[features]\nchanged = []\n'),
            "timing": (equivalence.MAIN, b'\nfn timing_changed() {}\n'),
            "rss": (equivalence.RSS, b'\nfn alternate_rss() {}\n'),
            "probe": (equivalence.PROBE, b'\nfn unexpected() {}\n'),
            "probe_test": (equivalence.PROBE_TEST, b'\nfn unexpected() {}\n'),
            "module": (equivalence.ROOT + "src/unknown.rs", b"mod unexpected;"),
        }
        for name, (path, suffix) in mutations.items():
            with self.subTest(name=name):
                previous = self.current.get(path)
                self.current[path] = (previous or b"") + suffix
                with self.assertRaises(ValueError):
                    self.verify()
                if previous is None:
                    del self.current[path]
                else:
                    self.current[path] = previous

    def test_missing_diagnostic_and_modified_default_target_are_rejected(self) -> None:
        probe = self.current.pop(equivalence.PROBE)
        with self.assertRaises(ValueError):
            self.verify()
        self.current[equivalence.PROBE] = probe
        self.current[equivalence.MANIFEST] = self.current[equivalence.MANIFEST].replace(
            b'default-run = "krr-html-performance-harness"', b'default-run = "diagram_resource_probe"')
        with self.assertRaises(ValueError):
            self.verify()
