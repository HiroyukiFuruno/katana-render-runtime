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
        self.root = Path(self.temp.name)
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

    def run_publisher(self, version: str, names: list[str]) -> subprocess.CompletedProcess[str]:
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
        environment = os.environ.copy()
        environment.update(
            {
                "PATH": f"{self.root / 'bin'}:{environment['PATH']}",
                "CARGO_METADATA": str(metadata_path),
                "CARGO_LOG": str(self.log),
                "CARGO_PUBLISHED": str(self.published),
                "CARGO_REGISTRY_TOKEN": "test-token",
                "PUBLISH_ATTEMPTS": "1",
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


if __name__ == "__main__":
    unittest.main()
