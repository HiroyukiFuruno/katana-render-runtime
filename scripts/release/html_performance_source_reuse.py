"""測定済みsourceと文書のみ異なる現sourceの対応を検証する。"""
from __future__ import annotations

import hashlib
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path

ALLOWED = frozenset({
    "docs/performance/html-performance-history.json",
    "docs/performance/README.md",
    "docs/quality-gate-improvement-tasks.md",
})
TEST_ROOT = "crates/katana-render-runtime/src/"
TEST_PATHS = frozenset(TEST_ROOT + name for name in (
    "svg_engine_filter_parser_boundary_tests.rs", "svg_engine_parser_boundary_tests.rs",
    "svg_engine_text_layout_spacing_tests.rs",
))
TEST_PARENT = TEST_ROOT + "svg_engine.rs"
TEST_LOGGER = """
#[cfg(test)]
fn init_test_logger() {
    static LOGGER: std::sync::Once = std::sync::Once::new();
    LOGGER.call_once(|| {
        if let Err(error) = env_logger::Builder::new()
            .is_test(true)
            .filter_level(log::LevelFilter::Warn)
            .try_init()
        {
            assert!(log::log_enabled!(log::Level::Warn), "{error}");
        }
    });
}
"""
# WHY: 接続の構文を既確認の特定sourceに固定し、文字列の出現では認定しない。
TEST_PARENT_SHA = frozenset({
    "7531f7db0247100b036f30e7f8215c29a9773805332e919ea8c8a8f3854009b1",
    "c18945e539f8e6176c435ae0104b96d9b964b544d63afcdbff0d96fbb2c4bd39",
})
TEST_ANCHOR_SHA = {
    TEST_ROOT + "lib.rs": "3099318935f89f3ee844f496cf69a2e5595db02233288a3236f60bf6945f7d3f",
    TEST_ROOT + "svg_engine_filter_parser_model_tests.rs": "58239e065fb9db3c7f96daa84da10297e45441e6c5ed30a845458ecd7f4368fa",
    TEST_ROOT + "svg_engine_text_model_tests.rs": "acebe5d6cbac8e9fdda49d5fc9035a11bcc47628c677a4255deccf8c95c279e3",
    TEST_ROOT + "svg_engine_text_layout_matrix_tests.rs": "1904c2b7bfc053712b83988c66432463e2c9581370daea0641925c4e35219be6",
    "crates/katana-render-runtime/vendor/svg-engine/usvg/lib.rs": "15fabe610787a1ba7905b45fe2d9c07f8fa0c2dbf5b8204f719fea3c0c48bed0",
    "crates/katana-render-runtime/vendor/svg-engine/usvg/tree/mod.rs": "f052759059bcd1b1b09a0a94c3046a6c48b1c157d7373cb318f0be9c9177bb50",
    "crates/katana-render-runtime/vendor/svg-engine/usvg/tree/text.rs": "7b48f425e37fd5fa71d336f85ee0be6df0b1222143f08b18c57c10a3f2b5f307",
}
Records = dict[str, tuple[str, int]]
GitBytes = Callable[[Sequence[str]], bytes]
GitText = Callable[[Sequence[str]], str]


@dataclass(frozen=True)
class SourceReuseContext:
    measured_head: str
    eligible_head: str
    measured_records: Mapping[str, tuple[str, int]]
    changed_paths: tuple[str, ...]
    artifact_equivalence: bool = False


def _modes(raw: bytes) -> dict[str, str]:
    result: dict[str, str] = {}
    for entry in raw.split(b"\0"):
        if not entry:
            continue
        metadata, name = entry.split(b"\t", 1)
        mode, kind, _oid = metadata.decode("ascii").split()
        path = name.decode("utf8")
        if kind != "blob" or mode not in ("100644", "100755") or path in result:
            raise ValueError("unsupported source tree entry")
        result[path] = mode
    return result


def validate_reuse_source(root: Path, measured_head: str, current: Records,
                          measured: Records, git_text: GitText,
                          git_bytes: GitBytes, *, allow_test_changes: bool = False) -> SourceReuseContext:
    eligible = git_text(("rev-parse", "HEAD")).strip()
    if git_text(("merge-base", measured_head, eligible)).strip() != measured_head:
        raise ValueError("measured HEAD is not an ancestor")
    if git_text(("status", "--porcelain=v1", "--untracked-files=all")):
        raise ValueError("release source is dirty or untracked")
    old_modes = _modes(git_bytes(("ls-tree", "-r", "-z", measured_head)))
    new_modes = _modes(git_bytes(("ls-tree", "-r", "-z", eligible)))
    if old_modes != new_modes or set(current) != set(measured) or set(current) != set(new_modes):
        raise ValueError("source membership or mode differs")
    changed = tuple(sorted(path for path in current if current[path] != measured[path]))
    test_changes = set(changed) - ALLOWED
    if test_changes:
        if not allow_test_changes or not test_changes <= TEST_PATHS | {TEST_PARENT}:
            raise ValueError("source changes exceed documentation allowance")
        # WHY: 事前にcfg(test)接続を確認したsource以外は候補にしない。
        parent = (root / TEST_PARENT).read_text(encoding="utf8")
        previous = git_text(("show", measured_head + ":" + TEST_PARENT))
        previous_sha = hashlib.sha256(previous.encode()).hexdigest()
        if (previous_sha not in TEST_PARENT_SHA
                or measured.get(TEST_PARENT, (None,))[0] != previous_sha):
            raise ValueError("test-only parent baseline differs")
        if TEST_PARENT in test_changes:
            if parent.count(TEST_LOGGER) != 1 or parent.replace(TEST_LOGGER, "", 1) != previous:
                raise ValueError("test-only parent delta differs")
        if any(measured.get(path, (None,))[0] != checksum
               or current.get(path, (None,))[0] != checksum
               for path, checksum in TEST_ANCHOR_SHA.items()):
            raise ValueError("test-only connection baseline differs")
    if any(old_modes.get(path) != "100644" for path in changed):
        raise ValueError("changed source is not a regular non-executable file")
    for name, (checksum, size) in current.items():
        path = root / name
        if (not path.is_file() or path.is_symlink() or path.resolve() != path
                or path.stat().st_size != size
                or hashlib.sha256(path.read_bytes()).hexdigest() != checksum):
            raise ValueError("working source differs from current committed tree: " + name)
    return SourceReuseContext(measured_head, eligible, dict(measured), changed, bool(test_changes))
