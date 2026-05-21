# katana-diagram-renderer — UI 分離計画 抜粋

作成日: 2026-05-17  
canonical: [`katana/docs/architecture/ui-separation/detailed-design-and-tasks.md`](../../katana/docs/architecture/ui-separation/detailed-design-and-tasks.md)

## このファイルの位置付け

本ファイルは KatanA ecosystem の **UI 分離構想 master** から `katana-diagram-renderer` (KDR) 担当部分を抜粋したもの。task ID は master と同一。**master が単一情報源**。

## Repository の役割

`katana-diagram-renderer` (KDR) は **diagram rendering canonical** として位置付ける。

- 純粋な diagram rendering (Mermaid / Draw.io / ZenUML / PlantUML) のみを所有する。
- document export と viewer ownership は deliberately exclude (現状方針を維持)。
- KCF (`katana-canvas-forge`) と責務が近いが、KDR は diagram に閉じ、export / artifact build は KDV forge に集約する。
- 長期的には KCF が KDR を呼ぶ形に整理 (KCF が transitional layer に縮小)。

詳細: master [`1.6 katana-diagram-renderer`](../../katana/docs/architecture/ui-separation/detailed-design-and-tasks.md#16-katana-diagram-renderer)

## 担当 Phase

- **Phase 7-A**: Responsibility split で KDR を canonical として ADR 記録 (本 repo のメイン作業)
- Phase 2 / 7-B (KDV / KCF 側): KDR は **触らない**

## Task list (master 抜粋)

### P7-A. Responsibility split (KDR 側で完結する部分)

- [ ] P7-A-003: KDR rendering responsibilities を一覧化する。
- [ ] P7-A-005: KDR を diagram rendering canonical として ADR `docs/adr/kdr-canonical-diagram-renderer.md` に記録する。

### KDR README / 方針更新

- [ ] P8-A-007: `katana-diagram-renderer` README に pure renderer 方針を追加する (export / viewer ownership は持たない明示)。

## 前提 (depends on) / 出力 (provides)

- **前提**:
  - P0 governance (各 repo の責務表が README に反映されていること)
  - P7-A-001 / 002 (KCF rendering / export responsibilities 一覧化が他 repo 側で完了)

- **出力**:
  - KDR を canonical diagram renderer とする ADR
  - pure renderer 方針 README
  - KCF と KDR の duplicated renderer types 解消の判断材料

## Done criteria

本 repo に関する master 9 章 Done criteria のうち、該当項目:

- [ ] KDR が diagram rendering の canonical owner として ADR に記録されている
- [ ] KDR README が pure renderer 方針を明示している
- [ ] KDR は export / viewer ownership を持たない (現状方針維持)

## 重要な非該当事項

本 repo は今回の UI 分離構想で **大きな機能変更を行わない**。主な作業は ADR 記録と README 方針追加のみ。KCF からの責務移管は KCF 側 (Phase 7 KCF 抜粋) と KDV 側 (Phase 2 / 7 KDV 抜粋) で進める。

## drift 検出

- 本ファイルの task ID は master と完全一致する。
- P8-A-001 の CI script が master と本ファイルの task ID 一致を検査する。

## 参照リンク

- [master detailed-design-and-tasks.md](../../katana/docs/architecture/ui-separation/detailed-design-and-tasks.md)
- [master principles.md](../../katana/docs/architecture/ui-separation/principles.md)
- [overview README](../../katana/docs/architecture/ui-separation/README.md)
- [KCF repo の Phase 7 抜粋](../../katana-canvas-forge/docs/ui-separation-plan.md)
- [KDV repo の Phase 2 / 7 抜粋](../../katana-document-viewer/docs/ui-separation-plan.md)
- [既存 docs/release.md](release.md)
- [既存 docs/runtime-assets.md](runtime-assets.md)
