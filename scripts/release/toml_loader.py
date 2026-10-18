"""Reuse the available TOML parser through the release checks' typed boundary."""
from __future__ import annotations

import importlib
import sys
from collections.abc import Callable
from pathlib import Path
from types import FunctionType, ModuleType
from typing import Protocol


class TomlParser(Protocol):
    @property
    def TOMLDecodeError(self) -> type[ValueError]: ...

    def loads(self, source: str) -> object: ...


class _TomlParserAdapter:
    def __init__(self, module: ModuleType) -> None:
        loads = getattr(module, "loads", None)
        decode_error = getattr(module, "TOMLDecodeError", None)
        if not callable(loads) or not isinstance(decode_error, type):
            raise ImportError("selected TOML parser has an incomplete interface")
        if not issubclass(decode_error, ValueError):
            raise ImportError("selected TOML parser has an invalid decode error type")
        self._loads: Callable[[str], object] = loads
        self._decode_error: type[ValueError] = decode_error

    @property
    def TOMLDecodeError(self) -> type[ValueError]:
        return self._decode_error

    def loads(self, source: str) -> object:
        return self._loads(source)


def _verify_vendored_origin(module: ModuleType, root: Path, name: str) -> None:
    expected = root / ("__init__.py" if name == "_tomli" else f"{name.rsplit('.', 1)[-1]}.py")
    filename = vars(module).get("__file__")
    if not isinstance(filename, str) or Path(filename).resolve() != expected:
        raise ImportError("vendored TOML parser is outside the owned release directory")
    if name == "_tomli":
        paths = vars(module).get("__path__")
        if not isinstance(paths, list) or len(paths) != 1 or not isinstance(paths[0], str) or Path(paths[0]).resolve() != root:
            raise ImportError("vendored TOML package has a foreign import path")
    callbacks = {
        "_tomli._parser": ("loads",),
        "_tomli._re": ("match_to_datetime", "match_to_localtime", "match_to_number"),
    }.get(name, ())
    for callback in callbacks:
        function = vars(module).get(callback)
        if not isinstance(function, FunctionType) or Path(function.__code__.co_filename).resolve() != expected:
            raise ImportError("vendored TOML function has a foreign origin")


def load_toml_parser() -> TomlParser:
    for name in ("tomllib", "tomli"):
        try:
            module = importlib.import_module(name)
        except ModuleNotFoundError as error:
            if error.name != name:
                raise
        else:
            return _TomlParserAdapter(module)
    # vendored parser の探索範囲はこの owned sibling directory に固定する。
    parser_root = str(Path(__file__).resolve().parent)
    vendor_root = Path(parser_root) / "_tomli"
    for name in ("_tomli", "_tomli._parser", "_tomli._re", "_tomli._types"):
        cached = sys.modules.get(name)
        if cached is not None:
            _verify_vendored_origin(cached, vendor_root, name)
    sys.path.insert(0, parser_root)
    try:
        module = importlib.import_module("_tomli._parser")
    finally:
        sys.path.remove(parser_root)
    _verify_vendored_origin(module, vendor_root, "_tomli._parser")
    return _TomlParserAdapter(module)
