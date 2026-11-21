from __future__ import annotations

import os
import unittest


GIT_FIXTURE_ENVIRONMENT = (
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CEILING_DIRECTORIES",
    "GIT_DISCOVERY_ACROSS_FILESYSTEM",
    "GIT_PREFIX",
)


def isolate_git_fixture_environment(test_case: unittest.TestCase) -> None:
    saved = {
        name: os.environ[name]
        for name in GIT_FIXTURE_ENVIRONMENT
        if name in os.environ
    }
    for name in GIT_FIXTURE_ENVIRONMENT:
        os.environ.pop(name, None)

    def restore() -> None:
        for name in GIT_FIXTURE_ENVIRONMENT:
            os.environ.pop(name, None)
        os.environ.update(saved)

    test_case.addCleanup(restore)
