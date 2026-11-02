"""Tests for fixed runtime-unused producer profiles."""

from __future__ import annotations

import unittest

import runtime_script_profiles as profiles


class RuntimeScriptProfilesTests(unittest.TestCase):
    def test_exact_seven_profiles_and_recipes(self) -> None:
        self.assertEqual(profiles.RUNTIME_SCRIPT_PROFILES, {
            "runtime-unused-fragment-loader": "./scripts/runtime-bundles/runtime-fragment-loader_test.ts",
            "runtime-unused-bundle-definitions": "./scripts/runtime-bundles/runtime-bundle-definitions_test.ts",
            "runtime-unused-check": "./scripts/runtime-bundles/runtime-unused-check_test.ts",
            "runtime-unused-tool-fingerprint": "./scripts/runtime-bundles/runtime-bundle-tool-fingerprint_test.ts",
            "runtime-unused-mathjax": "./scripts/runtime-bundles/runtime-unused-mathjax_test.ts",
            "runtime-unused-native-width": "./scripts/runtime-bundles/runtime-unused-native-width-hash_test.ts",
            "runtime-unused-rust-classic": "./scripts/runtime-bundles/runtime-unused-rust-classic_test.ts",
        })
        self.assertEqual(
            profiles.accepted_producers(),
            frozenset((*profiles.RUNTIME_SCRIPT_PROFILES, profiles.DEFAULT_PRODUCER)),
        )
        for producer, test_path in profiles.RUNTIME_SCRIPT_PROFILES.items():
            self.assertTrue(test_path.startswith("./scripts/runtime-bundles/"))
            self.assertTrue(test_path.endswith("_test.ts"))
            self.assertEqual(profiles.producer_recipe(producer), f"{producer}-run")

    def test_old_profile_remains_compatible_and_unknown_is_rejected(self) -> None:
        self.assertEqual(
            profiles.producer_recipe(profiles.DEFAULT_PRODUCER),
            "runtime-unused-script-regression-run",
        )
        with self.assertRaisesRegex(ValueError, "unknown_runtime_script_producer"):
            profiles.producer_recipe("unknown")
