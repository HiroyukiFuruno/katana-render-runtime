## Why

Draw.io 32.0.2 の公式JS増大により、v0.4.23のruntime crateは11,609,946 bytesとなり、配布上限10,485,760 bytesを超える。最新依存、オフライン描画、既存品質ゲートを維持してリリースするため、資産の配布単位を分ける。

## What Changes

- 同一workspaceに `katana-render-runtime-assets` を追加し、既存Draw.io圧縮JSをバイト単位で保持する。
- runtimeは資産crateの静的bytesを使用し、従来の展開、キャッシュ、checksum検査を維持する。
- 更新・パッケージ検証・公開順序・registry確認を3つの配布crateへ対応させる。
- 配布サイズ超過を公開前に検出する回帰を維持する。

## Capabilities

### New Capabilities

- `runtime-asset-package`: 自己完結する資産crateと、依存順の検証・公開。

### Modified Capabilities

なし。

## Impact

workspace manifest、runtime資産参照と生成導線、release検証・公開・契約テスト。既存公開runtime API、JSの内容、描画処理、性能受入条件は変更しない。Issue #89/#95 の公開完了に含める。
