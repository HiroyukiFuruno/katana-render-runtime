"""有限な実OS identity/lifecycle binding回帰。Bun/browserは起動しない。"""
from __future__ import annotations

import json
import sys
import tempfile
import time
import unittest
from pathlib import Path
from collections.abc import Callable
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "build"))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "release"))
sys.path.insert(0, str(Path(__file__).resolve().parent))
import cargo_capacity_guard_native_cli_observation as native_guard
import produce_html_performance_evidence as owner
import produce_raster_process_evidence as producer


class RasterProcessEvidenceTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.repo_root = Path(__file__).resolve().parents[2]
        self.attempt = self.root / "attempt"
        self.attempt.mkdir()
        self.config = self.root / "config.json"
        self.receipt = self.attempt / "raster-production-receipt.json"
        self.receipt.write_bytes(b"diagnostic receipt\n")
        self.config.write_text(json.dumps({
            "schema": producer.SESSION_SCHEMA, "diagnosticOnly": True, "accepted": False,
            "role": "official", "attempt": str(self.attempt), "nonce": "config-nonce",
            "inventory": ["unit"], "outputDir": str(self.root / "output"),
            "observerPython": str(self.root / "observer.py"), "endpoints": [],
        }))
        self.config_sha = native_guard.file_record(self.config)["sha256"]
        self.started_ns = time.time_ns() - 1_000_000_000
        self.ended_ns = time.time_ns() + 1_000_000_000
        self.browser_pid = 101
        self.browser_start = "1770000000.000000"
        self.collector_reaped = True

    def owner_result(self, *, pid: int = 101, exit_code: int = 0,
                     reaped: bool = True, raw_fds_closed: bool = True) -> owner.NativeCliCommandEvidence:
        lifecycle_path = self.attempt / "lifecycle.json"
        lifecycle_path.write_text(json.dumps({
            "schema": "krr-performance-command-lifecycle-v1", "identity": {"pid": pid},
            "groupClosed": True, "rawFdsClosed": raw_fds_closed, "failure": None,
            "cleanupFailure": None, "exitCode": exit_code,
        }))
        lifecycle = native_guard.file_record(lifecycle_path)
        observation = {
            "argv": ["/bin/sh", "run", "scripts/mermaid/diagram-update.ts"],
            "stdout": {"path": str(self.attempt / "stdout.raw"), "sha256": "0" * 64, "size": 0},
            "stderr": {"path": str(self.attempt / "stderr.raw"), "sha256": "0" * 64, "size": 0},
            "label": "mermaid-raster-official", "cwd": str(self.root), "attemptToken": "a" * 64,
            "environment": {}, "environmentSha256": "0" * 64, "pid": pid,
            "actualExit": exit_code, "startedNs": str(self.started_ns), "endedNs": str(self.ended_ns),
            "reaped": reaped, "errors": [],
        }
        return owner.NativeCliCommandEvidence(observation, lifecycle)

    def write_browser(self, *, producer_pid: int = 101, producer_start: str = "1770000000.000000",
                      collector_reaped: bool = True, distinct_collectors: bool = True) -> None:
        browser = {
            "pid": 103, "parentPid": producer_pid, "startIdentity": "1770000001.000000",
            "executablePath": "/bin/sh", "argv": ["/bin/sh", "--headless"],
        }
        producer_row: dict[str, object] = {"pid": producer_pid, "parentPid": 1, "startIdentity": producer_start}
        endpoint_identity = {"path": "/bin/sh", "canonical": "/bin/sh", "sha256": "0" * 64,
                             "size": 1, "dev": "1", "ino": "1", "mode": 0o100755}
        for index in range(2):
            collector_pid = 104 + index if distinct_collectors else 104
            measurement: dict[str, object] = {
                "schema": "katana-macos-owned-browser-process-v1", "platform": "darwin",
                "browser": browser, "producer": producer_row,
                "parentChain": [{"pid": producer_pid, "parentPid": 1, "startIdentity": producer_start}],
                "collector": {"pid": collector_pid, "parentPid": producer_pid,
                               "startIdentity": f"1770000001.{100000 + index:06d}"},
                "beforeEndEqual": True,
            }
            value = {
                "schema": producer.IDENTITY_SCHEMA, "role": "official", "attempt": str(self.attempt),
                "nonce": "config-nonce", "cdpBrowserPid": 103, "osMeasurement": measurement,
                "collectorExitCode": 0, "collectorReaped": collector_reaped, "collectorPid": collector_pid,
                "python": {}, "collector": {},
                "endpoint": {"name": "actual-browser", "kind": "browser", "version": "unit",
                             "before": endpoint_identity, "end": endpoint_identity},
            }
            (self.attempt / f"browser-identity-{index}.json").write_text(json.dumps(value))

    def run_binding(self, result: owner.NativeCliCommandEvidence,
                    mutate: Callable[[], None] | None = None) -> None:
        def run_owner(*_args: object, **_kwargs: object) -> owner.NativeCliCommandEvidence:
            self.write_browser(producer_pid=self.browser_pid, producer_start=self.browser_start,
                               collector_reaped=self.collector_reaped)
            if mutate is not None:
                mutate()
            return result
        with patch.object(owner, "observe_native_cli_command", side_effect=run_owner):
            producer.bind_process_evidence(
                self.repo_root, self.config, str(self.config_sha), "official", Path("/bin/sh"),
                self.root, self.root, self.root / "output", "*.md", Path("/bin/sh"), Path("/bin/sh"),
            )

    def test_binds_direct_owner_and_keeps_collector_reap_separate(self) -> None:
        # 1770000000.000000 is intentionally outside the actual wall interval; use current time.
        start_identity = f"{self.started_ns // 1_000_000_000}.{(self.started_ns // 1_000) % 1_000_000:06d}"
        self.browser_start = start_identity
        self.run_binding(self.owner_result())
        document = json.loads((self.attempt / "raster-process-evidence.json").read_text())
        self.assertEqual(document["browserProcess"]["producerPid"], 101)
        self.assertIs(document["browserProcess"]["reaped"], True)
        self.assertIs(document["descendantClosureProved"], False)

    def test_rejects_pid_start_exit_reap_raw_fd_and_collector_mixup(self) -> None:
        mutations = ("pid", "start", "exit", "missing-exit", "reap", "missing-reap", "raw-fd", "collector")
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                self.setUp()
                start_identity = f"{self.started_ns // 1_000_000_000}.{(self.started_ns // 1_000) % 1_000_000:06d}"
                self.browser_pid = 999 if mutation == "pid" else 101
                self.browser_start = "1.000000" if mutation == "start" else start_identity
                self.collector_reaped = mutation != "collector"
                result = self.owner_result(exit_code=1 if mutation == "exit" else 0,
                                           reaped=mutation != "reap", raw_fds_closed=mutation != "raw-fd")
                if mutation == "missing-exit":
                    result.observation.pop("actualExit")
                if mutation == "missing-reap":
                    result.observation.pop("reaped")
                with self.assertRaisesRegex((RuntimeError, ValueError), ".+"):
                    self.run_binding(result)

    def test_controlled_official_command_has_no_markdown_rewrite(self) -> None:
        command = producer._known_command(
            self.repo_root, producer._load_session(self.config, str(self.config_sha)),
            self.config, str(self.config_sha), "official", Path("/bin/sh"), self.root,
            self.root, self.root / "output", "*.md", Path("/bin/sh"), Path("/bin/sh"),
        )
        self.assertNotIn("--markdown-output", command)

    def test_raster_child_environment_keeps_render_inputs_and_drops_credentials(self) -> None:
        supplied = {
            "PATH": "/usr/bin", "HOME": str(self.root), "USER": "renderer",
            "LANG": "ja_JP.UTF-8", "LC_CTYPE": "ja_JP.UTF-8", "TMPDIR": str(self.root),
            "FONTCONFIG_FILE": str(self.root / "fonts.conf"),
            "PLAYWRIGHT_BROWSERS_PATH": str(self.root / "browsers"), "NODE_ENV": "production",
            "TZ": "Asia/Tokyo", "AWS_SECRET_ACCESS_KEY": "synthetic-secret",
            "TEMP": str(self.root), "SystemRoot": str(self.root / "windows"),
            "USERPROFILE": str(self.root / "profile"), "LOCALAPPDATA": str(self.root / "local"),
            "DISPLAY": ":9", "WAYLAND_DISPLAY": "wayland-test", "XAUTHORITY": str(self.root / "xauth"),
            "XDG_RUNTIME_DIR": str(self.root / "runtime"), "XDG_DATA_DIRS": str(self.root / "data-dirs"),
            "XDG_CONFIG_DIRS": str(self.root / "config-dirs"),
            "UNRELATED_PRIVATE_SETTING": "synthetic-private",
        }
        child = producer.raster_child_environment(supplied)
        self.assertEqual(child, {key: supplied[key] for key in (
            "PATH", "HOME", "USER", "LANG", "LC_CTYPE", "TMPDIR", "FONTCONFIG_FILE",
            "PLAYWRIGHT_BROWSERS_PATH", "NODE_ENV", "TZ", "TEMP", "SystemRoot", "USERPROFILE",
            "LOCALAPPDATA", "DISPLAY", "WAYLAND_DISPLAY", "XAUTHORITY", "XDG_RUNTIME_DIR",
            "XDG_DATA_DIRS", "XDG_CONFIG_DIRS",
        )})
        self.assertNotIn("AWS_SECRET_ACCESS_KEY", child)
        self.assertNotIn("UNRELATED_PRIVATE_SETTING", child)

    def test_raster_child_environment_rejects_runtime_and_unknown_render_overrides(self) -> None:
        for key in (
            "NODE_OPTIONS", "BUN_OPTIONS", "FONTCONFIG_UNREVIEWED_SETTING", "FREETYPE_RENDER_HINT",
            "PANGO_SCALE", "PANGOCAIRO_BACKEND", "SKIA_CACHE_PATH", "LIBGL_ALWAYS_SOFTWARE",
            "MESA_GL_VERSION_OVERRIDE", "VK_ICD_FILENAMES",
        ):
            with self.subTest(key=key):
                with self.assertRaisesRegex(RuntimeError, "unsupported raster environment keys"):
                    producer.raster_child_environment({"PATH": "/usr/bin", key: "synthetic-value"})

    def test_spawn_observer_receives_only_the_same_filtered_environment(self) -> None:
        start_identity = f"{self.started_ns // 1_000_000_000}.{(self.started_ns // 1_000) % 1_000_000:06d}"
        self.browser_start = start_identity
        supplied = {
            "PATH": "/usr/bin", "HOME": str(self.root), "LANG": "ja_JP.UTF-8",
            "TMPDIR": str(self.root), "FONTCONFIG_PATH": str(self.root / "fonts"),
            "GITHUB_TOKEN": "synthetic-secret", "NODE_OPTIONS": "",
        }
        expected = producer.raster_child_environment(supplied)

        def run_owner(*_args: object, **kwargs: object) -> owner.NativeCliCommandEvidence:
            self.assertEqual(kwargs.get("env"), expected)
            self.write_browser(producer_pid=self.browser_pid, producer_start=self.browser_start,
                               collector_reaped=self.collector_reaped)
            return self.owner_result()

        with patch.dict("os.environ", supplied, clear=True):
            with patch.object(owner, "observe_native_cli_command", side_effect=run_owner):
                producer.bind_process_evidence(
                    self.repo_root, self.config, str(self.config_sha), "official", Path("/bin/sh"),
                    self.root, self.root, self.root / "output", "*.md", Path("/bin/sh"), Path("/bin/sh"),
                )

    def test_unsupported_render_environment_fails_before_observer(self) -> None:
        with patch.dict("os.environ", {"PATH": "/usr/bin", "HOME": str(self.root),
                                       "FONTCONFIG_UNREVIEWED_SETTING": "synthetic-value"}, clear=True):
            with patch.object(owner, "observe_native_cli_command") as observed:
                with self.assertRaisesRegex(RuntimeError, "unsupported raster environment keys"):
                    producer.bind_process_evidence(
                        self.repo_root, self.config, str(self.config_sha), "official", Path("/bin/sh"),
                        self.root, self.root, self.root / "output", "*.md", Path("/bin/sh"), Path("/bin/sh"),
                    )
                observed.assert_not_called()

    def test_existing_proof_and_output_mismatch_rejected_before_child(self) -> None:
        owner_path = self.attempt / "raster-process-owner-observation.json"
        owner_path.write_text("prior\n")
        with patch.object(owner, "observe_native_cli_command") as observed:
            with self.assertRaisesRegex(RuntimeError, "already contains"):
                producer.bind_process_evidence(
                    self.repo_root, self.config, str(self.config_sha), "official", Path("/bin/sh"),
                    self.root, self.root, self.root / "output", "*.md", Path("/bin/sh"), Path("/bin/sh"),
                )
            observed.assert_not_called()
        owner_path.unlink()
        with patch.object(owner, "observe_native_cli_command") as observed:
            with self.assertRaisesRegex(RuntimeError, "differs from config"):
                producer.bind_process_evidence(
                    self.repo_root, self.config, str(self.config_sha), "official", Path("/bin/sh"),
                    self.root, self.root, self.root / "other-output", "*.md", Path("/bin/sh"), Path("/bin/sh"),
                )
            observed.assert_not_called()

    def test_changed_endpoint_between_live_observations_is_rejected(self) -> None:
        def change_endpoint() -> None:
            path = self.attempt / "browser-identity-1.json"
            value = json.loads(path.read_text())
            value["endpoint"]["version"] = "changed"
            path.write_text(json.dumps(value))

        start_identity = f"{self.started_ns // 1_000_000_000}.{(self.started_ns // 1_000) % 1_000_000:06d}"
        self.browser_start = start_identity
        with self.assertRaisesRegex((RuntimeError, ValueError), ".+"):
            self.run_binding(self.owner_result(), mutate=change_endpoint)


if __name__ == "__main__":
    unittest.main()
