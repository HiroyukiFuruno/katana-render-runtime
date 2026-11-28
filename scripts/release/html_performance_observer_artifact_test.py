"""Exercise observer runtime artifact packaging with isolated fixture files."""

from __future__ import annotations

import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import stat
import tarfile
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from unittest.mock import patch

import html_performance_observer_artifact as artifact

POSIX_FILE_ACCESS = (
    os.name == "posix"
    and bool(getattr(os, "O_NOFOLLOW", 0))
    and bool(getattr(os, "O_DIRECTORY", 0))
    and os.open in getattr(os, "supports_dir_fd", set())
)


class ObserverArtifactTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.gn_out = self.root / "gn-out"
        self.gn_out.mkdir()
        (self.gn_out / "gen").mkdir()
        self.binary = self.gn_out / "headless_shell"
        self.binary.write_bytes(b"observer executable fixture")
        self.binary.chmod(0o755)
        (self.gn_out / "gen/resource.pak").write_bytes(b"resource fixture")
        self.license_root = self.root / "license"
        self.license_root.mkdir()
        self.license_file = self.license_root / "LICENSE.headless_shell"
        self.license_file.write_bytes(b"license fixture")
        self.runtime_deps = self.root / "runtime_deps"
        self.runtime_deps.write_text(
            "headless_shell\ngen/resource.pak\nheadless_shell\n",
            encoding="utf-8",
        )
        self.archive = self.root / "out/observer.tar.gz"
        self.manifest = self.root / "out/manifest.json"
        self.archive.parent.mkdir()

    def create(self) -> artifact.ArtifactSummary:
        return artifact.create_artifact(
            self.runtime_deps,
            self.gn_out,
            self.license_file,
            self.archive,
            self.manifest,
        )

    def test_runtime_paths_normalize_gn_indentation_and_dot_components(self) -> None:
        self.assertEqual(
            artifact._runtime_paths(
                b"  gen/./resource.pak\n\theadless_shell  \n"
            ),
            ["gen/resource.pak", "headless_shell"],
        )

    def test_relative_file_open_fails_closed_without_no_follow_support(self) -> None:
        with patch.object(artifact, "_NOFOLLOW", 0):
            with self.assertRaisesRegex(
                RuntimeError,
                "secure no-follow file access is unavailable",
            ):
                artifact._open_relative(self.gn_out, "headless_shell")

    def test_cli_reports_failure_type_and_reason_only_to_stderr(self) -> None:
        self.runtime_deps.write_text("../outside\n", encoding="utf-8")
        stdout = io.StringIO()
        stderr = io.StringIO()

        with redirect_stdout(stdout), redirect_stderr(stderr):
            result = artifact.main(
                [
                    "--runtime-deps",
                    str(self.runtime_deps),
                    "--gn-out",
                    str(self.gn_out),
                    "--license-file",
                    str(self.license_file),
                    "--archive",
                    str(self.archive),
                    "--manifest",
                    str(self.manifest),
                ]
            )

        self.assertEqual(result, 1)
        self.assertEqual(stdout.getvalue(), "")
        self.assertIn("ValueError", stderr.getvalue())
        self.assertIn("canonical and relative", stderr.getvalue())
        self.assertNotIn(str(self.root), stderr.getvalue())
        self.assertFalse(self.archive.exists())
        self.assertFalse(self.manifest.exists())

    @unittest.skipUnless(POSIX_FILE_ACCESS, "requires POSIX no-follow dirfd file access")
    def test_archive_preserves_relative_files_modes_hashes_and_manifest(self) -> None:
        result = self.create()

        self.assertEqual(result.file_count, 3)
        manifest_bytes = self.manifest.read_bytes()
        value = json.loads(manifest_bytes)
        self.assertEqual(value["schema"], "krr-html-performance-observer-artifact-v1")
        rows = {row["path"]: row for row in value["files"]}
        self.assertEqual(set(rows), {
            "LICENSE.headless_shell",
            "gen/resource.pak",
            "headless_shell",
        })
        self.assertEqual(rows["headless_shell"]["mode"], "0755")
        self.assertEqual(
            rows["headless_shell"]["sha256"],
            hashlib.sha256(self.binary.read_bytes()).hexdigest(),
        )
        self.assertNotIn(str(self.root), manifest_bytes.decode("utf-8"))

        with tarfile.open(self.archive, "r:gz") as packaged:
            self.assertEqual(
                set(packaged.getnames()),
                set(rows) | {"manifest.json"},
            )
            binary = packaged.getmember("headless_shell")
            binary_contents = packaged.extractfile(binary)
            self.assertIsNotNone(binary_contents)
            assert binary_contents is not None
            self.assertEqual(stat.S_IMODE(binary.mode), 0o755)
            self.assertEqual(binary_contents.read(), self.binary.read_bytes())
            manifest_contents = packaged.extractfile("manifest.json")
            self.assertIsNotNone(manifest_contents)
            assert manifest_contents is not None
            self.assertEqual(
                manifest_contents.read(),
                manifest_bytes,
            )

        archive_bytes = self.archive.read_bytes()
        self.assertEqual(archive_bytes[4:8], b"\0" * 4)
        self.assertEqual(archive_bytes[3] & 0x08, 0)
        second_archive = self.root / "out/observer-second.tar.gz"
        second_manifest = self.root / "out/manifest-second.json"
        artifact.create_artifact(
            self.runtime_deps,
            self.gn_out,
            self.license_file,
            second_archive,
            second_manifest,
        )
        self.assertEqual(second_archive.read_bytes(), archive_bytes)
        self.assertEqual(second_manifest.read_bytes(), manifest_bytes)

    def test_unflushed_small_gzip_passes_final_output_verification(self) -> None:
        with self.archive.open("w+b", buffering=8192) as stream:
            created_state = artifact._state(os.fstat(stream.fileno()))
            with gzip.GzipFile(
                filename="",
                mode="wb",
                fileobj=stream,
                mtime=0,
            ) as compressed:
                compressed.write(b"small gzip fixture")
            self.assertEqual(self.archive.stat().st_size, 0)

            verified = artifact._verify_output(
                self.archive,
                stream,
                created_state,
            )

            self.assertEqual(
                verified.sha256,
                hashlib.sha256(self.archive.read_bytes()).hexdigest(),
            )

    def test_absolute_and_parent_paths_are_rejected_without_outputs(self) -> None:
        for unsafe in ("/outside/file", "../outside/file"):
            with self.subTest(path_kind="absolute" if unsafe.startswith("/") else "parent"):
                self.runtime_deps.write_text(unsafe + "\n", encoding="utf-8")
                with self.assertRaises(ValueError):
                    self.create()
                self.assertFalse(self.archive.exists())
                self.assertFalse(self.manifest.exists())

    @unittest.skipUnless(POSIX_FILE_ACCESS, "requires POSIX no-follow dirfd file access")
    def test_symlink_runtime_file_is_rejected(self) -> None:
        (self.gn_out / "link").symlink_to(self.binary)
        self.runtime_deps.write_text("link\n", encoding="utf-8")

        with self.assertRaises(OSError):
            self.create()

        self.assertFalse(self.archive.exists())
        self.assertFalse(self.manifest.exists())

    @unittest.skipUnless(POSIX_FILE_ACCESS, "requires POSIX no-follow dirfd file access")
    def test_changed_input_is_rejected_and_owned_outputs_are_removed(self) -> None:
        original_add = artifact._add_file

        def change_before_archive(
            archive: tarfile.TarFile,
            record: artifact.FileRecord,
        ) -> None:
            if record.archive_path == "headless_shell":
                self.binary.write_bytes(b"changed after inventory")
            original_add(archive, record)

        with patch.object(artifact, "_add_file", side_effect=change_before_archive):
            with self.assertRaises(ValueError):
                self.create()

        self.assertFalse(self.archive.exists())
        self.assertFalse(self.manifest.exists())

    @unittest.skipUnless(POSIX_FILE_ACCESS, "requires POSIX no-follow dirfd file access")
    def test_late_runtime_list_change_is_rejected_before_completion(self) -> None:
        original_add = artifact._add_file

        def change_after_last_member(
            archive: tarfile.TarFile,
            record: artifact.FileRecord,
        ) -> None:
            original_add(archive, record)
            if record.archive_path == "headless_shell":
                self.runtime_deps.write_text("headless_shell\n", encoding="utf-8")

        with patch.object(artifact, "_add_file", side_effect=change_after_last_member):
            with self.assertRaises(ValueError):
                self.create()

        self.assertFalse(self.archive.exists())
        self.assertFalse(self.manifest.exists())

    @unittest.skipUnless(POSIX_FILE_ACCESS, "requires POSIX no-follow dirfd file access")
    def test_late_archived_file_change_is_rejected_before_completion(self) -> None:
        original_add = artifact._add_file

        def change_after_license(
            archive: tarfile.TarFile,
            record: artifact.FileRecord,
        ) -> None:
            original_add(archive, record)
            if record.archive_path == "LICENSE.headless_shell":
                self.license_file.write_bytes(b"changed after archive")

        with patch.object(artifact, "_add_file", side_effect=change_after_license):
            with self.assertRaises(ValueError):
                self.create()

        self.assertFalse(self.archive.exists())
        self.assertFalse(self.manifest.exists())

    @unittest.skipUnless(POSIX_FILE_ACCESS, "requires POSIX no-follow dirfd file access")
    def test_final_verification_failure_keeps_replacement_inode(self) -> None:
        replacement = self.root / "replacement.bin"
        replacement.write_bytes(b"other process output")

        def replace_before_failure(path: Path, *_arguments: object) -> None:
            if path == self.archive:
                os.replace(replacement, path)
            raise ValueError("injected final verification failure")

        with patch.object(artifact, "_verify_output", side_effect=replace_before_failure):
            with self.assertRaises(ValueError):
                self.create()

        self.assertEqual(self.archive.read_bytes(), b"other process output")
        self.assertFalse(self.manifest.exists())

    @unittest.skipUnless(POSIX_FILE_ACCESS, "requires POSIX no-follow dirfd file access")
    def test_existing_archive_is_not_replaced(self) -> None:
        self.archive.write_bytes(b"existing artifact")

        with self.assertRaises(FileExistsError):
            self.create()

        self.assertEqual(self.archive.read_bytes(), b"existing artifact")
        self.assertFalse(self.manifest.exists())


if __name__ == "__main__":
    unittest.main()
