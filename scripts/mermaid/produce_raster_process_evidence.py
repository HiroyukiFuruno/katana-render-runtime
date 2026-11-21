"""Bind a Mermaid raster producer to the existing native CLI lifecycle owner."""
from __future__ import annotations

import argparse
import json
import os
import re
import secrets
import sys
from collections.abc import Mapping
from pathlib import Path
from typing import cast

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "build"))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "release"))
import cargo_capacity_guard_native_cli_observation as guard
import produce_html_performance_evidence as owner
import verify_html_performance_evidence as gate

JsonObject = dict[str, object]
PROCESS_SCHEMA = "krr-mermaid-raster-process-evidence-v1"
IDENTITY_SCHEMA = "katana-own-browser-identity-observation-v1"
SESSION_SCHEMA = "katana-mermaid-raster-producer-input-v1"
KNOWN_SCRIPTS = {
    "official": "scripts/mermaid/diagram-update.ts",
    "candidate": "scripts/mermaid/rasterize-svg-dir.ts",
}
RASTER_ENVIRONMENT_KEYS = frozenset((
    "PATH", "HOME", "USER", "LOGNAME", "LANG", "LC_ALL", "LC_CTYPE", "TMPDIR", "TMP", "TEMP",
    "SYSTEMROOT", "SystemRoot", "WINDIR", "COMSPEC", "PATHEXT", "USERPROFILE", "LOCALAPPDATA",
    "APPDATA", "DISPLAY", "WAYLAND_DISPLAY", "XAUTHORITY",
    "__CF_USER_TEXT_ENCODING", "FONTCONFIG_FILE", "FONTCONFIG_PATH", "XDG_CACHE_HOME",
    "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_RUNTIME_DIR", "XDG_DATA_DIRS", "XDG_CONFIG_DIRS",
    "PLAYWRIGHT_BROWSERS_PATH", "PUPPETEER_EXECUTABLE_PATH",
    "PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH", "PLAYWRIGHT_FIREFOX_EXECUTABLE_PATH",
    "PLAYWRIGHT_WEBKIT_EXECUTABLE_PATH", "BROWSER", "CHROME_BIN", "CHROMIUM_BIN", "FIREFOX_BIN",
    "NODE_ENV", "TZ",
))
RENDER_ENVIRONMENT_PREFIXES = (
    "FONTCONFIG_", "XDG_", "PLAYWRIGHT_", "PUPPETEER_", "CHROME_", "CHROMIUM_", "FIREFOX_",
    "FREETYPE_", "PANGO_", "PANGOCAIRO_", "SKIA_", "LIBGL_", "MESA_", "VK_", "DYLD_", "LD_",
)
UNSUPPORTED_RENDER_ENVIRONMENT_KEYS = frozenset(("NODE_OPTIONS", "NODE_PATH", "BUN_OPTIONS"))


def raster_child_environment(environment: Mapping[str, str]) -> dict[str, str]:
    unsupported = sorted(
        key for key, value in environment.items()
        if value and (key in UNSUPPORTED_RENDER_ENVIRONMENT_KEYS
                      or key.startswith(RENDER_ENVIRONMENT_PREFIXES)
                      and key not in RASTER_ENVIRONMENT_KEYS)
    )
    guard.require(not unsupported,
                  "unsupported raster environment keys: " + ",".join(unsupported))
    return {key: value for key, value in environment.items() if key in RASTER_ENVIRONMENT_KEYS}


def _object(value: object, label: str) -> JsonObject:
    return gate._object(value, label)


def _text(value: object, label: str) -> str:
    return gate._text(value, label)


def _integer(value: object, label: str) -> int:
    return gate._integer(value, label)


def _timestamp(value: object, label: str) -> int:
    text = _text(value, label)
    guard.require(text.isdecimal(), f"{label} must be decimal")
    return int(text)


def _pin(path: Path) -> JsonObject:
    return guard.file_record(path)


def _require_absolute(path: Path, label: str) -> Path:
    guard.require(path.is_absolute() and path.resolve() == path, f"{label} must be absolute")
    return path


def _load_session(config: Path, expected_sha256: str) -> JsonObject:
    pin = _pin(config)
    guard.require(pin["sha256"] == expected_sha256, "raster config SHA differs")
    value = gate.read_json(config)
    guard.require(set(value) == {
        "schema", "diagnosticOnly", "accepted", "role", "attempt", "nonce", "inventory",
        "outputDir", "observerPython", "endpoints",
    }, "raster producer config fields differ")
    guard.require(value["schema"] == SESSION_SCHEMA and value["diagnosticOnly"] is True
                  and value["accepted"] is False, "raster producer config is not diagnostic")
    attempt = _require_absolute(Path(_text(value["attempt"], "raster attempt")), "raster attempt")
    guard.require(attempt.is_dir() and not attempt.is_symlink(), "raster attempt must be canonical")
    _text(value["nonce"], "raster nonce")
    role = value["role"]
    guard.require(role in ("official", "candidate"), "raster role differs")
    guard.require(isinstance(value["inventory"], list) and len(value["inventory"]) > 0,
                  "raster inventory missing")
    return value


def _path_pin(path: Path, label: str) -> JsonObject:
    _require_absolute(path, label)
    return _pin(path)


def _known_command(root: Path, session: JsonObject, config: Path, config_sha256: str,
                   role: str, bun: Path, fixtures: Path, input_dir: Path, output_dir: Path,
                   fixture_glob: str, mermaid_js: Path, zenuml_js: Path) -> list[str]:
    guard.require(role == session["role"], "requested raster role differs from config")
    configured_output = _require_absolute(Path(_text(session["outputDir"], "raster output")), "raster output")
    guard.require(configured_output == output_dir, "raster output differs from config")
    script = root / KNOWN_SCRIPTS.get(role, "")
    guard.require(script.is_file() and script.resolve() == script, "known raster script is missing")
    _path_pin(bun, "Bun executable")
    guard.require((bun.stat().st_mode & 0o111) != 0, "Bun executable is not executable")
    for path, label in ((fixtures, "fixtures"), (input_dir, "input directory"),
                        (output_dir, "output directory"), (mermaid_js, "Mermaid runtime"),
                        (zenuml_js, "ZenUML runtime")):
        _require_absolute(path, label)
    common = ["--theme", "dark", "--reference-mode", "strictVendorOriginal",
              "--reference-scope", "formal", "--raster-session-config", str(config),
              "--raster-session-config-sha", config_sha256]
    if role == "official":
        return [str(bun), "run", str(script), "--fixtures", str(fixtures), "--output", str(output_dir), *common,
                "--mermaid-js", str(mermaid_js), "--mermaid-zenuml-js", str(zenuml_js)]
    return [str(bun), "run", str(script), "--input", str(input_dir), "--output", str(output_dir),
            *common, "--fixtures-dir", str(fixtures), "--fixture-glob", fixture_glob]


def _start_ns(identity: str) -> int:
    match = re.fullmatch(r"([0-9]+)\.([0-9]{1,6})", identity)
    if match is None:
        raise ValueError("producer start identity is not a kernel timestamp")
    return int(match[1]) * 1_000_000_000 + int(match[2].ljust(6, "0")) * 1_000


def _browser_observation(path: Path, session: JsonObject) -> tuple[JsonObject, JsonObject]:
    value = gate.read_json(path)
    guard.require(set(value) == {
        "schema", "role", "attempt", "nonce", "cdpBrowserPid", "osMeasurement",
        "collectorExitCode", "collectorReaped", "collectorPid", "python", "collector", "endpoint",
    }, "browser observation fields differ")
    guard.require(value["schema"] == IDENTITY_SCHEMA and value["role"] == session["role"]
                  and value["attempt"] == session["attempt"] and value["nonce"] == session["nonce"],
                  "browser observation session differs")
    measurement = _object(value["osMeasurement"], "OS browser measurement")
    browser = _object(measurement.get("browser"), "browser process")
    producer = _object(measurement.get("producer"), "producer process")
    collector = _object(measurement.get("collector"), "collector process")
    endpoint = _object(value["endpoint"], "browser endpoint")
    guard.require(measurement.get("schema") == "katana-macos-owned-browser-process-v1"
                  and measurement.get("platform") == "darwin"
                  and measurement.get("beforeEndEqual") is True
                  and value["collectorExitCode"] == 0 and value["collectorReaped"] is True,
                  "browser OS measurement closure differs")
    browser_pid = _integer(browser.get("pid"), "browser PID")
    producer_pid = _integer(producer.get("pid"), "producer PID")
    collector_pid = _integer(collector.get("pid"), "collector PID")
    guard.require(_integer(value["cdpBrowserPid"], "CDP browser PID") == browser_pid
                  and _integer(value["collectorPid"], "collector PID") == collector_pid
                  and _integer(collector.get("parentPid"), "collector parent PID") == producer_pid,
                  "browser collector ownership differs")
    guard.require(producer_pid != browser_pid and collector_pid != producer_pid,
                  "browser process ownership PIDs overlap")
    guard.require(endpoint.get("kind") == "browser" and endpoint.get("before") == endpoint.get("end"),
                  "browser endpoint changed during observation")
    chain_value = measurement.get("parentChain")
    guard.require(isinstance(chain_value, list) and len(chain_value) > 0, "browser parent chain missing")
    chain = cast(list[object], chain_value)
    browser_parent_pid = _integer(browser.get("parentPid"), "browser parent PID")
    parents = [_object(row, "parent") for row in chain]
    guard.require(len(parents) <= 64 and len({row.get("pid") for row in parents}) == len(parents)
                  and all(row.get("pid") != browser_pid for row in parents)
                  and parents[0].get("pid") == browser_parent_pid
                  and all(parents[index].get("parentPid") == parents[index + 1].get("pid")
                          for index in range(len(parents) - 1))
                  and parents[-1].get("pid") == producer_pid
                  and parents[-1].get("startIdentity") == producer.get("startIdentity"),
                  "browser parent chain does not reach producer")
    return value, {"browser": browser, "producer": producer, "endpoint": endpoint,
                   "parentChain": chain}


def bind_process_evidence(root: Path, config: Path, config_sha256: str, role: str, bun: Path,
                          fixtures: Path, input_dir: Path, output_dir: Path, fixture_glob: str,
                          mermaid_js: Path, zenuml_js: Path) -> Path:
    session = _load_session(config, config_sha256)
    attempt = Path(_text(session["attempt"], "raster attempt"))
    before = sorted(attempt.glob("browser-identity-*.json"))
    owner_path = attempt / "raster-process-owner-observation.json"
    output = attempt / "raster-process-evidence.json"
    guard.require(not before and not owner_path.exists() and not output.exists(),
                  "raster attempt already contains process evidence")
    token = secrets.token_hex(32)
    argv = _known_command(root, session, config, config_sha256, role, bun, fixtures, input_dir,
                          output_dir, fixture_glob, mermaid_js, zenuml_js)
    guard.require(argv[0] == str(bun), "raster producer executable differs from requested Bun")
    child_environment = raster_child_environment(os.environ)
    evidence = owner.observe_native_cli_command(argv, role="build", cwd=root, env=child_environment,
        log_directory=attempt / "lifecycle", attempt_token=token, label=f"mermaid-raster-{role}")
    observation = _object(evidence.observation, "native owner observation")
    observed_argv = observation.get("argv")
    guard.require(isinstance(observed_argv, list) and len(observed_argv) > 0
                  and observed_argv[0] == argv[0],
                  "native owner executable differs from controlled Bun")
    lifecycle_path = Path(_text(evidence.lifecycle["path"], "native lifecycle path"))
    lifecycle = gate.read_json(lifecycle_path)
    identity = _object(lifecycle.get("identity"), "native lifecycle identity")
    guard.require(observation.get("pid") == identity.get("pid") and observation.get("actualExit") == 0
                  and observation.get("reaped") is True and observation.get("errors") == [],
                  "native owner observation differs from lifecycle")
    guard.require(lifecycle.get("groupClosed") is True and lifecycle.get("rawFdsClosed") is True
                  and lifecycle.get("failure") is None and lifecycle.get("cleanupFailure") is None
                  and lifecycle.get("exitCode") == 0,
                  "native owner lifecycle closure differs")
    paths = sorted(attempt.glob("browser-identity-*.json"))
    guard.require(len(paths) == 2 and paths != before, "exactly two fresh browser observations required")
    parsed = [_browser_observation(path, session) for path in paths]
    first = parsed[0][1]
    for _, current in parsed[1:]:
        guard.require(current == first, "browser identity changed between observations")
    producer = cast(JsonObject, first["producer"])
    producer_pid = _integer(producer["pid"], "producer PID")
    producer_start = _text(producer["startIdentity"], "producer start identity")
    guard.require(producer_pid == observation["pid"]
                  and producer_start == _text(producer.get("startIdentity"), "producer start identity"),
                  "browser producer is not the owned direct child")
    started_ns = _timestamp(observation["startedNs"], "owner start time")
    ended_ns = _timestamp(observation["endedNs"], "owner end time")
    guard.require(started_ns // 1_000 <= _start_ns(producer_start) // 1_000 <= ended_ns // 1_000,
                  "producer start identity is outside owned spawn interval")
    browser = cast(JsonObject, first["browser"])
    endpoint = cast(JsonObject, first["endpoint"])
    parent_chain = cast(list[object], first["parentChain"])
    process: JsonObject = {
        "pid": _integer(browser["pid"], "browser PID"),
        "startIdentity": _text(browser["startIdentity"], "browser start identity"),
        "producerPid": producer_pid,
        "producerStartIdentity": producer_start,
        "parentChain": [{"pid": _integer(_object(row, "parent")["pid"], "parent PID"),
                          "startIdentity": _text(_object(row, "parent")["startIdentity"], "parent start")}
                         for row in parent_chain],
        "executable": _object(endpoint["end"], "browser executable identity"),
        "argv": cast(list[object], browser["argv"]),
        "version": _text(endpoint["version"], "browser version"),
        "beforeEndEqual": True,
        "producerExitCode": _integer(lifecycle["exitCode"], "producer exit code"),
        "reaped": observation["reaped"],
        "groupGone": lifecycle["groupClosed"],
    }
    with owner_path.open("xb") as handle:
        handle.write(json.dumps(observation, sort_keys=True, separators=(",", ":")).encode() + b"\n")
    document: JsonObject = {
        "schema": PROCESS_SCHEMA, "diagnosticOnly": True, "accepted": False,
        "role": session["role"], "attempt": session["attempt"], "nonce": session["nonce"],
        "config": _pin(config), "receipt": _pin(attempt / "raster-production-receipt.json"),
        "ownerObservation": _pin(owner_path), "lifecycle": _pin(lifecycle_path),
        "browserObservations": [_pin(path) for path in paths], "browserProcess": process,
        "descendantClosureProved": False,
    }
    with output.open("xb") as handle:
        handle.write(json.dumps(document, sort_keys=True, separators=(",", ":")).encode() + b"\n")
    return output


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    for name in ("config", "config-sha256", "role", "bun", "fixtures", "input", "output",
                 "fixture-glob", "mermaid-js", "mermaid-zenuml-js"):
        parser.add_argument(f"--{name}", required=True)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    root = Path(__file__).resolve().parents[2]
    bind_process_evidence(root, Path(args.config).resolve(), args.config_sha256, args.role,
                          Path(args.bun).resolve(), Path(args.fixtures).resolve(),
                          Path(args.input).resolve(), Path(args.output).resolve(), args.fixture_glob,
                          Path(args.mermaid_js).resolve(), Path(args.mermaid_zenuml_js).resolve())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
