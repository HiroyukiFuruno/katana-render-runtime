"""Tests for fixed runtime-unused producer profiles."""

from __future__ import annotations

import unittest

import runtime_script_profiles as profiles


class RuntimeScriptProfilesTests(unittest.TestCase):
    def test_exact_seven_runtime_bundle_profiles_and_recipes(self) -> None:
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
            frozenset(
                (
                    *profiles.RUNTIME_SCRIPT_PROFILES,
                    *profiles.QUALITY_SCRIPT_PROFILES,
                    profiles.DEFAULT_PRODUCER,
                )
            ),
        )
        for producer, test_path in profiles.RUNTIME_SCRIPT_PROFILES.items():
            self.assertTrue(test_path.startswith("./scripts/runtime-bundles/"))
            self.assertTrue(test_path.endswith("_test.ts"))
            self.assertEqual(profiles.producer_recipe(producer), f"{producer}-run")

    def test_two_exact_quality_profiles_are_accepted_without_changing_runtime_batch(self) -> None:
        self.assertEqual(
            profiles.QUALITY_SCRIPT_PROFILES,
            {
                "runtime-quality-owned-source-inventory": "./scripts/quality/owned-source-inventory_test.ts",
                "runtime-quality-owned-validation-contract": "./scripts/quality/owned-validation-contract_test.ts",
            },
        )
        self.assertEqual(len(profiles.RUNTIME_SCRIPT_PROFILES), 7)
        self.assertEqual(len(profiles.QUALITY_SCRIPT_PROFILES), 2)
        self.assertTrue(set(profiles.QUALITY_SCRIPT_PROFILES).issubset(profiles.accepted_producers()))
        for producer, test_path in profiles.QUALITY_SCRIPT_PROFILES.items():
            self.assertEqual(profiles.producer_recipe(producer), f"{producer}-run")
            self.assertEqual(profiles.fixed_profile_test_path(producer), test_path)

    def test_test_paths_require_dot_slash_for_all_fixed_and_default_profiles(self) -> None:
        for producer, test_path in (
            *profiles.RUNTIME_SCRIPT_PROFILES.items(),
            *profiles.QUALITY_SCRIPT_PROFILES.items(),
        ):
            with self.subTest(producer=producer):
                self.assertTrue(profiles.is_supported_test_path(test_path, producer))
                self.assertFalse(
                    profiles.is_supported_test_path(test_path.removeprefix("./"), producer)
                )
                self.assertFalse(
                    profiles.is_supported_test_path("/repo/" + test_path.removeprefix("./"), producer)
                )
        self.assertTrue(
            profiles.is_supported_test_path(
                "./scripts/runtime-bundles/arbitrary_test.ts", profiles.DEFAULT_PRODUCER
            )
        )
        self.assertFalse(
            profiles.is_supported_test_path(
                "scripts/runtime-bundles/arbitrary_test.ts", profiles.DEFAULT_PRODUCER
            )
        )

    def test_old_profile_remains_compatible_and_unknown_is_rejected(self) -> None:
        self.assertEqual(
            profiles.producer_recipe(profiles.DEFAULT_PRODUCER),
            "runtime-unused-script-regression-run",
        )
        with self.assertRaisesRegex(ValueError, "unknown_runtime_script_producer"):
            profiles.producer_recipe("unknown")
