"""Tests for the owned Python test inventory classifier."""
from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from types import ModuleType
from typing import ClassVar


SCRIPT = Path(__file__).with_name("owned-python-test-inventory.py")


def load_classifier() -> ModuleType:
    spec = importlib.util.spec_from_file_location("owned_python_inventory", SCRIPT)
    if spec is None or spec.loader is None:
        raise AssertionError("classifier module could not be loaded")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class OwnedPythonInventoryTest(unittest.TestCase):
    classifier: ClassVar[ModuleType]

    @classmethod
    def setUpClass(cls) -> None:
        cls.classifier = load_classifier()

    def classify(self, source: str) -> str:
        function = self.classifier.classify_python_source
        if not callable(function):
            raise AssertionError("classifier entry must be callable")
        result = function(source)
        if not isinstance(result, str):
            raise AssertionError("classifier result must be a string")
        return result

    def test_recognizes_fixed_import_forms_and_aliases(self) -> None:
        self.assertEqual(self.classify("import unittest as ut\nclass T(ut.TestCase): pass"), "unittest")
        self.assertEqual(
            self.classify("from unittest import TestCase as Case\nclass T(Case): pass"),
            "unittest",
        )

    def test_recognizes_local_unittest_subclass_chain(self) -> None:
        self.assertEqual(self.classify("import unittest\nclass BaseTest(unittest.TestCase): pass\nclass ChildTest(BaseTest): pass"), "unittest")
        self.assertEqual(self.classify("from unittest import TestCase as Case\nclass BaseTest(Case): pass\nclass MiddleTest(BaseTest): pass\nclass ChildTest(MiddleTest): pass"), "unittest")

    def test_local_unittest_class_rebinding_remains_unknown(self) -> None:
        self.assertEqual(self.classify("import unittest\nclass BaseTest(unittest.TestCase): pass\nBaseTest = object\nclass ChildTest(BaseTest): pass"), "unknown")
        self.assertEqual(self.classify("import unittest\nclass BaseTest(unittest.TestCase): pass\nclass ChildTest(BaseTest): pass\nBaseTest = object"), "unknown")
        self.assertEqual(self.classify("import unittest\nclass ChildTest(UnknownTest): pass"), "unknown")
        self.assertEqual(self.classify("import unittest\nclass BaseTest(unittest.TestCase): pass\nclass ChildTest(BaseTest, UnknownBase): pass"), "unknown")
        self.assertEqual(self.classify("import unittest\nclass BaseTest(unittest.TestCase): pass\nBaseTest.__bases__ = (object,)\nclass ChildTest(BaseTest): pass"), "unknown")

    def test_does_not_use_filename_or_text_as_evidence(self) -> None:
        self.assertEqual(self.classify('value = "class Fake(unittest.TestCase): pass"'), "none")
        self.assertEqual(self.classify("# class Fake(unittest.TestCase): pass\nvalue = 1"), "none")
        self.assertEqual(self.classify("def TestCase(): pass\nclass T: pass"), "none")

    def test_unknown_covers_shadowing_and_unsupported_bases(self) -> None:
        self.assertEqual(self.classify("import unittest as ut\nut = object()\nclass T(ut.TestCase): pass"), "unknown")
        self.assertEqual(self.classify("import unittest as ut\nimport fake as ut\nclass T(ut.TestCase): pass"), "unknown")
        self.assertEqual(self.classify("import unittest as ut\nut.TestCase = object\nclass T(ut.TestCase): pass"), "unknown")
        self.assertEqual(self.classify("from unittest import TestCase\nTestCase = object\nclass T(TestCase): pass"), "unknown")
        self.assertEqual(self.classify("class T(CustomTestCase): pass"), "unknown")
        self.assertEqual(self.classify("class T(unittest.TestCase): pass"), "unknown")

    def test_command_validates_paths_and_reports_records(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "unit_test.py").write_text(
                "from unittest import TestCase as C\nclass T(C): pass\n", encoding="utf-8"
            )
            (root / "plain.py").write_text("value = 1\n", encoding="utf-8")
            completed = subprocess.run(
                [sys.executable, str(SCRIPT), "--root", str(root)],
                input=json.dumps(["unit_test.py", "plain.py"]),
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertEqual(
                json.loads(completed.stdout),
                [
                    {"path": "unit_test.py", "kind": "unittest"},
                    {"path": "plain.py", "kind": "none"},
                ],
            )

    def test_command_rejects_missing_syntax_and_symlink_inputs(self) -> None:
        with tempfile.TemporaryDirectory() as directory, tempfile.TemporaryDirectory() as outside_directory:
            root = Path(directory)
            (root / "broken.py").write_text("class", encoding="utf-8")
            outside = Path(outside_directory) / "owned-inventory-outside.py"
            outside.write_text("value = 1\n", encoding="utf-8")
            (root / "linked.py").symlink_to(outside)
            (root / "nested").mkdir()
            (root / "nested" / "plain.py").write_text("value = 1\n", encoding="utf-8")
            for payload in (
                ["missing.py"],
                ["broken.py"],
                ["../broken.py"],
                ["linked.py"],
                ["./nested/plain.py"],
                ["nested//plain.py"],
                ["nested/./plain.py"],
                {"path": "broken.py"},
            ):
                completed = subprocess.run(
                    [sys.executable, str(SCRIPT), "--root", str(root)],
                    input=json.dumps(payload),
                    text=True,
                    capture_output=True,
                    check=False,
                )
                self.assertNotEqual(completed.returncode, 0, payload)


if __name__ == "__main__":
    unittest.main()
