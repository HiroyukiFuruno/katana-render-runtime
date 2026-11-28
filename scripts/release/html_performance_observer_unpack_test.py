"""Isolated observer archive validation and extraction tests."""
from __future__ import annotations

import hashlib
import gzip
import io
import json
import os
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch
from typing import IO

import html_performance_observer_artifact as artifact
import html_performance_observer_unpack as unpacker


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


@unittest.skipUnless(hasattr(os, "O_NOFOLLOW") and hasattr(os, "O_DIRECTORY")
                     and hasattr(os, "fchmod") and os.open in os.supports_dir_fd
                     and os.mkdir in os.supports_dir_fd, "secure relative access unavailable")
class ObserverUnpackTest(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.output = self.root / "output"
        self.archive = self.root / "observer.tar.gz"
        self.binary = b"executable fixture"
        self.files = {"headless_shell": (self.binary, 0o755),
                      "LICENSE.headless_shell": (b"license", 0o444),
                      "resources/data.pak": (b"resources", 0o644)}

    def build(self, *, extra: str | None = None, bad_digest: bool = False,
              symlink: bool = False, metadata: str | None = None) -> tuple[str, str, str]:
        rows = [{"path": name, "sha256": ("0" * 64 if bad_digest and name == "resources/data.pak"
                                              else digest(data)),
                 "size": len(data), "mode": f"{mode:04o}"}
                for name, (data, mode) in sorted(self.files.items())]
        manifest = (json.dumps({"schema": artifact._SCHEMA, "files": rows}, sort_keys=True,
                               separators=(",", ":")) + "\n").encode()
        with tarfile.open(self.archive, "w:gz") as archive:
            for name, (data, mode) in self.files.items():
                info = tarfile.TarInfo(name)
                info.mode, info.size = mode, len(data)
                if name == "headless_shell":
                    if metadata == "uname":
                        info.uname = "private-user"
                    elif metadata == "linkname":
                        info.linkname = "private-link"
                archive.addfile(info, io.BytesIO(data))
            if extra is not None:
                info = tarfile.TarInfo(extra)
                if symlink:
                    info.type = tarfile.SYMTYPE
                    info.linkname = "headless_shell"
                archive.addfile(info, io.BytesIO(b""))
            info = tarfile.TarInfo("manifest.json")
            info.mode, info.size = 0o444, len(manifest)
            archive.addfile(info, io.BytesIO(manifest))
        if metadata in ("devmajor", "devminor"):
            raw = bytearray(gzip.decompress(self.archive.read_bytes()))
            header = bytearray(raw[:tarfile.BLOCKSIZE])
            offset = 329 if metadata == "devmajor" else 337
            header[offset:offset + 8] = b"0000001\0"
            header[148:156] = b" " * 8
            checksum = sum(header)
            header[148:156] = f"{checksum:06o}\0 ".encode("ascii")
            raw[:tarfile.BLOCKSIZE] = header
            self.archive.write_bytes(gzip.compress(bytes(raw)))
            with tarfile.open(self.archive, "r:gz") as archive:
                actual = archive.getmember("headless_shell")
                self.assertTrue(actual.isfile())
                self.assertEqual(getattr(actual, metadata), 1)
        return digest(self.archive.read_bytes()), digest(manifest), digest(self.binary)

    def test_extracts_only_verified_files_with_modes(self) -> None:
        pins = self.build()
        self.assertEqual(unpacker.unpack(self.archive, *pins, self.output), len(self.files))
        for name, (value, mode) in self.files.items():
            path = self.output / name
            self.assertEqual(path.read_bytes(), value)
            self.assertEqual(path.stat().st_mode & 0o7777, mode)

    def test_invalid_inputs_are_rejected_without_removing_new_output(self) -> None:
        cases: tuple[dict[str, object], ...] = ({"extra": "../escape"}, {"extra": "/absolute"},
                 {"extra": "headless_shell"}, {"extra": "extra"},
                 {"extra": "link", "symlink": True}, {"bad_digest": True},
                 {"metadata": "uname"}, {"metadata": "linkname"},
                 {"metadata": "devmajor"}, {"metadata": "devminor"})
        for index, case in enumerate(cases):
            with self.subTest(case=case):
                self.output = self.root / f"output-{index}"
                extra = case.get("extra")
                assert extra is None or isinstance(extra, str)
                metadata = case.get("metadata")
                assert metadata is None or isinstance(metadata, str)
                pins = self.build(extra=extra, bad_digest=case.get("bad_digest") is True,
                                  symlink=case.get("symlink") is True, metadata=metadata)
                with self.assertRaises((ValueError, OSError)):
                    unpacker.unpack(self.archive, *pins, self.output)
                if case.get("bad_digest") is True:
                    self.assertTrue(self.output.is_dir())
                else:
                    self.assertFalse(self.output.exists())

    def test_mutation_after_a_member_is_written_is_rejected(self) -> None:
        pins = self.build()
        original = unpacker._write_member
        def mutate(root_fd: int, name: str, source: IO[bytes], mode: int) -> tuple[str, int]:
            result = original(root_fd, name, source, mode)
            if name == "resources/data.pak":
                (self.output / "headless_shell").write_bytes(b"changed")
            return result
        with patch.object(unpacker, "_write_member", side_effect=mutate):
            with self.assertRaisesRegex(ValueError, "extracted member changed"):
                unpacker.unpack(self.archive, *pins, self.output)
        self.assertTrue(self.output.is_dir())

    def test_unknown_file_and_directory_replacement_are_rejected(self) -> None:
        original = unpacker._write_member
        for operation in ("unknown", "replace"):
            with self.subTest(operation=operation):
                self.output = self.root / ("output-" + operation)
                pins = self.build()
                def mutate(root_fd: int, name: str, source: IO[bytes], mode: int,
                           *, selected: str = operation) -> tuple[str, int]:
                    result = original(root_fd, name, source, mode)
                    if name == "resources/data.pak":
                        if selected == "unknown":
                            (self.output / "unlisted").write_bytes(b"foreign")
                        else:
                            self.output.rename(self.root / "moved-output")
                            self.output.mkdir()
                            (self.output / "keep").write_bytes(b"foreign")
                    return result
                with patch.object(unpacker, "_write_member", side_effect=mutate):
                    with self.assertRaises((ValueError, OSError)):
                        unpacker.unpack(self.archive, *pins, self.output)
                if operation == "replace":
                    self.assertEqual((self.output / "keep").read_bytes(), b"foreign")
                else:
                    self.assertEqual((self.output / "unlisted").read_bytes(), b"foreign")

    def test_each_pin_and_existing_output_are_enforced(self) -> None:
        pins = self.build()
        for index in range(3):
            altered = list(pins)
            altered[index] = "0" * 64
            with self.subTest(index=index), self.assertRaises(ValueError):
                unpacker.unpack(self.archive, altered[0], altered[1], altered[2], self.output)
            self.assertFalse(self.output.exists())
        self.output.mkdir()
        keep = self.output / "keep"
        keep.write_bytes(b"original")
        with self.assertRaises(FileExistsError):
            unpacker.unpack(self.archive, *pins, self.output)
        self.assertEqual(keep.read_bytes(), b"original")


if __name__ == "__main__":
    unittest.main()
