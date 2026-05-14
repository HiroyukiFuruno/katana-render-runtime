# Runtime asset 管理

## 固定対象

| runtime | version | file | sha256 |
| --- | --- | --- | --- |
| Mermaid.js | 3.3.1 | `crates/katana-diagram-renderer/vendor/mermaid/3.3.1/mermaid.min.js` | `217b66ef4279c33c141b4afe22effad10a91c02558dc70917be2c0981e78ed87` |
| Draw.io | 29.7.10 | `crates/katana-diagram-renderer/vendor/drawio/29.7.10/drawio.min.js` | `a8b7897de995a4e7dd3a541a5e7250d64a295440f728f0ddae72179cdf5a83d5` |

Draw.io `29.7.10` は既存の local runtime から固定している。GitHub release
には `v29.7.10` が存在しないため、runtime 未検出時の案内 URL は release 一覧にする。

## 参照箇所

- `crates/katana-diagram-renderer/src/markdown/runtime_assets.rs`
- `crates/katana-diagram-renderer/src/renderer/runtime.rs`
- `crates/katana-diagram-renderer/src/renderer/output.rs`
- `Justfile`

既定の描画経路では、上記 asset を一時領域の
`katana-diagram-renderer/vendor/<runtime>/<version>/` へ展開して読み込む。
`MERMAID_JS` / `DRAWIO_JS` を明示した場合だけ、指定 path を優先する。

## 確認

```bash
cd /Users/hiroyuki_furuno/works/private/katana-diagram-renderer
just runtime-asset-check
just runtime-asset-latest all
```

`runtime-asset-latest` は current / latest / update_hint を表示するだけで、
repository 内の file は変更しない。

## 更新

```bash
cd /Users/hiroyuki_furuno/works/private/katana-diagram-renderer
just mermaid-update <version>
just drawio-update <version>
```

更新 recipe は asset、`.sha256`、Rust 側の固定値、reference snapshot、
local full compare、CI/CD representative compare を同じ流れで実行する。
