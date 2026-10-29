"""固定コピーCLIの実ファイル・排他出力・引数境界を確認する。"""

from __future__ import annotations

from contextlib import redirect_stderr, redirect_stdout
import hashlib
import io
import json
from pathlib import Path
import stat
import tempfile
import unittest

import freeze_html_capture_binary as bridge


class CaptureBinaryFreezeTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.source = self.root / "runtime-tests"
        self.source.write_bytes(b"owned native executable fixture")
        self.destination = self.root / "frozen/runtime-tests"

    def arguments(self) -> list[str]:
        return ["--source", str(self.source), "--destination", str(self.destination)]

    def test_capture_uses_frozen_bytes_after_build_artifact_is_retired(self) -> None:
        output = io.StringIO()
        with redirect_stdout(output):
            self.assertEqual(bridge.main(self.arguments()), 0)
        report = json.loads(output.getvalue())
        self.assertEqual(report, {"source": str(self.source),
                                 "path": str(self.destination),
                                 "sha256": hashlib.sha256(self.source.read_bytes()).hexdigest()})
        self.source.rename(self.root / "retired-build-artifact")
        self.assertEqual(hashlib.sha256(self.destination.read_bytes()).hexdigest(),
                         report["sha256"])
        self.assertEqual(stat.S_IMODE(self.destination.stat().st_mode), 0o555)

    def test_existing_frozen_directory_is_rejected_without_replacement(self) -> None:
        with redirect_stdout(io.StringIO()):
            bridge.main(self.arguments())
        before = self.destination.stat()
        with self.assertRaises(FileExistsError), redirect_stdout(io.StringIO()):
            bridge.main(self.arguments())
        self.assertEqual(self.destination.stat(), before)

    def test_missing_arguments_are_rejected_before_creating_artifacts(self) -> None:
        with self.assertRaises(SystemExit) as raised, redirect_stderr(io.StringIO()):
            bridge.main(["--source", str(self.source)])
        self.assertEqual(raised.exception.code, 2)
        self.assertFalse(self.destination.parent.exists())


if __name__ == "__main__":
    unittest.main()
