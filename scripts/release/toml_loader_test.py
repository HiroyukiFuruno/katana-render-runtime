"""Tests for the shared release TOML parser boundary and import isolation."""
from __future__ import annotations

import importlib
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from types import ModuleType
from unittest.mock import patch

SCRIPT = Path(__file__).with_name("toml_loader.py")
SPEC = importlib.util.spec_from_file_location("release_toml_loader_test_subject", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise ImportError("shared TOML loader cannot be loaded")
LOADER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(LOADER)
REAL_IMPORT = importlib.import_module


class ParserModule(ModuleType):
    TOMLDecodeError = ValueError

    def loads(self, source: str) -> object:
        return {"selected": self.__name__, "source": source}


def parser_module(name: str) -> ModuleType:
    return ParserModule(name)


def no_host_toml(name: str, package: str | None = None) -> ModuleType:
    if name in {"tomllib", "tomli"}:
        raise ModuleNotFoundError(name=name)
    return REAL_IMPORT(name, package)


class TomlLoaderTest(unittest.TestCase):
    def test_stdlib_is_selected_before_external(self) -> None:
        module = parser_module("tomllib")
        with patch.object(LOADER.importlib, "import_module", return_value=module) as resolver:
            parser = LOADER.load_toml_parser()
        self.assertEqual(resolver.call_args_list, [unittest.mock.call("tomllib")])
        self.assertIs(parser.TOMLDecodeError, module.TOMLDecodeError)
        self.assertEqual(parser.loads("key = 1"), {"selected": "tomllib", "source": "key = 1"})

    def test_external_is_selected_only_after_missing_stdlib(self) -> None:
        module = parser_module("tomli")
        seen: list[str] = []

        def resolve(name: str, package: str | None = None) -> ModuleType:
            seen.append(name)
            if name == "tomllib":
                raise ModuleNotFoundError(name=name)
            return module

        with patch.object(LOADER.importlib, "import_module", resolve):
            parser = LOADER.load_toml_parser()
        self.assertEqual(seen, ["tomllib", "tomli"])
        self.assertIs(parser.TOMLDecodeError, module.TOMLDecodeError)
        self.assertEqual(parser.loads("value = 2"), {"selected": "tomli", "source": "value = 2"})

    def test_vendored_identity_parse_errors_and_path_restoration(self) -> None:
        before = sys.path.copy()
        with patch.object(LOADER.importlib, "import_module", no_host_toml):
            parser = LOADER.load_toml_parser()
        vendor = REAL_IMPORT("_tomli._parser")
        self.assertIs(parser.TOMLDecodeError, vendor.TOMLDecodeError)
        self.assertEqual(parser.loads("key = 'single quoted'\n"), {"key": "single quoted"})
        with self.assertRaises(parser.TOMLDecodeError):
            parser.loads("[")
        self.assertEqual(sys.path, before)

    def test_factory_does_not_latch_a_previous_selected_parser(self) -> None:
        with patch.object(LOADER.importlib, "import_module", return_value=parser_module("tomllib")):
            normal = LOADER.load_toml_parser()
        with patch.object(LOADER.importlib, "import_module", no_host_toml):
            fallback = LOADER.load_toml_parser()
        self.assertEqual(normal.loads("key = 1"), {"selected": "tomllib", "source": "key = 1"})
        self.assertEqual(fallback.loads("key = 1"), {"key": 1})
        self.assertIsNot(normal.TOMLDecodeError, fallback.TOMLDecodeError)

    def test_cached_external_module_still_uses_the_current_resolver(self) -> None:
        module = parser_module("tomli")
        with patch.dict(sys.modules, {"tomli": module}):
            with patch.object(LOADER.importlib, "import_module", no_host_toml):
                parser = LOADER.load_toml_parser()
        self.assertEqual(parser.loads("cached = true"), {"cached": True})

    def test_builtins_patch_alone_does_not_force_cached_module_absence(self) -> None:
        import builtins

        original = builtins.__import__

        def reject(name: str, globals: dict[str, object] | None = None, locals: dict[str, object] | None = None, fromlist: tuple[str, ...] = (), level: int = 0) -> ModuleType:
            if name in {"tomllib", "tomli"}:
                raise ModuleNotFoundError(name=name)
            return original(name, globals, locals, fromlist, level)

        with patch.dict(sys.modules, {"tomllib": parser_module("cached-stdlib")}):
            with patch.object(builtins, "__import__", reject):
                parser = LOADER.load_toml_parser()
        self.assertEqual(parser.loads("value = 1"), {"selected": "cached-stdlib", "source": "value = 1"})

    def test_internal_import_failure_is_not_treated_as_parser_absence(self) -> None:
        for name in ("tomllib", "tomli"):
            with self.subTest(name=name):
                def resolve(selected: str, package: str | None = None, missing: str = name) -> ModuleType:
                    if selected == "tomllib" and missing == "tomli":
                        raise ModuleNotFoundError(name=selected)
                    raise ModuleNotFoundError(name="broken_parser_dependency")

                with patch.object(LOADER.importlib, "import_module", resolve):
                    with self.assertRaises(ModuleNotFoundError) as caught:
                        LOADER.load_toml_parser()
                self.assertEqual(caught.exception.name, "broken_parser_dependency")

    def test_invalid_interface_does_not_fall_back(self) -> None:
        for field, value in (("loads", None), ("TOMLDecodeError", object), ("TOMLDecodeError", 3)):
            with self.subTest(field=field, value=value):
                module = parser_module("tomllib")
                setattr(module, field, value)
                with patch.object(LOADER.importlib, "import_module", return_value=module) as resolver:
                    with self.assertRaises(ImportError):
                        LOADER.load_toml_parser()
                self.assertEqual(resolver.call_count, 1)

    def test_foreign_vendored_cache_is_rejected_and_path_restored(self) -> None:
        module = parser_module("_tomli._parser")
        module.__file__ = str(Path(tempfile.gettempdir()) / "foreign-parser.py")
        before = sys.path.copy()
        with patch.dict(sys.modules, {"_tomli._parser": module}):
            with patch.object(LOADER.importlib, "import_module", no_host_toml):
                with self.assertRaisesRegex(ImportError, "outside the owned"):
                    LOADER.load_toml_parser()
        self.assertEqual(sys.path, before)

    def test_failed_vendored_import_restores_path(self) -> None:
        before = sys.path.copy()

        def missing(name: str, package: str | None = None) -> ModuleType:
            raise ModuleNotFoundError(name=name)

        with patch.object(LOADER.importlib, "import_module", missing):
            with self.assertRaises(ModuleNotFoundError):
                LOADER.load_toml_parser()
        self.assertEqual(sys.path, before)

    def test_consumers_reject_cached_foreign_helper_without_invoking_or_deleting_it(self) -> None:
        program = """import importlib.util, json, pathlib, sys, types
release = pathlib.Path(sys.argv[1])
filename, spoof = sys.argv[2:]
foreign = types.ModuleType('toml_loader')
foreign.__file__ = str(release / 'toml_loader.py') if spoof == 'yes' else str(pathlib.Path.cwd() / 'foreign-toml-loader.py')
calls = []
def foreign_factory():
    calls.append('called')
    raise RuntimeError('foreign helper factory invoked')
foreign.load_toml_parser = foreign_factory
sys.modules['toml_loader'] = foreign
before = sys.path.copy()
name = 'foreign_helper_' + filename.removesuffix('.py')
spec = importlib.util.spec_from_file_location(name, release / filename)
if spec is None or spec.loader is None:
    raise RuntimeError('consumer loader missing')
module = importlib.util.module_from_spec(spec)
sys.modules[name] = module
try:
    spec.loader.exec_module(module)
except ImportError:
    pass
else:
    raise RuntimeError('foreign helper was not rejected')
if calls or sys.modules['toml_loader'] is not foreign or sys.path != before:
    raise RuntimeError('factory ran or cached helper/path was changed')
print(json.dumps({'rejected': True, 'factoryCalled': False, 'cachePreserved': True, 'pathStable': True}))
"""
        for filename in ("update_usvg_skrifa_pair.py", "verify_dependency_freshness.py"):
            for spoof in ("no", "yes"):
                with self.subTest(filename=filename, spoof=spoof):
                    with tempfile.TemporaryDirectory() as directory:
                        result = subprocess.run([sys.executable, "-c", program, str(SCRIPT.resolve().parent), filename, spoof], cwd=directory, env={**os.environ, "PYTHONPATH": ""}, capture_output=True, text=True, check=False)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(json.loads(result.stdout), {"rejected": True, "factoryCalled": False, "cachePreserved": True, "pathStable": True})

    def test_vendored_parent_path_and_spoofed_child_are_refused_before_external_code(self) -> None:
        program = r"""import importlib, importlib.util, json, pathlib, sys, types
from unittest.mock import patch
helper, mode, directory = sys.argv[1:]
root = pathlib.Path(directory)
external = root / 'external'
external.mkdir()
marker = root / 'executed.txt'
(external / '__init__.py').write_text('# foreign parent package')
(external / '_parser.py').write_text('from pathlib import Path\nPath(' + repr(str(marker)) + ').write_text("foreign import executed")\nTOMLDecodeError = ValueError\ndef loads(source): return {}\n')
spec = importlib.util.spec_from_file_location('vendor_parent_cache_subject', helper)
subject = importlib.util.module_from_spec(spec)
spec.loader.exec_module(subject)
if mode == 'child-spoof':
    foreign = types.ModuleType('_tomli._parser')
    foreign.__file__ = str(pathlib.Path(helper).with_name('_tomli') / '_parser.py')
    foreign.TOMLDecodeError = ValueError
    def foreign_loads(source):
        marker.write_text('foreign loads executed')
        return {}
    foreign.loads = foreign_loads
    key = '_tomli._parser'
elif mode == 're-spoof':
    import re
    foreign = types.ModuleType('_tomli._re')
    foreign.__file__ = str(pathlib.Path(helper).with_name('_tomli') / '_re.py')
    foreign.RE_DATETIME = re.compile('(?!)')
    foreign.RE_LOCALTIME = re.compile('(?!)')
    foreign.RE_NUMBER = re.compile('[0-9]+')
    def foreign_match(*args):
        marker.write_text('foreign regex callback executed')
        return 1
    foreign.match_to_datetime = foreign_match
    foreign.match_to_localtime = foreign_match
    foreign.match_to_number = foreign_match
    key = '_tomli._re'
else:
    foreign = types.ModuleType('_tomli')
    foreign.__file__ = str(pathlib.Path(helper).with_name('_tomli') / '__init__.py') if mode == 'parent-spoof' else str(external / '__init__.py')
    foreign.__path__ = [str(external)]
    key = '_tomli'
sys.modules[key] = foreign
original = importlib.import_module
def no_host(name, package=None):
    if name in {'tomllib', 'tomli'}:
        raise ModuleNotFoundError(name=name)
    return original(name, package)
before = sys.path.copy()
try:
    with patch.object(importlib, 'import_module', no_host):
        parser = subject.load_toml_parser()
        parser.loads('key = 1')
except ImportError:
    pass
else:
    raise RuntimeError('foreign vendored parser was admitted')
if marker.exists() or sys.modules[key] is not foreign or sys.path != before:
    raise RuntimeError('foreign code executed or cache/path changed')
print(json.dumps({'rejected': True, 'externalCodeCalled': False, 'cachePreserved': True, 'pathStable': True}))
"""
        for mode in ("parent-foreign", "parent-spoof", "child-spoof", "re-spoof"):
            with self.subTest(mode=mode):
                with tempfile.TemporaryDirectory() as directory:
                    result = subprocess.run([sys.executable, "-c", program, str(SCRIPT.resolve()), mode, directory], capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(json.loads(result.stdout), {"rejected": True, "externalCodeCalled": False, "cachePreserved": True, "pathStable": True})

    def test_both_consumers_file_load_from_other_cwd_and_preserve_paths(self) -> None:
        release = str(SCRIPT.resolve().parent)
        program = """import contextlib, importlib, importlib.util, json, pathlib, sys
from unittest.mock import patch
release = pathlib.Path(sys.argv[1])
before = sys.path.copy()
original = importlib.import_module
def no_host(name, package=None):
    if name in {'tomllib', 'tomli'}:
        raise ModuleNotFoundError(name=name)
    return original(name, package)
for phase in ('normal', 'fallback'):
    context = contextlib.nullcontext() if phase == 'normal' else patch.object(importlib, 'import_module', no_host)
    decoders = []
    with context:
        for filename in ('update_usvg_skrifa_pair.py', 'verify_dependency_freshness.py'):
            name = phase + '_' + filename.removesuffix('.py')
            spec = importlib.util.spec_from_file_location(name, release / filename)
            if spec is None or spec.loader is None:
                raise RuntimeError('consumer loader missing')
            module = importlib.util.module_from_spec(spec)
            sys.modules[name] = module
            spec.loader.exec_module(module)
            if module.tomllib.loads('value = 7') != {'value': 7}:
                raise RuntimeError('wrong parser result')
            decoders.append(module.tomllib.TOMLDecodeError)
    if decoders[0] is not decoders[1]:
        raise RuntimeError('consumer decode-error identity differs')
    if phase == 'fallback' and not decoders[0].__module__.startswith('_tomli'):
        raise RuntimeError('fallback did not select the vendored parser')
if before != sys.path:
    raise RuntimeError('consumer path mutation leaked')
print(json.dumps({'loaded': 4, 'pathStable': True, 'decodeIdentity': True}))
"""
        with tempfile.TemporaryDirectory() as directory:
            result = subprocess.run([sys.executable, "-c", program, release], cwd=directory, env={**os.environ, "PYTHONPATH": ""}, capture_output=True, text=True, check=True)
        self.assertEqual(json.loads(result.stdout), {"loaded": 4, "pathStable": True, "decodeIdentity": True})


if __name__ == "__main__":
    unittest.main()
