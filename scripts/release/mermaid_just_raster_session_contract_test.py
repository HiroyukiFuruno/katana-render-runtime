#!/usr/bin/env python3
"""実 just 展開と shell argv による外部 session 値の伝播を検査する。"""

from __future__ import annotations

import json
import os
import shlex
import subprocess
import tempfile
import unittest
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CONFIG_ENV = "MERMAID_RASTER_SESSION_CONFIG"
SHA_ENV = "MERMAID_RASTER_SESSION_CONFIG_SHA"
CONFIG_FLAG = "--raster-session-config"
SHA_FLAG = "--raster-session-config-sha"
SHA = "0123456789abcdef" * 4


@dataclass(frozen=True)
class RasterProducerRoute:
    recipe: str
    producer: str
    command_prefix: tuple[str, ...]
    config_env: str
    sha_env: str
    config_flag: str
    sha_flag: str
    required_arguments: tuple[tuple[str, str], ...]


STRICT_REFERENCE_ROUTE = RasterProducerRoute(
    "mermaid-reference-strict",
    "scripts/mermaid/produce_raster_process_evidence.py",
    ("python3",),
    CONFIG_ENV,
    SHA_ENV,
    "--config",
    "--config-sha256",
    (("--role", "official"),),
)
ROUTES = (
    STRICT_REFERENCE_ROUTE,
    *(
        RasterProducerRoute(
            recipe,
            "scripts/mermaid/rasterize-svg-dir.ts",
            ("bun", "run"),
            CONFIG_ENV,
            SHA_ENV,
            CONFIG_FLAG,
            SHA_FLAG,
            (
                ("--reference-mode", "strictVendorOriginal"),
                ("--reference-scope", "formal"),
            ),
        )
        for recipe in (
            "mermaid-compare",
            "mermaid-compare-prebuilt",
            "mermaid-capture-prebuilt",
        )
    ),
)


def producer_argv(
    route: RasterProducerRoute,
    config: str | None,
    sha: str | None,
) -> list[str]:
    env = os.environ.copy()
    for name, value in ((route.config_env, config), (route.sha_env, sha)):
        if value is None:
            env.pop(name, None)
        else:
            env[name] = value
    env["MERMAID_RASTER_ARG_CANARY"] = "unexpected-shell-expansion"
    result = subprocess.run(
        [
            "rtk",
            "proxy",
            "just",
            "--dry-run",
            route.recipe,
            "tests/fixtures/mermaid/representative",
        ],
        cwd=ROOT,
        env=env,
        capture_output=True,
        text=True,
        check=True,
    )
    prefix = f"{' '.join((*route.command_prefix, route.producer))} "
    commands = [line for line in (result.stdout + result.stderr).splitlines() if line.startswith(prefix)]
    if len(commands) != 1:
        raise AssertionError(f"expected one producer command, got {commands!r}")
    # WHY: 描画を実行せず、just が作った quoting を実 shell の引数展開で検査する。
    arguments = commands[0][len(prefix) :]
    serializer = "import json, sys; print(json.dumps(sys.argv[1:]))"
    shell = subprocess.run(
        [
            "rtk",
            "proxy",
            "bash",
            "-uc",
            f"set -- {arguments}; exec python3 -c {shlex.quote(serializer)} \"$@\"",
        ],
        cwd=ROOT,
        env=env,
        capture_output=True,
        text=True,
        check=True,
    )
    decoded: object = json.loads(shell.stdout)
    if not isinstance(decoded, list):
        raise AssertionError("shell argv must be a JSON list")
    argv: list[str] = []
    for value in decoded:
        if not isinstance(value, str):
            raise AssertionError("shell argv entries must be strings")
        argv.append(value)
    return argv


def flag_value(argv: list[str], flag: str) -> str:
    if argv.count(flag) != 1:
        raise AssertionError(f"expected exactly one {flag}, got {argv!r}")
    index = argv.index(flag)
    if index + 1 == len(argv):
        raise AssertionError(f"missing argument for {flag}")
    return argv[index + 1]


class MermaidJustRasterSessionContractTest(unittest.TestCase):
    routes: tuple[RasterProducerRoute, ...] = ROUTES

    def test_external_config_and_sha_keep_exact_shell_arguments_on_all_strict_routes(self) -> None:
        config = str(ROOT / "tmp" / "external session $MERMAID_RASTER_ARG_CANARY 'single' \"double\".json")
        for route in self.routes:
            with self.subTest(recipe=route.recipe):
                argv = producer_argv(route, config, SHA)
                self.assertEqual(flag_value(argv, route.config_flag), config)
                self.assertEqual(flag_value(argv, route.sha_flag), SHA)
                for flag, value in route.required_arguments:
                    self.assertEqual(flag_value(argv, flag), value)

    def test_unset_environment_keeps_empty_arguments_on_all_strict_routes(self) -> None:
        for route in self.routes:
            with self.subTest(recipe=route.recipe):
                argv = producer_argv(route, None, None)
                self.assertEqual(flag_value(argv, route.config_flag), "")
                self.assertEqual(flag_value(argv, route.sha_flag), "")

    def test_either_unset_external_value_remains_empty(self) -> None:
        config = str(ROOT / "tmp" / "caller-supplied session.json")
        for route in self.routes:
            for supplied_path, supplied_sha in ((None, SHA), (config, None)):
                with self.subTest(recipe=route.recipe, path=supplied_path, sha=supplied_sha):
                    argv = producer_argv(route, supplied_path, supplied_sha)
                    self.assertEqual(flag_value(argv, route.config_flag), supplied_path or "")
                    self.assertEqual(flag_value(argv, route.sha_flag), supplied_sha or "")


class MermaidJustRasterComparisonContractTest(MermaidJustRasterSessionContractTest):
    routes = tuple(
        RasterProducerRoute(
            recipe,
            "scripts/mermaid/reference-compare.ts",
            ("bun", "run"),
            "MERMAID_RASTER_BINDING_CONFIG",
            "MERMAID_RASTER_BINDING_CONFIG_SHA",
            "--raster-binding-config",
            "--raster-binding-config-sha",
            (
                ("--reference-mode", "strictVendorOriginal"),
                ("--reference-scope", "formal"),
            ),
        )
        for recipe in ("mermaid-compare", "mermaid-compare-prebuilt", "mermaid-score-strict")
    )

    def test_score_only_recipe_does_not_render_or_capture(self) -> None:
        result = subprocess.run(
            ["rtk", "proxy", "just", "--dry-run", "mermaid-score-strict", "tests/fixtures/mermaid/representative"],
            cwd=ROOT, capture_output=True, text=True, check=True,
        )
        expanded = result.stdout + result.stderr
        commands = [line.strip() for line in expanded.splitlines() if line.startswith("bun run ")]
        self.assertEqual(len(commands), 1)
        self.assertTrue(commands[0].startswith("bun run scripts/mermaid/reference-compare.ts "))
        self.assertNotIn("scripts/mermaid/rasterize-svg-dir.ts", expanded)
        self.assertNotIn("just mermaid-render-strict", expanded)
        self.assertNotIn("scripts/mermaid/diagram-update.ts", expanded)
        self.assertNotIn("scripts/mermaid/produce_raster_process_evidence.py", expanded)

    def test_existing_each99_threshold_is_preserved_on_both_comparison_routes(self) -> None:
        for route in self.routes:
            with self.subTest(recipe=route.recipe):
                argv = producer_argv(route, None, None)
                self.assertEqual(flag_value(argv, "--min-score"), "99")


class MermaidFullComparisonDomainContractTest(unittest.TestCase):
    values = {
        "MERMAID_RASTER_SESSION_CONFIG_EN": "/caller/en session.json",
        "MERMAID_RASTER_SESSION_CONFIG_SHA_EN": "1" * 64,
        "MERMAID_RASTER_BINDING_CONFIG_EN": "/caller/en binding.json",
        "MERMAID_RASTER_BINDING_CONFIG_SHA_EN": "2" * 64,
        "MERMAID_RASTER_SESSION_CONFIG_JA": "/caller/ja session.json",
        "MERMAID_RASTER_SESSION_CONFIG_SHA_JA": "3" * 64,
        "MERMAID_RASTER_BINDING_CONFIG_JA": "/caller/ja binding.json",
        "MERMAID_RASTER_BINDING_CONFIG_SHA_JA": "4" * 64,
    }

    def test_full_recipe_selects_distinct_pinned_config_pairs_and_roots(self) -> None:
        env = os.environ.copy()
        env.update(self.values)
        result = subprocess.run(
            [
                "rtk", "proxy", "just", "--dry-run", "mermaid-compare-full", "99",
                "tmp/full output", "tmp/official output",
            ],
            cwd=ROOT,
            env=env,
            capture_output=True,
            text=True,
            check=True,
        )
        expanded = result.stdout + result.stderr
        self.assertIn('en) session_config="$3"; session_sha="$4"; binding_config="$5"; binding_sha="$6"', expanded)
        self.assertIn('ja) session_config="$7"; session_sha="$8"; binding_config="$9"; binding_sha="${10}"', expanded)
        self.assertIn('"${11}/$slug" "*.md" "${12}/$slug"', expanded)
        self.assertIn('_ {} "99" "$session_config_en" "$session_sha_en" "$binding_config_en" "$binding_sha_en" "$session_config_ja" "$session_sha_ja" "$binding_config_ja" "$binding_sha_ja" "$compare_output" "$official_root"', expanded)
        self.assertIn("unset MERMAID_RASTER_SESSION_CONFIG_EN", expanded)
        self.assertIn('MERMAID_RASTER_SESSION_CONFIG="$session_config" MERMAID_RASTER_SESSION_CONFIG_SHA="$session_sha" MERMAID_RASTER_BINDING_CONFIG="$binding_config" MERMAID_RASTER_BINDING_CONFIG_SHA="$binding_sha" just mermaid-compare-prebuilt', expanded)
        for value in (*self.values.values(), "tmp/full output", "tmp/official output"):
            self.assertIn(value, expanded)
        defaults = subprocess.run(
            ["rtk", "proxy", "just", "--dry-run", "mermaid-compare-full"],
            cwd=ROOT,
            env=env,
            capture_output=True,
            text=True,
            check=True,
        )
        default_recipe = defaults.stdout + defaults.stderr
        self.assertIn("compare_output='tmp/krr-mermaid-full'", default_recipe)
        self.assertIn("official_root='tmp/krr-mermaid-strict-official'", default_recipe)

    def test_missing_language_config_fails_before_either_child_recipe(self) -> None:
        for missing_language in ("EN", "JA"):
            with self.subTest(missing_language=missing_language), tempfile.TemporaryDirectory() as temporary:
                receipt = Path(temporary) / "build-receipt.json"
                receipt.write_text("{}\n", encoding="utf-8")
                log_dir = Path(temporary) / "logs"
                env = os.environ.copy()
                for name in (*self.values, "MERMAID_RASTER_SESSION_CONFIG", "MERMAID_RASTER_SESSION_CONFIG_SHA", "MERMAID_RASTER_BINDING_CONFIG", "MERMAID_RASTER_BINDING_CONFIG_SHA"):
                    env.pop(name, None)
                env.update(self.values)
                env.update({
                    "MERMAID_RASTER_SESSION_CONFIG": "/caller/shared-session.json",
                    "MERMAID_RASTER_SESSION_CONFIG_SHA": "a" * 64,
                    "MERMAID_RASTER_BINDING_CONFIG": "/caller/shared-binding.json",
                    "MERMAID_RASTER_BINDING_CONFIG_SHA": "b" * 64,
                })
                missing_name = f"MERMAID_RASTER_SESSION_CONFIG_SHA_{missing_language}"
                env.pop(missing_name)
                env["KRR_BUILD_RECEIPT"] = str(receipt)
                env["RUNTIME_UPDATE_LOG_DIR"] = str(log_dir)
                result = subprocess.run(
                    ["rtk", "proxy", "just", "mermaid-compare-full", "tests/fixtures/mermaid/en"],
                    cwd=ROOT,
                    env=env,
                    capture_output=True,
                    text=True,
                    check=False,
                )
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(f"{missing_language} strict raster session and binding config/SHA are required", result.stderr)
                self.assertFalse(log_dir.exists(), "preflight must fail before starting either language worker")


if __name__ == "__main__":
    unittest.main()
