## ADDED Requirements

### Requirement: HTML frame artifactはhost所有の安全な永続化契約を持つ

KRR SHALL provide opt-in host-owned storage of static frames. Artifacts MUST validate format/sourceSHA/origin/runtime version/config/resource/font fingerprints/viewport/scale and frame buffer dimensions. Raw HTML、DOM、JS heap、form値を別fieldとして保存してはならない。pixelsとoriginの機密性、rootと削除のhost所有を明記しなければならない。

#### Scenario: 独立プロセスで保存済みframeを再利用する
- **WHEN** hostが保存を有効化し、別processで同じprovenanceを要求する
- **THEN** frameはhitし、source/resource/font/version/viewport等の変更はmissになる

### Requirement: 保存は容量制限・原子更新・明示エラーを維持する

Store MUST enforce total quota and bounded reading, reject invalid keys/symlink paths, and atomically replace complete files through temporary write/sync/rename. 失敗時は旧artifactを保持し、破損やunsupported formatを明示エラーで返さなければならない。writer競合とクラッシュ残留lockはhostが判別できなければならない。

#### Scenario: 更新と破損・容量不足を扱う
- **WHEN** 同じkeyの更新、過大artifact、破損JSON、競合writerを処理する
- **THEN** 正常更新は完全frameへ置換され、失敗は旧frameを壊さずtyped errorを返す

### Requirement: 操作session復元は現sourceからfresh runtimeを構築する

Resume MUST verify current runtime and source provenance, open a fresh Rust/V8 runtime and restore scroll. 静止artifactをJS状態snapshotと偽ってはならない。close時owned-memory解放と復元後scroll/link/form/JSの操作を検証しなければならない。

#### Scenario: 再起動後も操作を継続する
- **WHEN** 独立processでartifactからsessionを再構築する
- **THEN** 現sourceのJSが実行され、フォーム入力、JS click、navigation、scroll、closeが動作する
