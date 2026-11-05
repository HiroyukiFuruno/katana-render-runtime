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
        old_sha = hashlib.sha256(old).hexdigest()
        current_sha = hashlib.sha256(current).hexdigest()
        if old_sha != "847b5541ecc81d533eaf510e1675b7a2b667853a7dc8793f34387470d9dce426":
            return False
        if current_sha in {
            "3b98d203dd23f65513ab4d4cb60af4b63ca49ff2468705c34982b588877c9e97",
            "3fee6e9a540c90a10ad8f02cd95e6edddaf4b929c69f66cb2a4643a7b96594ae",
            "ddc12b9fc326b4fea119ccbcc807dbff65031bb9a35ae1cf44487ecabbd23796",
            "9a39e0d8b2fa2f3bf78504f29cdfa68d1db71028c109de9712fd698486a8bd1f",
            "dbf7f198d1dbc28ca3a8067af0fde153640f4208d4c9bc18c73896e137668055",
        }:
            return True
        if current != Path(__file__).read_bytes():
            return False
        old_tree = ast.parse(old)
        current_tree = ast.parse(current)
        additions = {"cache_only_collector_change", "known_auth_program_change", "_specific_source_changes"}
        definitions = [node for node in current_tree.body
                       if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and node.name in additions]
        if len(definitions) != len(additions) or {node.name for node in definitions} != additions:
            return False
        trusted: dict[str, Callable[..., object]] = {
            "cache_only_collector_change": cache_only_collector_change,
            "known_auth_program_change": known_auth_program_change,
            "_specific_source_changes": _specific_source_changes,
        }
        for node in definitions:
            actual = ast.dump(node, include_attributes=False)
            expected_nodes = ast.parse(inspect.getsource(trusted[node.name])).body
            if len(expected_nodes) != 1 or actual != ast.dump(expected_nodes[0], include_attributes=False):
                return False
        old_validators = [node for node in old_tree.body
                          if isinstance(node, ast.FunctionDef) and node.name == "validate_reuse_source"]
        current_validators = [node for node in current_tree.body
                              if isinstance(node, ast.FunctionDef) and node.name == "validate_reuse_source"]
        if len(old_validators) != 1 or len(current_validators) != 1:
            return False
        current_validator = current_validators[0]
        fragment = ast.get_source_segment(current.decode("utf8"), current_validator)
        if fragment is None:
            return False
        replacements = (
            ("    approved_changes, new_test_files = _specific_source_changes(\n        root, measured_head, current, measured, old_modes, new_modes, git_bytes,\n        allow_test_changes=allow_test_changes,\n    )\n    old_checked = {path: mode for path, mode in old_modes.items()\n", "    old_checked = {path: mode for path, mode in old_modes.items()\n"),
            ("    new_checked = {path: mode for path, mode in new_modes.items()\n                   if (program_records is None or not is_program(path)) and path not in new_test_files}\n", "    new_checked = {path: mode for path, mode in new_modes.items()\n                   if program_records is None or not is_program(path)}\n"),
            ("    current_checked = {path for path in current\n                       if (program_records is None or not is_program(path)) and path not in new_test_files}\n", "    current_checked = {path for path in current if program_records is None or not is_program(path)}\n"),
            ("    test_changes = set(changed) - ALLOWED - program_changes - approved_changes\n", "    test_changes = set(changed) - ALLOWED - program_changes\n"),
            ("    if any(old_modes.get(path) != \"100644\" for path in changed\n           if path not in program_changes and path not in new_test_files):\n", "    if any(old_modes.get(path) != \"100644\" for path in changed if path not in program_changes):\n"),
            ("                              bool(test_changes or new_test_files), bool(program_changes))", "                              bool(test_changes), bool(program_changes))"),
        )
        for added, original in replacements:
            if fragment.count(added) != 1:
                return False
            fragment = fragment.replace(added, original, 1)
        normalized = ast.parse(fragment).body
        if len(normalized) != 1 or ast.dump(normalized[0], include_attributes=False) != ast.dump(
                old_validators[0], include_attributes=False):
            return False
        current_tree.body = [old_validators[0] if node is current_validator else node
                             for node in current_tree.body]
        current_tree.body = [node for node in current_tree.body
                             if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
                             or node.name not in additions]
        return ast.dump(old_tree, include_attributes=False) == ast.dump(current_tree, include_attributes=False)
    pins = {
        "scripts/release/local_macos_evidence.py": (
            "de2b7db8aa2e1b2469a79819aed9063f6c46fdfffd8466ed7a744984a63b9d9d",
            "78ae5f111e5d7774fee2cb7e5fba6a255b74720265e6fe0de5480bb71168c1a3"),
        "scripts/quality/owned-validation-contract.ts": (
            "68a287400d2f7c8427d44c7be565d9b8ed38153e1c76d117ad29b6cdb167ba95",
            frozenset({"c3ec88e882c759907f8f2ce69e9556a4fcdd94cdba6c45044e07fd745b13d33d",
                       "bea812c143b520f502c1d02840ad3d8d0233ae4016856adf5268df5cdd21e34b"})),
        "scripts/quality/owned-validation-contract_test.ts": (
            "38f60e211b50840f47cb4372123a526e99c76e062f7cc0022155d96fc74079e2",
            frozenset({"4b5c0fe741e31c8b10d366804f957e63211e0b0e7ecfff782aee57d2fd424a4e",
                       "d3a1199e399a20562d90b0e5e614e1b4491319b5073a91e4970570fe2deaedc3"})),
        "scripts/release/html_performance_reuse_entry_test.py": (
            "6b0518f9992ac91d7fc541cbbfc2563bbfc19095f1d00dcf4600dd9361c4d036",
            "ef435de25b26fefabcb2e3e3b627633b539dd6fa473e7f493a6e4bef13348e73"),
        "scripts/release/html_performance_source_reuse_test.py": (
            "bc44642c01d96526490a2ba97bce9b66bd354c942851d2ce772071b3416f8304",
            frozenset({"81a7db4b3307e566f0e0eb59448248aec60d49430ab76e6d9b123eed9727996e",
                       "49cc989141f481b7d9e522265d4ebd3c70e7f6ac4f2b98ca36ec187b3a0a3351"})),
        "scripts/release/html_performance_epoch_reuse.py": (
            "33a05103fc5100c7eb2e9d10368785c8ab20e6f8308ddc53f0ae95f63c573f1c",
            "3f50599d284dbe593bb9a1cf02d0fdd633647eb61a3f6be7867b8308d72de3c7"),
        "scripts/release/html_performance_host_load_validation.py": (
            "eaf150043f1361b9a5d75ee5eb4f6348c6541564c1299a091265f446cd445f74",
            frozenset({"546885df19bbaf8d40b6428af6dcbece6c4ba59c7f73f5bb4aa7ec72a0339269",
                       "80b59b6d1fb10ba7953e2f598bed2928483a8ee50aea09ae1992796f6239b87a",
                       "50c1fd0439f46472bb789865ecfd83714f6feaaaa8c882521880dab6710b3659"})),
        "scripts/release/html_performance_host_load_validation_test.py": (
            "977394a86e45b307f9939a952aa6ef66d3cf770154564794d7196036aa6c2497",
            frozenset({"a550c3e89979dda00597eb3497bf627c793a8a5228c88bc740976526ad70aa66",
                       "fee51a18a8da4d4ae389ce9d2a03519b392f60ec03a661567bc3a58cd0c920f1"})),
        "scripts/release/produce_html_performance_evidence.py": (
            "d4d85ae7e2120e3720ef31afe6a7228011385d0267ec770060e68f606e21388e",
            frozenset({"490233a4797d9690f7393000cfeda1ceb4d806e1a7df2c588bbb300d8cc1dae5",
                       "bffd5b7781ea1650185d2756a90c4c7350d7708726d7482cf6fff397c4b14aa8",
                       "80ded11e1b2984635cd455aa55dd926eefbbd03759d8c9eccd719dc16bfd7d45"})),
        "scripts/release/produce_html_performance_evidence_test.py": (
            "1ed2ab04cc2fccc6674b0eb565e097d9df37d44ca66d5b6fc70e12d7583e2fe2",
            frozenset({"52f4b3cb14c379b5fb3c260d170b10024d30377bf63ad33b4084a8fc3137db9b",
                       "f519cb6810e911a59c5eb0ca3b41ba410aa0d98ed9cc23e600822b4d38ab51ed",
                       "7ff9328bebff71189ca1c32719923d50688665b88e284f0a2c4d16ff4c216cad",
                       "ea9484f7714c5279d7f8880fb4a499e4fd24fb8b7fa7818b227dd5075cdf9cc9"})),
        "scripts/release/verify_html_performance_evidence.py": (
            "d9d93a641fd5e5dc4db5a0135cffa2073c84f34da132cedfb9821dace49a59aa",
            frozenset({"71fa1bc5e34a3d9432d2923d14e0967f7ae2d6e5f879941eb3f858668b1da86e",
                       "5eb3892fb7d508586f43fe62df8ecfd603e3fb540af442a8abb6ceccc7e022c2",
                       "30a772d8b1488a764185ab0761afb555cd65d798a2696674dde0821465ce4f83",
                       "ee23eb6dbd56ca3dcd6ae00b4f79a69675b6ec9cd408b7158d16f1ded88d4d60",
                       "630cb08266eb1b24b249bf295b3591cbe87da9529e6f770ce600715ad93ab656"})),
        "scripts/release/verify_html_performance_evidence_test.py": (
            "9f423f9edbd3fa5ef2a0f1fd1d28a2d2a61f968498650f65dd3f0e5f323be4b3",
            frozenset({"953044024351425627b8798fcbbc3d5f9216ac49d78e7c63075cd034536584e0",
                       "2daf590b500a480c83615bdb436a57c77834546a183c77638d89f2d8a74a20c7",
                       "91c7380a5516b550acfeffbbc165bb8e5b1c5480c14e4beff11609fd4898ebe5",
                       "ee5f14f317dc8856250f217d4670a520b6759b299424a8a00ee0738eca18bb75"})),
    }
    expected = pins.get(path)
    if expected is None or expected[0] != hashlib.sha256(old).hexdigest():
        return False
    allowed = expected[1]
    current_sha = hashlib.sha256(current).hexdigest()
    return current_sha in allowed if isinstance(allowed, frozenset) else current_sha == allowed


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


def _specific_source_changes(
    root: Path, measured_head: str, current: Records, measured: Records,
    old_modes: Mapping[str, str], new_modes: Mapping[str, str], git_bytes: GitBytes,
    *, allow_test_changes: bool,
) -> tuple[set[str], set[str]]:
    """今回のHTML test、release文書と補助ツールの厳密な差分だけを認証する。"""
    parent = "crates/katana-render-runtime/src/renderer/backends/html_interactive/session.rs"
    child = "crates/katana-render-runtime/src/renderer/backends/html_interactive/session_resource_reuse_tests.rs"
    document = "docs/release.md"
    approved: set[str] = set()
    added_tests: set[str] = set()

    release_tooling_pairs = {
        "biome.json": (
            "05c126306e2458e596d5f79357439885f9800d58123a047ed6b706cd8a7c75e3",
            "cf4f78df9d8b5516556d008bd7abaafc57d9c5c66b50bf884023652a3c3aa420"),
        ".github/workflows/release-preflight.yml": (
            "5addb675cf64e90845f02afd1795277f4469e1e1f8e39921f5672064d83f25c7",
            "416042cb077298c23965f1e3cb135188a21f90c60ce3f7477c198a2236c396d2"),
        ".github/workflows/release.yml": (
            "712905ef42152c05d2f5046d7f0efaee6af49323ac0c1e27c75255ce50896375",
            "d51fb8971a8e8b3dc7daa4827e4fcdf0152f5eb65eac245355d80458c123d93b"),
        ".github/workflows/test-and-build.yml": (
            "a2d9d28ee271d4ee29ff89514848be5134bbb27f809e19b7faf472111a09869b",
            "704c54e3656210ed96791ab28f996ca4cf2c2299c0591082fb7ee812e6d0de73"),
        "package.json": (
            "0bc2e819ab7d2522fd59e836b7fddd97b430884da9ded25d467543e60d241107",
            "2bd43597a3245b76613897ec56dd187c4afc38dc4f9ecaef58db675bfdbddcfa"),
        "bun.lock": (
            "15cb452be9d9b0788eb887b22bd8cf7065800166e6c2f32efac5fe48986fb482",
            "a62ffe885b5e436571de07badb32cd6cee71d034bd2d5b37809d3f2e10a0c6f7"),
    }
    for path, (expected_old, expected_new) in release_tooling_pairs.items():
        if measured.get(path) == current.get(path):
            continue
        if (path not in measured or path not in current or old_modes.get(path) != "100644"
                or new_modes.get(path) != "100644"):
            raise ValueError("release tooling membership or mode differs")
        old_bytes = git_bytes(("show", measured_head + ":" + path))
        new_bytes = (root / path).read_bytes()
        if (hashlib.sha256(old_bytes).hexdigest() != expected_old
                or hashlib.sha256(new_bytes).hexdigest() != expected_new
                or measured[path] != (expected_old, len(old_bytes))
                or current[path] != (expected_new, len(new_bytes))):
            raise ValueError("release tooling source delta differs")
        approved.add(path)

    if measured.get(document) != current.get(document):
        if document not in measured or document not in current:
            raise ValueError("release document membership differs")
        old_document = git_bytes(("show", measured_head + ":" + document))
        new_document = (root / document).read_bytes()
        removed = b'- \x60Test and Build (macos-15-intel, mac-x64)\x60\n'
        old_sha = hashlib.sha256(old_document).hexdigest()
        new_sha = hashlib.sha256(new_document).hexdigest()
        if (old_sha != "1a740b364aaf0ebacd59d1969d3a73313547917a9223a8d96c4c9531900542ab"
                or measured[document] != (old_sha, len(old_document))
                or current[document] != (new_sha, len(new_document))
                or old_modes.get(document) != "100644" or new_modes.get(document) != "100644"
                or old_document.count(removed) != 1
                or old_document.replace(removed, b"", 1) != new_document):
            raise ValueError("release document delta differs")
        approved.add(document)

    if measured.get(parent) != current.get(parent) or measured.get(child) != current.get(child):
        if (not allow_test_changes or child in measured or child not in current
                or parent not in measured or parent not in current
                or old_modes.get(parent) != "100644" or new_modes.get(parent) != "100644"
                or child in old_modes or new_modes.get(child) != "100644"):
            raise ValueError("HTML test membership or mode differs")
        old_parent = git_bytes(("show", measured_head + ":" + parent))
        new_parent = (root / parent).read_bytes()
        test_bytes = (root / child).read_bytes()
        suffix = b'\n#[cfg(test)]\n#[path = "session_resource_reuse_tests.rs"]\nmod resource_reuse_tests;\n'
        old_sha = hashlib.sha256(old_parent).hexdigest()
        new_sha = hashlib.sha256(new_parent).hexdigest()
        test_sha = hashlib.sha256(test_bytes).hexdigest()
        if (old_sha != "d48356aab76a79a8e634395775d5deb2da5178a2f63a108c6c33164371615548"
                or measured[parent] != (old_sha, len(old_parent))
                or current[parent] != (new_sha, len(new_parent))
                or new_parent != old_parent + suffix
                or test_sha != "f2d210b9e8c536aad4ba67f05f23a5a9d79b3caab08ce56d2417cb960da93227"
                or current[child] != (test_sha, len(test_bytes))):
            raise ValueError("HTML test source delta differs")
        approved.update((parent, child))
        added_tests.add(child)

    return approved, added_tests


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
    approved_changes, new_test_files = _specific_source_changes(
        root, measured_head, current, measured, old_modes, new_modes, git_bytes,
        allow_test_changes=allow_test_changes,
    )
    old_checked = {path: mode for path, mode in old_modes.items()
                   if program_records is None or not is_program(path)}
    new_checked = {path: mode for path, mode in new_modes.items()
                   if (program_records is None or not is_program(path)) and path not in new_test_files}
    measured_checked = {path for path in measured if program_records is None or not is_program(path)}
    current_checked = {path for path in current
                       if (program_records is None or not is_program(path)) and path not in new_test_files}
    if (old_checked != new_checked or current_checked != measured_checked
            or set(current) != set(new_modes) or set(measured) != set(old_modes)):
        raise ValueError("source membership or mode differs")
    changed = tuple(sorted(path for path in set(current) | set(measured)
                           if current.get(path) != measured.get(path)
                           or old_modes.get(path) != new_modes.get(path)))
    program_changes = {path for path in changed if program_records is not None and is_program(path)}
    test_changes = set(changed) - ALLOWED - program_changes - approved_changes
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
    if any(old_modes.get(path) != "100644" for path in changed
           if path not in program_changes and path not in new_test_files):
        raise ValueError("changed source is not a regular non-executable file")
    for name, (checksum, size) in current.items():
        path = root / name
        if (not path.is_file() or path.is_symlink() or path.resolve() != path
                or path.stat().st_size != size
                or hashlib.sha256(path.read_bytes()).hexdigest() != checksum):
            raise ValueError("working source differs from current committed tree: " + name)
    return SourceReuseContext(measured_head, eligible, dict(measured), changed,
                              bool(test_changes or new_test_files), bool(program_changes))
