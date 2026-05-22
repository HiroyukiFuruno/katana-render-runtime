# Runtime asset 管理

## 固定対象

| runtime | version | file | sha256 |
| --- | --- | --- | --- |
| Mermaid.js | 11.15.0 | `crates/katana-render-runtime/vendor/mermaid/11.15.0/mermaid.min.js` | `70137e77bb273bb2ef972b86e8b0400cca8be53cb25bfc45911a186dc98665de` |
| Mermaid ZenUML | 0.2.3 | `crates/katana-render-runtime/vendor/mermaid-zenuml/0.2.3/mermaid-zenuml.min.js` | `28eeec88021d9e9728df4d005ff723a3d71da29a21dbcfa2a628232c35ef2ab6` |
| Draw.io | 30.0.2 | `crates/katana-render-runtime/vendor/drawio/30.0.2/drawio.min.js` | `0435d7a829549490482d576a37556224fa190d538610c96908632e5cda7c601f` |
| MathJax | 4.1.2 | `crates/katana-render-runtime/vendor/mathjax/4.1.2/tex-svg.js` | `e201dba4a20191563337e7f95ebeef6724bd2fbdc079c431b4bb8ecdfc059c33` |
| ZenUML Core | 3.47.9 | `crates/katana-render-runtime/vendor/zenuml-core/3.47.9/zenuml.js` | `ece11a311907401113f965e110c25c04c6a9b3dcbbb234bf2cd593a3f3ebe3df` |
| PlantUML | 1.2026.4 | `crates/katana-render-runtime/vendor/plantuml/1.2026.4/plantuml.jar.sha256` | `1783d4569855f2f0a17e65bd192add377c7f2b5e3e1781b65dc94d084de98699` |

MathJax は公式配布の JavaScript asset を repository 上で checksum 管理し、実行に使う自己完結 bundle は
`crates/katana-render-runtime/src/markdown/diagram_runtime/generated/runtime-bundles.sha256`
で固定する。crates.io package には generated bundle を含め、公式 CDN asset は二重収録しない。

## 参照箇所

- `crates/katana-render-runtime/src/markdown/runtime_assets.rs`
- `crates/katana-render-runtime/src/renderer/runtime.rs`
- `crates/katana-render-runtime/src/renderer/output.rs`
- `Justfile`

既定の描画経路では、上記 asset を一時領域の
`katana-render-runtime/vendor/<runtime>/<version>/` へ展開して読み込む。
`MERMAID_JS` / `MERMAID_ZENUML_JS` / `DRAWIO_JS` / `MATHJAX_JS` / `ZENUML_CORE_JS`
を明示した場合だけ、指定 path を優先する。

## 確認

```bash
cd /Users/hiroyuki_furuno/works/private/katana-diagram-renderer
just runtime-asset-check
just runtime-asset-latest all
just mathjax-latest
```

`runtime-asset-latest` と `mathjax-latest` は current / latest / update_hint を表示するだけで、
repository 内の file は変更しない。

## 更新

```bash
cd /Users/hiroyuki_furuno/works/private/katana-diagram-renderer
just mermaid-update <version>
just drawio-update <version>
just mathjax-update <version>
```

更新 recipe は asset、`.sha256`、Rust 側の固定値を同じ流れで更新する。
MathJax は `@mathjax/src` の固定版と generated bundle も更新する。
Mermaid / Draw.io は reference snapshot と compare gate も更新対象に含める。
