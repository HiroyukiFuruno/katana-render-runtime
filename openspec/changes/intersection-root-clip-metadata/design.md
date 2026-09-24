## Context

現在のlayoutはoverflow clipをSVG出力にだけ使い、runtime bridgeはnodeごとの矩形だけを送る。containing blockにもowner nodeがないため、JS runtimeはElement rootのDOM descendant判定以外でlayout所属を判断できない。

## Goals / Non-Goals

**Goals:**

- 各targetのpositioning containing blockとancestor clip列をlayoutからruntimeへ伝える。
- Element root observerがroot外containing blockのfixed/absolute targetを除外する。
- 祖先clipを交差矩形とratioに反映する。
- metadataの欠損・不整合は交差なしとしてfail closedにする。

**Non-Goals:**

- CSS layout engine全般の再設計。
- `overflow: visible` の矩形をclipとして扱うこと。
- 既存のviewport rootまたはdocument rootの交差意味の変更。

## Decisions

- `ContainingBlock` はowner node idまたはviewport sentinelを保持する。座標だけではroot内外を判別できないためである。
- `ElementBox` はtarget自身のpositioning blockと、overflow clipを発生させるancestor node/rect列を保持する。描画用clip pathから逆算しない。
- Geometry bridgeはmetadataを同じlayout snapshotで送る。JSはevent pathとmetadataのnode idを照合し、Element root外のcontaining blockとclip ancestorを適用する。
- metadataが未知ならElement root observerはtargetを非交差にする。欠損をviewport扱いにするfallbackはroot外targetを誤受理する。

## Risks / Trade-offs

- [Bridge payload増加] → node idと矩形だけを送り、clip列をancestor単位でdeduplicateする。
- [layoutのnode owner伝播漏れ] → fixed/absolute/root内controlを実hostで回帰し、metadata欠損をfail closedにする。
- [clipの二重適用] → JSがmetadata由来clipだけを適用し、SVG clip pathを参照しない。

## Migration Plan

1. metadataをlayoutとbridgeへ追加し、既存geometry schemaを拡張する。
2. JS observerにroot所属・clip交差を追加する。
3. 実host回帰と既存IntersectionObserver契約を通す。
4. 不一致時はruntimeをfail closedにし、ロールバックはmetadata利用前のrevisionへ戻す。

## Open Questions

- clip列のdeduplicateをlayout側かbridge側のどちらで行うかはpayload計測で決める。
