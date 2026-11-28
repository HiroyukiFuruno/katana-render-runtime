## Why

HTMLは実Chromeと大きく異なり、Mermaidの公式比較基準も壊れていたため、従来の比較だけでは不具合を検出できなかった。Issue #106へ永続化・復元要求を統合し、#108と同じv0.4.24で独立基準による検証と公開を完了する。

## What Changes

- 実Google ChromeとKRRの独立PNGを同一viewport/scale/scrollで比較し、各画像のRMSE/MAE最小score99を必須にする。
- 私有資料を保存・公開せず、似た構成の架空内容fixtureを使用する。
- CSS origin、相対文字サイズ、inline/style保持、表・flex/grid・行高を修正する。
- host opt-inの静止frame artifact保存と、現sourceからのfresh Rust/V8 session再構築APIを追加する。
- Mermaidのnode変換を構造に沿って修正し、独立Chrome/vendor基準を検証する。

## Capabilities

### New Capabilities

- `html-frame-artifacts`: provenance、容量、原子書込、明示clear、fresh session再構築の契約。

### Modified Capabilities

- `html-dom-runtime`: runtimeの外部browser禁止を維持し、独立Chrome参照検証を明示的に許可して画像99と操作互換性を必須化する。

## Impact

Rust HTML CSS/layout/paintと公開artifact API、Mermaid JS bundle、Chrome比較scripts、架空fixture、Just/CI。製品runtimeへbrowserを導入せず、既存JNI/JVM方式・品質・coverageを維持する。元semantic/interaction/performance各95契約、Draft reviewから公開registry版検証までを維持する。
