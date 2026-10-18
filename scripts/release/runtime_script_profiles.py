"""Fixed producer profiles for runtime-unused leaf tests."""

from __future__ import annotations

from typing import Final

DEFAULT_PRODUCER: Final = "runtime-unused-script-regression"

RUNTIME_SCRIPT_PROFILES: Final[dict[str, str]] = {
    "runtime-unused-fragment-loader": "./scripts/runtime-bundles/runtime-fragment-loader_test.ts",
    "runtime-unused-bundle-definitions": "./scripts/runtime-bundles/runtime-bundle-definitions_test.ts",
    "runtime-unused-check": "./scripts/runtime-bundles/runtime-unused-check_test.ts",
    "runtime-unused-tool-fingerprint": "./scripts/runtime-bundles/runtime-bundle-tool-fingerprint_test.ts",
    "runtime-unused-mathjax": "./scripts/runtime-bundles/runtime-unused-mathjax_test.ts",
    "runtime-unused-native-width": "./scripts/runtime-bundles/runtime-unused-native-width-hash_test.ts",
    "runtime-unused-rust-classic": "./scripts/runtime-bundles/runtime-unused-rust-classic_test.ts",
}


def producer_recipe(producer: str) -> str:
    if producer == DEFAULT_PRODUCER:
        return "runtime-unused-script-regression-run"
    if producer in RUNTIME_SCRIPT_PROFILES:
        return f"{producer}-run"
    raise ValueError("unknown_runtime_script_producer")


def accepted_producers() -> frozenset[str]:
    return frozenset((DEFAULT_PRODUCER, *RUNTIME_SCRIPT_PROFILES))
