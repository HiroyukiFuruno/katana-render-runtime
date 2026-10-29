"""Report the installed scientific runtime bytes used by the HTML gate."""

from __future__ import annotations

import argparse
import hashlib
from importlib import metadata
import json
from pathlib import Path


def _distribution_identity(name: str, expected_version: str) -> dict[str, str]:
    distribution = metadata.distribution(name)
    if distribution.version != expected_version:
        raise RuntimeError(f"{name} version differs: {distribution.version!r}")
    files = distribution.files
    if files is None:
        raise RuntimeError(f"{name} has no installed file inventory")
    entries: list[tuple[str, str]] = []
    for relative in files:
        relative_path = str(relative)
        path = Path(str(distribution.locate_file(relative_path)))
        if not path.is_file() or path.is_symlink():
            raise RuntimeError(f"{name} installed file is unavailable: {relative_path}")
        entries.append((relative_path, hashlib.sha256(path.read_bytes()).hexdigest()))
    payload = json.dumps(sorted(entries), separators=(",", ":"), ensure_ascii=True).encode()
    return {"name": name, "version": distribution.version,
            "sha256": hashlib.sha256(payload).hexdigest()}


def installed_identity(scipy_version: str, numpy_version: str) -> str:
    identity = {
        "numpy": _distribution_identity("numpy", numpy_version),
        "reflink-copy": _distribution_identity("reflink-copy", "0.3.3"),
        "scipy": _distribution_identity("scipy", scipy_version),
    }
    payload = json.dumps(identity, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode()
    return hashlib.sha256(payload).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--scipy-version", required=True)
    parser.add_argument("--numpy-version", required=True)
    args = parser.parse_args()
    print(installed_identity(args.scipy_version, args.numpy_version))


if __name__ == "__main__":
    main()
