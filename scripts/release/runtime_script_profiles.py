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

QUALITY_SCRIPT_PROFILES: Final[dict[str, str]] = {
    "runtime-quality-owned-source-inventory": "./scripts/quality/owned-source-inventory_test.ts",
    "runtime-quality-owned-validation-contract": "./scripts/quality/owned-validation-contract_test.ts",
}

QUALITY_SCRIPT_TEST_PATHS: Final[frozenset[str]] = frozenset(
    path.removeprefix("./") for path in QUALITY_SCRIPT_PROFILES.values()
)


def producer_recipe(producer: str) -> str:
    if producer == DEFAULT_PRODUCER:
        return "runtime-unused-script-regression-run"
    if producer in RUNTIME_SCRIPT_PROFILES or producer in QUALITY_SCRIPT_PROFILES:
        return f"{producer}-run"
    raise ValueError("unknown_runtime_script_producer")


def accepted_producers() -> frozenset[str]:
    return frozenset((DEFAULT_PRODUCER, *RUNTIME_SCRIPT_PROFILES, *QUALITY_SCRIPT_PROFILES))


def fixed_profile_test_path(producer: str) -> str | None:
    return RUNTIME_SCRIPT_PROFILES.get(producer) or QUALITY_SCRIPT_PROFILES.get(producer)


def is_supported_test_path(path: str, producer: str) -> bool:
    if not path.startswith("./"):
        return False
    normalized = path.removeprefix("./")
    if (
        normalized.startswith("scripts/runtime-bundles/")
        and normalized.endswith("_test.ts")
        and ".." not in normalized.split("/")
    ):
        return True
    return (
        normalized in QUALITY_SCRIPT_TEST_PATHS
        and QUALITY_SCRIPT_PROFILES.get(producer, "").removeprefix("./") == normalized
    )
