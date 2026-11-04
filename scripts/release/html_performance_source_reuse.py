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

RELEASE_ONLY_PROGRAMS = frozenset({
    "scripts/release/verify-release-target.py",
    "scripts/release/verify_release_target_test.py",
})


def equivalent_program_records(left: Mapping[str, tuple[str, int]],
                               right: Mapping[str, tuple[str, int]]) -> bool:
    """実行しないrelease gateの内容だけを、同一集合の比較から除く。"""
    return left.keys() == right.keys() and all(
        left[path] == right[path] for path in left if path not in RELEASE_ONLY_PROGRAMS
    )


def cache_only_collector_change(old: bytes, current: bytes) -> bool:
    """認証queryの配線だけを移した既知の変更を、byte単位で照合する。"""
    lifecycle = (b'    lifecycle = output.parent / f"{output.name}.lifecycle"\n'
                 b'    gate.require(not lifecycle.exists(), "fresh measurement lifecycle output required")\n'
                 b'    git_query = _make_git_query(lifecycle)\n')
    old_call = b'resume_batch.resolve(), root, fixed_epoch=fixed_epoch,'
    new_call = b'resume_batch.resolve(), root, fixed_epoch=fixed_epoch, git_query=git_query,'
    anchor = b'    if resume_batch is not None:\n'
    marker = b'def produce('
    if old.count(marker) != 1:
        return False
    prefix, body = old.split(marker, 1)
    if body.count(lifecycle) != 1 or body.count(old_call) != 1 or body.count(anchor) != 1:
        return False
    transformed = body.replace(lifecycle, b'', 1).replace(old_call, new_call, 1)
    transformed = transformed.replace(anchor, lifecycle + anchor, 1)
    return prefix + marker + transformed == current


def known_auth_program_change(path: str, old: bytes, current: bytes) -> bool:
    """この移行でレビューする認証・輸送コードの実体だけを許可する。"""
    import ast
    import inspect

    if path == "scripts/release/html_performance_source_reuse.py":
        old_tree = ast.parse(old)
        current_tree = ast.parse(current)
        additions = {"cache_only_collector_change", "known_auth_program_change"}
        definitions = [node for node in current_tree.body
                       if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and node.name in additions]
        if len(definitions) != len(additions) or {node.name for node in definitions} != additions:
            return False
        trusted: dict[str, Callable[..., bool]] = {
            "cache_only_collector_change": cache_only_collector_change,
            "known_auth_program_change": known_auth_program_change,
        }
        for node in definitions:
            actual = ast.dump(node, include_attributes=False)
            expected_nodes = ast.parse(inspect.getsource(trusted[node.name])).body
            if len(expected_nodes) != 1 or actual != ast.dump(expected_nodes[0], include_attributes=False):
                return False
        current_tree.body = [node for node in current_tree.body
                             if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
                             or node.name not in additions]
        return ast.dump(old_tree, include_attributes=False) == ast.dump(current_tree, include_attributes=False)
    pins = {
        "scripts/release/html_performance_epoch_reuse.py": (
            "33a05103fc5100c7eb2e9d10368785c8ab20e6f8308ddc53f0ae95f63c573f1c",
            "3f50599d284dbe593bb9a1cf02d0fdd633647eb61a3f6be7867b8308d72de3c7"),
        "scripts/release/produce_html_performance_evidence.py": (
            "d4d85ae7e2120e3720ef31afe6a7228011385d0267ec770060e68f606e21388e",
            "490233a4797d9690f7393000cfeda1ceb4d806e1a7df2c588bbb300d8cc1dae5"),
        "scripts/release/produce_html_performance_evidence_test.py": (
            "1ed2ab04cc2fccc6674b0eb565e097d9df37d44ca66d5b6fc70e12d7583e2fe2",
            "52f4b3cb14c379b5fb3c260d170b10024d30377bf63ad33b4084a8fc3137db9b"),
        "scripts/release/verify_html_performance_evidence.py": (
            "d9d93a641fd5e5dc4db5a0135cffa2073c84f34da132cedfb9821dace49a59aa",
            "71fa1bc5e34a3d9432d2923d14e0967f7ae2d6e5f879941eb3f858668b1da86e"),
        "scripts/release/verify_html_performance_evidence_test.py": (
            "9f423f9edbd3fa5ef2a0f1fd1d28a2d2a61f968498650f65dd3f0e5f323be4b3",
            "649ff5531031391cdf7ea802ca0fa014c52dd8ad687cd249b335287878c97e00"),
    }
    expected = pins.get(path)
    return expected is not None and expected == (
        hashlib.sha256(old).hexdigest(), hashlib.sha256(current).hexdigest())


@dataclass(frozen=True)
class SourceReuseContext:
    measured_head: str
    eligible_head: str
    measured_records: Mapping[str, tuple[str, int]]
    changed_paths: tuple[str, ...]
    artifact_equivalence: bool = False
    program_equivalence: bool = False


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
                          git_bytes: GitBytes, *, allow_test_changes: bool = False,
                          program_records: Mapping[str, tuple[str, int]] | None = None,
                          program_modes: Mapping[str, str] | None = None) -> SourceReuseContext:
    eligible = git_text(("rev-parse", "HEAD")).strip()
    if git_text(("merge-base", measured_head, eligible)).strip() != measured_head:
        raise ValueError("measured HEAD is not an ancestor")
    if git_text(("status", "--porcelain=v1", "--untracked-files=all")):
        raise ValueError("release source is dirty or untracked")
    old_modes = _modes(git_bytes(("ls-tree", "-r", "-z", measured_head)))
    new_modes = _modes(git_bytes(("ls-tree", "-r", "-z", eligible)))
    if (program_records is None) != (program_modes is None):
        raise ValueError("program source proof is incomplete")
    def is_program(path: str) -> bool:
        return path.startswith("scripts/") or path == "Justfile"
    if program_records is not None and program_modes is not None:
        if (not equivalent_program_records(
                {path: value for path, value in current.items() if is_program(path)}, program_records)
                or {path: value for path, value in new_modes.items() if is_program(path)} != program_modes):
            raise ValueError("current measurement program differs from authenticated program")
    old_checked = {path: mode for path, mode in old_modes.items()
                   if program_records is None or not is_program(path)}
    new_checked = {path: mode for path, mode in new_modes.items()
                   if program_records is None or not is_program(path)}
    measured_checked = {path for path in measured if program_records is None or not is_program(path)}
    current_checked = {path for path in current if program_records is None or not is_program(path)}
    if (old_checked != new_checked or current_checked != measured_checked
            or set(current) != set(new_modes) or set(measured) != set(old_modes)):
        raise ValueError("source membership or mode differs")
    changed = tuple(sorted(path for path in set(current) | set(measured)
                           if current.get(path) != measured.get(path)
                           or old_modes.get(path) != new_modes.get(path)))
    program_changes = {path for path in changed if program_records is not None and is_program(path)}
    test_changes = set(changed) - ALLOWED - program_changes
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
    if any(old_modes.get(path) != "100644" for path in changed if path not in program_changes):
        raise ValueError("changed source is not a regular non-executable file")
    for name, (checksum, size) in current.items():
        path = root / name
        if (not path.is_file() or path.is_symlink() or path.resolve() != path
                or path.stat().st_size != size
                or hashlib.sha256(path.read_bytes()).hexdigest() != checksum):
            raise ValueError("working source differs from current committed tree: " + name)
    return SourceReuseContext(measured_head, eligible, dict(measured), changed,
                              bool(test_changes), bool(program_changes))
