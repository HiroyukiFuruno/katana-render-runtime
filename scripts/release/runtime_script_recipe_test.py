"""Tests for the official Just AST reader used by runtime-script identities."""

from __future__ import annotations

import json
import subprocess
import unittest
from pathlib import Path
from unittest.mock import patch

import runtime_script_recipe as recipe
import runtime_script_profiles as profiles


TEST_COMMAND = (
    "bun test --path-ignore-patterns 'tmp/**' "
    "./scripts/runtime-bundles/example_test.ts"
)


def just_dump(
    test_command: str = TEST_COMMAND,
    recipe_name: str = recipe.RECIPE_NAME,
) -> dict[str, object]:
    return {
        "recipes": {
            recipe_name: {
                "name": recipe_name,
                "namepath": recipe_name,
                "dependencies": [],
                "parameters": [],
                "body": [[test_command]],
            }
        },
        "assignments": {"SHELL": "bash"},
    }


def invoke_dump(
    value: object, producer: str = profiles.DEFAULT_PRODUCER
) -> recipe.RuntimeScriptRecipe:
    process = subprocess.CompletedProcess([], 0, json.dumps(value).encode(), b"")
    with patch("runtime_script_recipe.subprocess.run", return_value=process):
        return recipe.read_runtime_script_recipe(Path("/repo"), producer)


class RuntimeScriptRecipeTests(unittest.TestCase):
    def test_reader_supports_each_fixed_leaf_recipe(self) -> None:
        for producer, test_path in profiles.RUNTIME_SCRIPT_PROFILES.items():
            with self.subTest(producer=producer):
                parsed = invoke_dump(
                    just_dump(
                        "bun test --path-ignore-patterns 'tmp/**' " + test_path,
                        profiles.producer_recipe(producer),
                    ),
                    producer,
                )
                self.assertEqual(parsed.recipe_name, f"{producer}-run")
                self.assertEqual(parsed.commands[0][-1], test_path)

    def test_reader_returns_ordered_literal_argv_without_copying_file_inventory(self) -> None:
        parsed = invoke_dump(just_dump())
        self.assertEqual(parsed.recipe_name, recipe.RECIPE_NAME)
        self.assertEqual(
            parsed.commands,
            (
                (
                    "bun",
                    "test",
                    "--path-ignore-patterns",
                    "tmp/**",
                    "./scripts/runtime-bundles/example_test.ts",
                ),
            ),
        )
        self.assertEqual(len(parsed.recipe_sha256), 64)

    def test_reader_rejects_unexpected_command_or_file_shape(self) -> None:
        for command in (
            "python3 -c 'pass'",
            TEST_COMMAND + " ./scripts/runtime-bundles/example_test.ts",
            TEST_COMMAND.replace("tmp/**", "**/*"),
        ):
            with self.subTest(command=command), self.assertRaises(recipe.RuntimeScriptRecipeError):
                invoke_dump(just_dump(command))

        producer = "runtime-unused-fragment-loader"
        with self.assertRaisesRegex(recipe.RuntimeScriptRecipeError, "profile_test_mismatch"):
            invoke_dump(
                just_dump(
                    TEST_COMMAND,
                    profiles.producer_recipe(producer),
                ),
                producer,
            )

    def test_reader_rejects_recipe_dependencies_and_extra_command_groups(self) -> None:
        dump = just_dump()
        recipes = dump["recipes"]
        assert isinstance(recipes, dict)
        leaf = recipes[recipe.RECIPE_NAME]
        assert isinstance(leaf, dict)
        leaf["dependencies"] = ["other"]
        with self.assertRaises(recipe.RuntimeScriptRecipeError):
            invoke_dump(dump)
        leaf["dependencies"] = []
        body = leaf.get("body")
        assert isinstance(body, list)
        body.append([TEST_COMMAND])
        with self.assertRaises(recipe.RuntimeScriptRecipeError):
            invoke_dump(dump)
