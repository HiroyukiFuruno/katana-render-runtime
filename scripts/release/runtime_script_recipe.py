"""Read the fixed runtime-script leaf from Just's structured dump."""

from __future__ import annotations

import hashlib
import json
import shlex
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import cast

import runtime_script_profiles as profiles

RECIPE_NAME = profiles.producer_recipe(profiles.DEFAULT_PRODUCER)
TEST_PREFIX = ("bun", "test", "--path-ignore-patterns", "tmp/**")


class RuntimeScriptRecipeError(ValueError):
    """Raised when the fixed Just recipe cannot be read safely."""


@dataclass(frozen=True)
class RuntimeScriptRecipe:
    recipe_name: str
    recipe_sha256: str
    commands: tuple[tuple[str, ...], ...]


def _sha256(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _canonical_sha256(value: object) -> str:
    encoded = json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    return _sha256(encoded.encode())


def _read_dump(repo: Path) -> dict[str, object]:
    try:
        process = subprocess.run(
            ["just", "--dump", "--dump-format", "json"],
            cwd=repo,
            capture_output=True,
            check=False,
        )
    except OSError as error:
        raise RuntimeScriptRecipeError("just_dump_unavailable") from error
    if process.returncode != 0:
        raise RuntimeScriptRecipeError("just_dump_failed")
    try:
        value = json.loads(process.stdout)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise RuntimeScriptRecipeError("just_dump_invalid_json") from error
    if not isinstance(value, dict):
        raise RuntimeScriptRecipeError("just_dump_shape_unsupported")
    return cast(dict[str, object], value)


def _parse_commands(recipe: dict[str, object], producer: str) -> tuple[tuple[str, ...], ...]:
    body = recipe.get("body")
    if not isinstance(body, list) or len(body) != 1:
        raise RuntimeScriptRecipeError("just_recipe_body_unsupported")
    commands: list[tuple[str, ...]] = []
    for group in body:
        if not isinstance(group, list) or len(group) != 1 or not isinstance(group[0], str):
            raise RuntimeScriptRecipeError("just_recipe_command_unsupported")
        try:
            argv = tuple(shlex.split(group[0], posix=True))
        except ValueError as error:
            raise RuntimeScriptRecipeError("just_recipe_command_unsupported") from error
        if not argv:
            raise RuntimeScriptRecipeError("just_recipe_command_unsupported")
        commands.append(argv)
    if commands[0][: len(TEST_PREFIX)] != TEST_PREFIX:
        raise RuntimeScriptRecipeError("just_recipe_command_profile_changed")
    test_paths = commands[0][len(TEST_PREFIX) :]
    if not test_paths or any(
        not path.startswith("./scripts/runtime-bundles/") or not path.endswith("_test.ts")
        for path in test_paths
    ) or len(set(test_paths)) != len(test_paths):
        raise RuntimeScriptRecipeError("just_recipe_test_paths_unsupported")
    expected_path = profiles.RUNTIME_SCRIPT_PROFILES.get(producer)
    if expected_path is not None and tuple(test_paths) != (expected_path,):
        raise RuntimeScriptRecipeError("just_recipe_profile_test_mismatch")
    return tuple(commands)


def read_runtime_script_recipe(
    repo: Path, producer: str = profiles.DEFAULT_PRODUCER
) -> RuntimeScriptRecipe:
    """Return the recipe digest and literal argv groups for the fixed leaf."""
    recipe_name = profiles.producer_recipe(producer)
    dump = _read_dump(repo)
    recipes, assignments = dump.get("recipes"), dump.get("assignments")
    if not isinstance(recipes, dict) or not isinstance(assignments, dict):
        raise RuntimeScriptRecipeError("just_dump_shape_unsupported")
    recipe = recipes.get(recipe_name)
    if (
        not isinstance(recipe, dict)
        or recipe.get("name") != recipe_name
        or recipe.get("namepath") != recipe_name
        or recipe.get("dependencies") != []
        or recipe.get("parameters") != []
    ):
        raise RuntimeScriptRecipeError("just_recipe_profile_changed")
    commands = _parse_commands(cast(dict[str, object], recipe), producer)
    digest = _canonical_sha256({"recipe": recipe, "assignments": assignments})
    return RuntimeScriptRecipe(recipe_name, digest, commands)
