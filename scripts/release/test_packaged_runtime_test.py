from __future__ import annotations

import hashlib
import io
import json
import tarfile
import tempfile
import unittest
from pathlib import Path

import test_packaged_runtime as packaged


class PackagedRuntimeTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        self.archive = self.root / f"{packaged.ASSET_CRATE}-1.2.3.crate"

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def write_archive(self, members: dict[str, bytes], *, symlink: bool = False) -> None:
        with tarfile.open(self.archive, mode="w:gz") as tar:
            for name, content in members.items():
                info = tarfile.TarInfo(name)
                info.size = len(content)
                tar.addfile(info, io.BytesIO(content))
            if symlink:
                info = tarfile.TarInfo(f"{packaged.ASSET_CRATE}-1.2.3/link")
                info.type = tarfile.SYMTYPE
                info.linkname = "../../outside"
                tar.addfile(info)

    def valid_members(self) -> dict[str, bytes]:
        return {
            f"{packaged.ASSET_CRATE}-1.2.3/Cargo.toml": b"[package]\n",
            f"{packaged.ASSET_CRATE}-1.2.3/src/lib.rs": b"pub static BYTES: &[u8] = b\"asset\";\n",
        }

    def lockfile(self, checksum: str, *, source: str = packaged.REGISTRY_SOURCE) -> Path:
        path = self.root / "Cargo.lock"
        path.write_text(
            "version = 4\n\n[[package]]\n"
            f'name = "{packaged.ASSET_CRATE}"\nversion = "1.2.3"\n'
            f'source = "{source}"\nchecksum = "{checksum}"\n',
            encoding="utf-8",
        )
        return path

    def test_archive_files_and_lock_checksum_match(self) -> None:
        self.write_archive(self.valid_members())
        checksum = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        self.assertEqual(
            set(packaged.asset_archive_members(self.archive, "1.2.3")),
            {"Cargo.toml", "src/lib.rs"},
        )
        self.assertEqual(
            packaged.verify_asset_lock_entry(self.lockfile(checksum), self.archive, "1.2.3"),
            checksum,
        )

    def test_lock_checksum_mismatch_is_rejected(self) -> None:
        self.write_archive(self.valid_members())
        with self.assertRaisesRegex(ValueError, "does not match"):
            packaged.verify_asset_lock_entry(self.lockfile("0" * 64), self.archive, "1.2.3")

    def test_non_registry_asset_lock_entry_is_rejected(self) -> None:
        self.write_archive(self.valid_members())
        checksum = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        with self.assertRaisesRegex(ValueError, "crates.io registry source"):
            packaged.verify_asset_lock_entry(
                self.lockfile(checksum, source="path+file:///tmp/assets"), self.archive, "1.2.3"
            )

    def test_unsafe_or_nonregular_archive_members_are_rejected(self) -> None:
        self.write_archive({f"{packaged.ASSET_CRATE}-1.2.3/../../outside": b"bad"})
        with self.assertRaisesRegex(ValueError, "unsafe"):
            packaged.asset_archive_members(self.archive, "1.2.3")
        for unsafe_name in (
            f"{packaged.ASSET_CRATE}-1.2.3/sub\\..\\outside",
            f"{packaged.ASSET_CRATE}-1.2.3/C:/outside",
        ):
            with self.subTest(path=unsafe_name):
                self.write_archive({unsafe_name: b"bad"})
                with self.assertRaisesRegex(ValueError, "unsafe"):
                    packaged.asset_archive_members(self.archive, "1.2.3")
        self.write_archive(self.valid_members(), symlink=True)
        with self.assertRaisesRegex(ValueError, "unsupported.*member type"):
            packaged.asset_archive_members(self.archive, "1.2.3")

    def test_stage_is_byte_identical_and_repeatable(self) -> None:
        members = self.valid_members()
        self.write_archive(members)
        destination = self.root / "vendor" / f"{packaged.ASSET_CRATE}-1.2.3"
        expected_archive_sha = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        self.assertEqual(
            packaged.stage_asset_archive(self.archive, destination, "1.2.3"), expected_archive_sha
        )
        manifest = json.loads((destination / ".cargo-checksum.json").read_text(encoding="utf-8"))
        self.assertEqual(manifest["package"], expected_archive_sha)
        expected_checksums = {
            name.split("/", 1)[1]: hashlib.sha256(content).hexdigest()
            for name, content in members.items()
        }
        self.assertEqual(manifest["files"], expected_checksums)
        for name, content in members.items():
            self.assertEqual((destination / name.split("/", 1)[1]).read_bytes(), content)
        self.assertEqual(
            packaged.stage_asset_archive(self.archive, destination, "1.2.3"), expected_archive_sha
        )

    def test_stage_refuses_to_replace_unowned_directory(self) -> None:
        self.write_archive(self.valid_members())
        destination = self.root / "vendor" / f"{packaged.ASSET_CRATE}-1.2.3"
        destination.mkdir(parents=True)
        (destination / "keep.txt").write_text("keep", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "unowned"):
            packaged.stage_asset_archive(self.archive, destination, "1.2.3")
        self.assertEqual((destination / "keep.txt").read_text(encoding="utf-8"), "keep")

    def test_existing_vendor_manifest_rejects_windows_paths(self) -> None:
        destination = self.root / "vendor" / f"{packaged.ASSET_CRATE}-1.2.3"
        destination.mkdir(parents=True)
        for unsafe_name in (r"..\outside", "C:outside"):
            with self.subTest(path=unsafe_name):
                content = b"outside"
                (destination / unsafe_name).write_bytes(content)
                manifest = {
                    "files": {unsafe_name: hashlib.sha256(content).hexdigest()},
                    "package": "0" * 64,
                }
                (destination / ".cargo-checksum.json").write_text(
                    json.dumps(manifest), encoding="utf-8"
                )
                with self.assertRaisesRegex(ValueError, "unsafe existing vendor manifest path"):
                    packaged._read_owned_vendor_manifest(destination)
                (destination / unsafe_name).unlink()

    def test_test_command_keeps_registry_lock_and_enables_offline_vendor_source(self) -> None:
        command = packaged.packaged_test_command(
            ["rtk", "proxy", "cargo"],
            Path("target/package/runtime/Cargo.toml"),
            Path("target/package/vendor"),
            1,
        )
        self.assertIn("--locked", command)
        self.assertIn("--offline", command)
        self.assertIn('source.crates-io.replace-with="packaged-runtime-test-vendor"', command)
        self.assertEqual(command[-2:], ["--test-threads", "1"])


if __name__ == "__main__":
    unittest.main()
