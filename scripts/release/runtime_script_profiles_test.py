"""Tests for fixed runtime-unused producer profiles."""

from __future__ import annotations

import unittest

import runtime_script_profiles as profiles


class RuntimeScriptProfilesTests(unittest.TestCase):
    def test_exact_seven_profiles_and_recipes(self) -> None:
        self.assertEqual(len(profiles.RUNTIME_SCRIPT_PROFILES), 7)
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
