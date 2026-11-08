from __future__ import annotations

import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path


class PublishCratesTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name).resolve()
        self.source = self.root / "source"
        scripts = self.source / "scripts/release"
        scripts.mkdir(parents=True)
        repository = Path(__file__).parents[2]
        (scripts / "publish-crates.sh").write_bytes(
            (repository / "scripts/release/publish-crates.sh").read_bytes()
        )
        (scripts / "verify-version.sh").write_text(
            '#!/usr/bin/env bash\necho "version_bare=${1#v}"\n', encoding="utf-8"
        )
        (scripts / "verify_completed_release_context.py").write_text(
            "import os, pathlib, sys\n"
            "args = sys.argv[1:]\n"
            "expected = [\n"
            "    '--repo', os.environ['RELEASE_REPO'],\n"
            "    '--head', os.environ['RELEASE_HEAD'],\n"
            "    '--version', os.environ['RELEASE_VERSION'],\n"
            "    '--acquisition-receipt', os.environ['RELEASE_RECEIPT'],\n"
            "]\n"
            "with pathlib.Path(os.environ['CARGO_LOG']).open('a') as stream:\n"
            "    stream.write('verify\\n')\n"
            "if args != expected: raise SystemExit('release verifier argv differs')\n"
            "if os.environ.get('RELEASE_GATE_FAIL') == 'true':\n"
            "    raise SystemExit('coverage does not satisfy 100/0')\n",
            encoding="utf-8",
        )
        subprocess.run(["git", "init", "-q", str(self.source)], check=True)
        subprocess.run(
            ["git", "-C", str(self.source), "config", "user.name", "Fixture"], check=True
        )
        subprocess.run(
            ["git", "-C", str(self.source), "config", "user.email", "fixture@example.invalid"],
            check=True,
        )
        receipt = self.source / "tmp/html-performance-release-evidence/acquisition.json"
        receipt.parent.mkdir(parents=True)
        receipt.write_text("{}\n", encoding="utf-8")
        subprocess.run(["git", "-C", str(self.source), "add", "-A"], check=True)
        subprocess.run(["git", "-C", str(self.source), "commit", "-qm", "fixture base"], check=True)
        cargo_bin = self.root / "bin"
        cargo_bin.mkdir()
        cargo = cargo_bin / "cargo"
        cargo.write_text(
            "#!/usr/bin/env python3\n"
            "import json, os, pathlib, sys\n"
            "args = sys.argv[1:]\n"
            "log = pathlib.Path(os.environ['CARGO_LOG'])\n"
            "with log.open('a') as stream: stream.write(' '.join(args) + '\\n')\n"
            "if args[:2] == ['metadata', '--no-deps']:\n"
            " print(pathlib.Path(os.environ['CARGO_METADATA']).read_text()); sys.exit(0)\n"
            "if args[0] == 'info':\n"
            " package = args[1].split('@')[0]\n"
            " published = pathlib.Path(os.environ['CARGO_PUBLISHED']).read_text().splitlines() if pathlib.Path(os.environ['CARGO_PUBLISHED']).exists() else []\n"
            " sys.exit(0 if package in published else 1)\n"
            "if args[0] == 'publish':\n"
            " package = args[args.index('-p') + 1]\n"
            " with pathlib.Path(os.environ['CARGO_PUBLISHED']).open('a') as stream: stream.write(package + '\\n')\n"
            " sys.exit(0)\n"
            "sys.exit(2)\n",
            encoding="utf-8",
        )
        cargo.chmod(0o755)
        self.log = self.root / "cargo.log"
        self.published = self.root / "published.txt"

    def tearDown(self) -> None:
        self.temp.cleanup()

    def run_publisher(
        self,
        version: str,
        names: list[str],
        *,
        gate_failure: bool = False,
        missing_verifier: bool = False,
    ) -> subprocess.CompletedProcess[str]:
        packages = []
        for name in names:
            manifest = self.source / "crates" / name / "Cargo.toml"
            manifest.parent.mkdir(parents=True, exist_ok=True)
            manifest.write_text("[package]\n", encoding="utf-8")
            packages.append(
                {
                    "name": name,
                    "version": version,
                    "source": None,
                    "manifest_path": str(manifest),
                }
            )
        metadata_path = self.root / "metadata.json"
        metadata_path.write_text(
            json.dumps({"workspace_root": str(self.source), "packages": packages}),
            encoding="utf-8",
        )
        subprocess.run(["git", "-C", str(self.source), "add", "-A"], check=True)
        subprocess.run(
            ["git", "-C", str(self.source), "commit", "-qm", "fixture packages"], check=True
        )
        verifier = self.source / "scripts/release/verify_completed_release_context.py"
        if missing_verifier:
            verifier.unlink()
        head = subprocess.run(
            [
                "git", "--no-replace-objects", "-C", str(self.source),
                "rev-parse", "--verify", "HEAD",
            ],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
        receipt = self.source / "tmp/html-performance-release-evidence/acquisition.json"
        environment = os.environ.copy()
        environment.update(
            {
                "PATH": f"{self.root / 'bin'}:{environment['PATH']}",
                "CARGO_METADATA": str(metadata_path),
                "CARGO_LOG": str(self.log),
                "CARGO_PUBLISHED": str(self.published),
                "CARGO_REGISTRY_TOKEN": "test-token",
                "PUBLISH_ATTEMPTS": "1",
                "RELEASE_REPO": str(self.source),
                "RELEASE_HEAD": head,
                "RELEASE_VERSION": version,
                "RELEASE_RECEIPT": str(receipt),
                "RELEASE_GATE_FAIL": "true" if gate_failure else "false",
            }
        )
        return subprocess.run(
            ["bash", str(self.source / "scripts/release/publish-crates.sh"), f"v{version}"],
            cwd=self.source,
            env=environment,
            capture_output=True,
            text=True,
            check=False,
        )

    def published_packages(self) -> list[str]:
        return self.published.read_text(encoding="utf-8").splitlines() if self.published.exists() else []

    def test_publishes_current_three_package_release_in_dependency_order(self) -> None:
        result = self.run_publisher(
            "0.4.23",
            [
                "katana-render-runtime",
                "katana-render-runtime-assets",
                "katana-render-runtime-cli",
            ],
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.published_packages(),
            ["katana-render-runtime-assets", "katana-render-runtime", "katana-render-runtime-cli"],
        )
        self.assertEqual(
            self.log.read_text(encoding="utf-8").splitlines()[:2],
            ["verify", "metadata --no-deps --format-version 1"],
        )

    def test_historical_release_publishes_its_two_packages(self) -> None:
        result = self.run_publisher(
            "0.4.22", ["katana-render-runtime", "katana-render-runtime-cli"]
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.published_packages(),
            ["katana-render-runtime", "katana-render-runtime-cli"],
        )

    def test_current_release_fails_closed_when_assets_package_is_missing(self) -> None:
        result = self.run_publisher(
            "0.4.23", ["katana-render-runtime", "katana-render-runtime-cli"]
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("required package missing", result.stderr)
        self.assertEqual(self.published_packages(), [])
        self.assertNotIn("info", self.log.read_text(encoding="utf-8"))

    def test_rejected_coverage_gate_prevents_all_cargo_attempts(self) -> None:
        result = self.run_publisher(
            "0.4.23",
            ["katana-render-runtime-assets", "katana-render-runtime", "katana-render-runtime-cli"],
            gate_failure=True,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("coverage does not satisfy 100/0", result.stderr)
        self.assertEqual(self.log.read_text(encoding="utf-8").splitlines(), ["verify"])
        self.assertEqual(self.published_packages(), [])

    def test_missing_release_verifier_prevents_all_cargo_attempts(self) -> None:
        result = self.run_publisher(
            "0.4.23",
            ["katana-render-runtime-assets", "katana-render-runtime", "katana-render-runtime-cli"],
            missing_verifier=True,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.log.exists())
        self.assertEqual(self.published_packages(), [])


if __name__ == "__main__":
    unittest.main()
