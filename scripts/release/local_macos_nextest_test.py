import hashlib
import importlib.util
import io
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path.cwd() / "scripts/release"))
spec = importlib.util.spec_from_file_location("proposal_evidence", Path(__file__).with_name("local_macos_evidence.py"))
assert spec and spec.loader
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def archive_with(name: str, data: bytes) -> bytes:
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w:gz") as archive:
        info = tarfile.TarInfo(name)
        info.size = len(data)
        archive.addfile(info, io.BytesIO(data))
    return buffer.getvalue()


class NextestBindingTest(unittest.TestCase):
    def test_official_binary_is_available_to_cargo_from_isolated_path(self) -> None:
        cargo = subprocess.run(["rustup", "which", "cargo"], capture_output=True, text=True, check=True).stdout.strip()
        if cargo is None:
            self.fail("cargo is required for this regression")
        binary = b"#!/bin/sh\nprintf 'cargo-nextest fixture\\n'\n"
        archive = archive_with("cargo-nextest", binary)
        with tempfile.TemporaryDirectory() as temp, patch.object(module, "NEXTEST_ARCHIVE_SHA256", hashlib.sha256(archive).hexdigest()), patch.object(module, "NEXTEST_BINARY_SHA256", hashlib.sha256(binary).hexdigest()):
            root = Path(temp)
            executable = module.materialize_trusted_nextest(archive, root)
            aliases = root / "aliases"
            aliases.mkdir()
            (aliases / "cargo-nextest").symlink_to(executable)
            env = dict(os.environ, PATH=str(aliases) + os.pathsep + "/usr/bin:/bin", CARGO_HOME=str(root / "cargo-home"))
            result = subprocess.run([cargo, "nextest", "--version"], env=env, capture_output=True, text=True, check=True)
            self.assertIn("cargo-nextest fixture", result.stdout)

    def test_wrong_binary_or_archive_is_rejected(self) -> None:
        archive = archive_with("cargo-nextest", b"wrong")
        with tempfile.TemporaryDirectory() as temp:
            with self.assertRaises(module.EvidenceError):
                module.materialize_trusted_nextest(archive, Path(temp))
            with patch.object(module, "NEXTEST_ARCHIVE_SHA256", hashlib.sha256(archive).hexdigest()):
                with self.assertRaises(module.EvidenceError):
                    module.materialize_trusted_nextest(archive, Path(temp))

    def test_unsafe_member_is_rejected_even_with_matching_archive_digest(self) -> None:
        archive = archive_with("../cargo-nextest", b"bad")
        with tempfile.TemporaryDirectory() as temp, patch.object(module, "NEXTEST_ARCHIVE_SHA256", hashlib.sha256(archive).hexdigest()):
            with self.assertRaises(module.EvidenceError):
                module.materialize_trusted_nextest(archive, Path(temp))


if __name__ == "__main__":
    unittest.main()
