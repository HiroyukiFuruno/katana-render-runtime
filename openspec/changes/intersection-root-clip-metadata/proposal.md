## Why

HTML runtime の `IntersectionObserver` はDOM descendantだけでElement rootへの所属を判断している。fixed/absolute のcontaining blockがroot外にあるケースと、祖先の `overflow` clipがあるケースでは、ブラウザと異なる交差状態を返す。

## What Changes

- Layoutからruntime bridgeへ、各要素のpositioning contextとancestor clip矩形を渡す。
- Element rootのobserver計算で、containing blockがroot外のtargetを非交差として扱う。
- observer交差矩形をclip ancestorごとに切り詰める。
- 実host回帰でfixed/absolute、root内control、ancestor clipを固定する。

## Capabilities

### New Capabilities

- `html-runtime-intersection-observer-parity`: HTML runtimeのIntersectionObserverがlayout所属とclipを反映する契約。

### Modified Capabilities

なし。

## Impact

`html_interactive` layout metadata、HTML runtime geometry bridge、`dom_bootstrap.js`、実host回帰テストに影響する。公開APIと依存関係は変更しない。
