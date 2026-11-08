from __future__ import annotations

import os
import stat
import subprocess
import sys
from collections.abc import Iterator
from contextlib import ExitStack, contextmanager
from pathlib import Path

from local_review_lock import review_lock
from local_review_state import ReviewError

ROOT = Path(__file__).resolve().parents[2]
LOCK_PATH = ROOT / ".moon/cache/krr-terminal-report.lock"
ROOT_PRODUCER_LOCK_PATH = ROOT / "tmp/quality/standalone-performance-coverage/root-nextest-producer.lock"
PERFORMANCE_COVERAGE_MODE = "--performance-coverage"
PERFORMANCE_COVERAGE_COMMAND = ["just", "performance-harness-coverage-run"]
LOCK_CONTENTION_MESSAGE = (
    "another verification or update is active; retry after it finishes"
)


class _LockContentionError(ReviewError):
    pass


@contextmanager
def _acquire_lock(path: Path) -> Iterator[None]:
    lock_stack = ExitStack()
    try:
        try:
            lock_stack.enter_context(review_lock(path))
        except ReviewError as error:
            raise _LockContentionError from error
        yield
    finally:
        lock_stack.close()


def _cargo_clean_target(command_arguments: list[str], root: Path) -> Path | None:
    build_scripts = str(Path(__file__).resolve().parents[1] / "build")
    if build_scripts not in sys.path:
        sys.path.insert(0, build_scripts)
    from cargo_capacity_guard import _unwrap_cargo

    words = _unwrap_cargo(command_arguments[1:])
    if len(words) < 2 or Path(words[0]).name != "cargo" or words[1] != "clean":
        return None

    target_value = os.environ.get("CARGO_TARGET_DIR")
    target = Path(target_value) if target_value is not None else root / "target"
    for index, argument in enumerate(words[2:]):
        if argument == "--target-dir":
            if index + 3 >= len(words):
                raise RuntimeError("cargo clean --target-dir requires a path")
            target = Path(words[index + 3])
        elif argument.startswith("--target-dir="):
            target = Path(argument.partition("=")[2])
    if not target.is_absolute():
        target = root / target
    target = target.resolve()
    if _is_dev_profile_clean(words[2:]) and not (target / "debug").is_symlink():
        target /= "debug"
    return target.resolve()


def _is_dev_profile_clean(options: list[str]) -> bool:
    selectors: list[str] = []
    index = 0
    while index < len(options):
        option = options[index]
        if option == "--target-dir":
            index += 2
        elif option.startswith("--target-dir="):
            index += 1
        else:
            selectors.append(option)
            index += 1
    return selectors in (["--profile", "dev"], ["--profile=dev"])


def _is_scoped_package_clean(command_arguments: list[str]) -> bool:
    from cargo_capacity_guard import _unwrap_cargo

    words = _unwrap_cargo(command_arguments[1:])
    if len(words) != 8 or words[0] != "cargo" or words[1:4] != ["clean", "--profile", "dev"]:
        return False
    return (words[4] == words[6] == "-p"
            and {words[5], words[7]} == {"katana-render-runtime", "katana-render-runtime-assets"})


def _stable_clean_path(path: Path) -> os.stat_result:
    if not path.is_absolute() or path.resolve(strict=True) != path:
        raise RuntimeError("clean plan contains a noncanonical or symbolic path")
    metadata = path.lstat()
    if not (stat.S_ISREG(metadata.st_mode) or stat.S_ISDIR(metadata.st_mode)):
        raise RuntimeError("clean plan contains an unsupported filesystem entry")
    return metadata


def _raise_walk_error(error: OSError) -> None:
    raise error


def _protected_clean_entries(references: list[str]) -> tuple[list[Path], set[tuple[int, int]]]:
    protected: list[Path] = []
    identities: set[tuple[int, int]] = set()
    for reference in references:
        path = Path(reference)
        details = _stable_clean_path(path)
        protected.append(path)
        identities.add((details.st_dev, details.st_ino))
        if stat.S_ISDIR(details.st_mode):
            for parent, directories, files in os.walk(path, followlinks=False, onerror=_raise_walk_error):
                for name in (*directories, *files):
                    child = Path(parent) / name
                    details = _stable_clean_path(child)
                    identities.add((details.st_dev, details.st_ino))
    return protected, identities


def _check_planned_directory_entries(planned: set[Path]) -> None:
    for path in planned:
        if path.is_dir():
            for parent, directories, files in os.walk(path, followlinks=False, onerror=_raise_walk_error):
                for name in (*directories, *files):
                    child = Path(parent) / name
                    _stable_clean_path(child)
                    if child not in planned:
                        raise RuntimeError("package clean plan omits a directory entry")


def _validate_package_clean_plan(command_arguments: list[str], root: Path, references: list[str]) -> None:
    target = root / "target"
    debug = target / "debug"
    _stable_clean_path(target)
    _stable_clean_path(debug)
    configured_target = Path(os.environ.get("CARGO_TARGET_DIR", str(target)))
    if not configured_target.is_absolute():
        configured_target = root / configured_target
    if configured_target != target or configured_target.resolve(strict=True) != target:
        raise RuntimeError("package clean requires the canonical repository target")
    protected, identities = _protected_clean_entries(references)
    try:
        result = subprocess.run([*command_arguments[1:], "--dry-run", "--verbose"], cwd=root,
                                check=False, shell=False, capture_output=True, text=True)
    except UnicodeError as error:
        raise RuntimeError("package clean dry-run output is not valid text") from error
    if result.returncode != 0 or not result.stdout or not result.stdout.endswith("\n"):
        raise RuntimeError("package clean dry-run failed or has no complete plan")
    planned: set[Path] = set()
    for line in result.stdout.splitlines():
        path = Path(line)
        if not line or str(path) != line or path == debug or not path.is_relative_to(debug):
            raise RuntimeError("package clean plan escaped the debug target")
        details = _stable_clean_path(path)
        if ((details.st_dev, details.st_ino) in identities
                or any(path == ref or path.is_relative_to(ref) or ref.is_relative_to(path) for ref in protected)):
            raise RuntimeError("package clean plan overlaps a protected reference")
        planned.add(path)
    if not planned:
        raise RuntimeError("package clean plan is empty")
    _check_planned_directory_entries(planned)


def _protected_clean_reference(command_arguments: list[str], root: Path) -> Path | None:
    build_scripts = str(Path(__file__).resolve().parents[1] / "build")
    if build_scripts not in sys.path:
        sys.path.insert(0, build_scripts)
    from cargo_capacity_guard import collect_protected_references

    target = _cargo_clean_target(command_arguments, root)
    if target is None:
        return None
    references = collect_protected_references(
        [], os.environ.get("CARGO_CAPACITY_PROTECTED_REFS"),
    )
    if _is_scoped_package_clean(command_arguments):
        _validate_package_clean_plan(command_arguments, root.resolve(strict=True), references)
        return None
    if not references:
        return None
    for reference in references:
        path = Path(reference).resolve()
        if path == target or path.is_relative_to(target):
            return path
    return None


def _run_child(command: list[str], root: Path) -> int:
    try:
        result = subprocess.run(command, cwd=root, check=False, shell=False)
    except OSError as error:
        print(f"unable to run locked command: {error}", file=sys.stderr)
        return 1
    return result.returncode


def run_locked_command(
    arguments: list[str],
    lock_path: Path = LOCK_PATH,
    root_producer_lock_path: Path = ROOT_PRODUCER_LOCK_PATH,
    root: Path = ROOT,
) -> int:
    if arguments and arguments[0] == PERFORMANCE_COVERAGE_MODE:
        if arguments != [PERFORMANCE_COVERAGE_MODE]:
            print(
                "usage: run_locked_command.py --performance-coverage",
                file=sys.stderr,
            )
            return 2
        root_producer_lock_path.parent.mkdir(parents=True, exist_ok=True)
        with _acquire_lock(root_producer_lock_path):
            return _run_child(PERFORMANCE_COVERAGE_COMMAND, root)

    protect_root_producer = bool(arguments) and arguments[0] == "--protect-root-producer"
    command_arguments = arguments[1:] if protect_root_producer else arguments
    if not command_arguments or command_arguments[0] != "--" or len(command_arguments) == 1:
        print(
            "usage: run_locked_command.py [--protect-root-producer] -- COMMAND [ARG ...]",
            file=sys.stderr,
        )
        return 2
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    with ExitStack() as locks:
        locks.enter_context(_acquire_lock(lock_path))
        if protect_root_producer:
            # root coverageと同じ非待機ロックで、生成中の成果物をcleanから保護する。
            root_producer_lock_path.parent.mkdir(parents=True, exist_ok=True)
            locks.enter_context(_acquire_lock(root_producer_lock_path))
            try:
                protected_reference = _protected_clean_reference(command_arguments, root)
            except (OSError, RuntimeError) as error:
                print(f"refusing protected cargo clean: {error}", file=sys.stderr)
                return 1
            if protected_reference is not None:
                print(
                    f"refusing protected cargo clean; reserved reference is inside target: {protected_reference}",
                    file=sys.stderr,
                )
                return 1
        return _run_child(command_arguments[1:], root)


def _main(arguments: list[str]) -> int:
    try:
        return run_locked_command(arguments)
    except _LockContentionError:
        print(LOCK_CONTENTION_MESSAGE, file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(_main(sys.argv[1:]))
