"""Verify the narrowly defined RSS extraction in a historical measurement harness."""
from __future__ import annotations

import hashlib
import tomllib
from collections.abc import Callable, Mapping

Records = Mapping[str, tuple[str, int]]
ReadBlob = Callable[[str, str], bytes]
ROOT = "scripts/html/performance-harness/"
MANIFEST = ROOT + "Cargo.toml"
MAIN = ROOT + "src/main.rs"
RSS = ROOT + "src/rss.rs"
PROBE = ROOT + "src/bin/diagram_resource_probe.rs"
PROBE_TEST = ROOT + "tests/diagram_resource_probe.rs"
LOCK = ROOT + "Cargo.lock"
RETENTION = ROOT + "src/retention.rs"
RETENTION_TEST = ROOT + "src/retention_tests.rs"
CLI_TEST = ROOT + "tests/cli_execution.rs"
RETENTION_CLI_TEST = ROOT + "tests/cli_execution/retention_contract_tests.rs"
_RETENTION_FILES = {
    RETENTION: ("76fd880f1888c8a582d442bfaf256154be0d018d7d684e63996120898b19c1f2", 1570),
    RETENTION_TEST: ("1cfaf55f43daebb04155b8c76a1cfabdd92821025cd9df7369a7cea96aa85b59", 357),
    RETENTION_CLI_TEST: ("964eaeb545a2d6392c66f49859c83386daff30d4ff783962a74c9115eddd8ab1", 4773),
}
_RETENTION_DISPATCH = ('    #[cfg(feature = "candidate")]\n'
                       '    if env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--retention")) {\n'
                       '        return retention::run();\n    }\n')
_RETENTION_MODULE = ('#[cfg(feature = "candidate")]\n#[path = "retention.rs"]\nmod retention;\n\n')
_RETENTION_TEST_MODULE = ('#[cfg(feature = "candidate")]\n'
                          '#[path = "cli_execution/retention_contract_tests.rs"]\n'
                          'mod retention_contract_tests;\n')
_PROBE_SHA256 = "8165e428addc3accc7e0938847013b6963a75723de0e903512ba2356d4bee75a"
_CURRENT_PROBE = ("8106355f055cb8632f87e9e0faecb272c4f93676b199e5d06115e2da4cdcfddb", 7593)
_CURRENT_PROBE_TEST = ("1c59361923af0ab1dff6e16a4d2ad9084bbf81df29f60d0765a3369a08df1a68", 4451)

_CONSTANTS = ('#[cfg(target_os = "macos")]\nconst BYTES_PER_RSS_UNIT: u64 = 1;\n'
              '#[cfg(not(target_os = "macos"))]\nconst BYTES_PER_RSS_UNIT: u64 = 1024;\n')
_FUNCTION = ('fn peak_rss_bytes() -> Result<u64, Box<dyn std::error::Error>> {\n'
             '    /* WHY: 同じgetrusageのピークRSSを保ち、ABIとOSエラー処理は既存ライブラリへ委譲する。 */\n'
             '    let usage = nix::sys::resource::getrusage(nix::sys::resource::UsageWho::RUSAGE_SELF)?;\n'
             '    let value = u64::try_from(usage.max_rss())?;\n'
             '    Ok(value.saturating_mul(BYTES_PER_RSS_UNIT))\n}\n')


def _replace_once(value: str, old: str, new: str) -> str:
    if value.count(old) != 1:
        raise ValueError("historical harness transformation is ambiguous")
    return value.replace(old, new, 1)


def _checked_blob(read: ReadBlob, revision: str, path: str, records: Records) -> bytes:
    data = read(revision, path)
    if records.get(path) != (hashlib.sha256(data).hexdigest(), len(data)):
        raise ValueError("harness committed blob differs")
    return data


def _manifest_equivalent(old: bytes, current: bytes) -> None:
    original = tomllib.loads(old.decode("utf8"))
    revised = tomllib.loads(current.decode("utf8"))
    package = revised.get("package")
    if not isinstance(package, dict) or package.pop("default-run", None) != "krr-html-performance-harness":
        raise ValueError("harness default target differs")
    if revised.pop("bin", None) != [{"name": "diagram_resource_probe", "path": "src/bin/diagram_resource_probe.rs"}]:
        raise ValueError("harness diagnostic target differs")
    if original != revised:
        raise ValueError("harness package settings differ")


def _main_equivalent(old: bytes, current: bytes, rss: bytes) -> None:
    source = old.decode("utf8")
    updated = _replace_once(source, "mod provenance;\n", "mod provenance;\nmod rss;\n")
    updated = _replace_once(updated, "use provenance::{arm_name, hex_digest, provenance};\n",
                            "use provenance::{arm_name, hex_digest, provenance};\nuse rss::ProcessRss;\n")
    updated = _replace_once(updated, _CONSTANTS, "")
    updated = _replace_once(updated, "let peak_rss_bytes = peak_rss_bytes()?;",
                            "let peak_rss_bytes = ProcessRss::peak_bytes()?;")
    updated = _replace_once(updated, _FUNCTION + "\n", "")
    body = _FUNCTION.replace("fn peak_rss_bytes()", "    pub(crate) fn peak_bytes()", 1)
    body = "\n".join("    " + line if line else line for line in body.split("\n"))
    body = body.replace("        pub(crate) fn peak_bytes()", "    pub(crate) fn peak_bytes()", 1)
    expected_rss = _CONSTANTS + "\npub(crate) struct ProcessRss;\n\nimpl ProcessRss {\n" + body + "}\n"
    if updated.encode() != current or expected_rss.encode() != rss:
        raise ValueError("harness RSS extraction differs")


def _retention_equivalent(before: Records, after: Records, old_head: str, read: ReadBlob) -> None:
    for path, expected in _RETENTION_FILES.items():
        data = _checked_blob(read, "HEAD", path, after)
        if (hashlib.sha256(data).hexdigest(), len(data)) != expected:
            raise ValueError("harness retention source differs")
    for path, additions in ((MAIN, (_RETENTION_DISPATCH, _RETENTION_MODULE)),
                            (CLI_TEST, (_RETENTION_TEST_MODULE,))):
        old = _checked_blob(read, old_head, path, before)
        current = _checked_blob(read, "HEAD", path, after).decode("utf8")
        for addition in additions:
            current = _replace_once(current, addition, "")
        if current.encode() != old:
            raise ValueError("harness retention integration differs")


def verify_harness_sources(old: Records, current: Records, old_head: str, read: ReadBlob) -> None:
    before = {p: v for p, v in old.items() if p.startswith(ROOT) and p != LOCK}
    after = {p: v for p, v in current.items() if p.startswith(ROOT) and p != LOCK}
    if before == after:
        return
    additions = set(after) - set(before)
    if (additions == set(_RETENTION_FILES) and set(before) == set(after) - additions
            and all(before[path] == after[path] for path in before if path not in {MAIN, CLI_TEST})):
        _retention_equivalent(before, after, old_head, read)
        return
    if (additions not in ({RSS, PROBE}, {RSS, PROBE, PROBE_TEST}) or set(before) - set(after)
            or any(before[p] != after[p] for p in before if p not in {MAIN, MANIFEST})):
        raise ValueError("reference harness source differs")
    _manifest_equivalent(_checked_blob(read, old_head, MANIFEST, before),
                         _checked_blob(read, "HEAD", MANIFEST, after))
    _main_equivalent(_checked_blob(read, old_head, MAIN, before),
                     _checked_blob(read, "HEAD", MAIN, after),
                     _checked_blob(read, "HEAD", RSS, after))
    probe = _checked_blob(read, "HEAD", PROBE, after)
    if additions == {RSS, PROBE}:
        valid_probe = hashlib.sha256(probe).hexdigest() == _PROBE_SHA256
    else:
        test = _checked_blob(read, "HEAD", PROBE_TEST, after)
        valid_probe = ((hashlib.sha256(probe).hexdigest(), len(probe)) == _CURRENT_PROBE
                       and (hashlib.sha256(test).hexdigest(), len(test)) == _CURRENT_PROBE_TEST)
    if not valid_probe:
        raise ValueError("harness diagnostic source differs")
