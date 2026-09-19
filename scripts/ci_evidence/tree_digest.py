#!/usr/bin/env python3
"""Create a reproducible, fail-closed digest of a GitHub recursive Git tree."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Sequence

try:
    from .schema import TreeSchemaError, canonical_entries
except ImportError:  # Direct script execution has no package context.
    from schema import TreeSchemaError, canonical_entries

SCHEMA = "krr-ci-evidence-input-tree-digest-v1"


def canonical_json_bytes(value: object) -> bytes:
    """Encode JSON once, without whitespace or lossy ASCII escaping."""

    try:
        return json.dumps(
            value,
            ensure_ascii=False,
            allow_nan=False,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("utf-8", "strict")
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise TreeSchemaError("cannot encode canonical UTF-8 JSON") from exc


def canonical_input_tree(git_tree: object) -> dict[str, object]:
    entries = canonical_entries(git_tree)
    return {"entries": [entry.as_json() for entry in entries], "schema": SCHEMA}


def digest_input_tree(git_tree: object) -> dict[str, object]:
    """Return the canonical input tree plus its SHA-256 digest."""

    input_tree = canonical_input_tree(git_tree)
    digest = hashlib.sha256(canonical_json_bytes(input_tree)).hexdigest()
    return {
        "algorithm": "sha256",
        "digest": digest,
        "input_tree": input_tree,
        "schema": SCHEMA,
    }


def _read_json(path: Path) -> object:
    try:
        with path.open("r", encoding="utf-8", newline="") as handle:
            return json.load(handle)
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise TreeSchemaError(f"cannot read Git tree JSON from {path}") from exc


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", required=True, type=Path, help="GitHub recursive-tree JSON response")
    parser.add_argument("--output", type=Path, help="write canonical digest JSON to this regular file")
    args = parser.parse_args(argv)

    try:
        result = digest_input_tree(_read_json(args.input))
        output = canonical_json_bytes(result) + b"\n"
        if args.output is None:
            import sys

            sys.stdout.buffer.write(output)
        else:
            if args.output.exists() and not args.output.is_file():
                raise TreeSchemaError(f"output is not a regular file: {args.output}")
            args.output.write_bytes(output)
    except TreeSchemaError as exc:
        parser.error(str(exc))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
