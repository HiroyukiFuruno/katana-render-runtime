"""完了した既知incremental sessionの兄弟object閉包をTMPで検証する。"""
from __future__ import annotations

from collections import defaultdict
from collections.abc import Collection
from pathlib import Path
import os
import tempfile
import unittest
from cargo_capacity_guard_lock import INCREMENTAL_INDEX_NAMES, _incremental_index_closure, evict_artifacts

Candidate = tuple[Path, Path, os.stat_result]
Inode = tuple[int, int]


class SiblingClosureTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.profile = self.root / 'debug'
        self.session = self.profile / 'incremental/katana_render_runtime-abc/s-abc-def-ghi'
        self.session.mkdir(parents=True)
        self.rows: list[Candidate] = []
        for name in sorted(INCREMENTAL_INDEX_NAMES) + ['first.o', 'sibling.o']:
            p = self.session / name
            p.write_bytes(b'x' * 4096)
            self.rows.append((self.profile, p, p.stat()))

    def closure(self, protected: Collection[Inode] = (), omit_index: bool = False) -> dict[Inode, list[Candidate]] | None:
        aliases: defaultdict[Inode, list[Candidate]] = defaultdict(list)
        sessions: dict[Path, Path] = {}
        members: defaultdict[Path, list[Candidate]] = defaultdict(list)
        indexes: defaultdict[Path, dict[str, Candidate]] = defaultdict(dict)
        for row in self.rows:
            value = row[2]
            aliases[(value.st_dev, value.st_ino)].append(row)
            sessions[row[1]] = self.session
            members[self.session].append(row)
            if row[1].name in INCREMENTAL_INDEX_NAMES:
                indexes[self.session][row[1].name] = row
        if omit_index:
            indexes[self.session].pop('work-products.bin')
        first = next(row for row in self.rows if row[1].name == 'first.o')
        return _incremental_index_closure([first], aliases, sessions, indexes, members, set(protected))

    def test_first_object_closes_all_siblings_before_any_index_is_invalidated(self) -> None:
        closure = self.closure()
        if closure is None:
            raise AssertionError("completed sibling closure must be admitted")
        self.assertEqual({row[1].name for rows in closure.values() for row in rows},
                         INCREMENTAL_INDEX_NAMES | {'first.o', 'sibling.o'})
        self.assertEqual(len(closure), 5)

    def test_protected_sibling_rejects_whole_session(self) -> None:
        sibling = next(row for row in self.rows if row[1].name == 'sibling.o')
        self.assertIsNone(self.closure({(sibling[2].st_dev, sibling[2].st_ino)}))

    def test_unknown_alias_of_sibling_rejects_whole_session(self) -> None:
        sibling = next(row for row in self.rows if row[1].name == 'sibling.o')
        os.link(sibling[1], self.root / 'outside-alias')
        self.rows[self.rows.index(sibling)] = (*sibling[:2], sibling[1].stat())
        self.assertIsNone(self.closure())

    def test_missing_index_stays_unadmitted_without_trusted_ownership(self) -> None:
        self.assertIsNone(self.closure(omit_index=True))

    def test_real_selector_admits_entire_completed_sibling_closure_in_dry_run(self) -> None:
        (self.profile / '.fingerprint').mkdir()
        protected = self.profile / 'current-app'
        protected.write_bytes(b'protected')
        current = sum(path.stat().st_blocks * 512 for path in self.root.rglob('*') if path.is_file())
        code, report = evict_artifacts(self.root, current - 1, current, True,
                                      protected_refs=[protected],
                                      incremental_prefixes=['katana_render_runtime-'])
        self.assertEqual(code, 0)
        self.assertEqual({Path(path).name for path in report['candidate_paths']},
                         INCREMENTAL_INDEX_NAMES | {'first.o', 'sibling.o'})
        self.assertEqual(report['estimated_candidate_bytes'], sum(row[2].st_blocks * 512 for row in self.rows))
        self.assertTrue(all(row[1].exists() for row in self.rows))
        self.assertEqual(protected.read_bytes(), b'protected')

if __name__ == '__main__':
    unittest.main()
