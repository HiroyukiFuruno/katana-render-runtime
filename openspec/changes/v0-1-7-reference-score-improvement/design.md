## Context

v0.1.0 の release-check は通過しているが、local full compare では Draw.io official diagrams に 99 点未満が残っている。

確認済みの代表例:

- `i18n`: 94.32
- `link`: 97.38

v0.1.0 はまだ KatanA に取り込まないため release する。ただし、score が低いまま KatanA 側の検証を削ぐと品質低下を見逃すため、v0.1.7 で score 改善を完了してから KatanA 側への取り込み（consume）を判断する。

この範囲は旧 v0.1.1 / v0.1.2 として Jules 側へ渡していたが、粒度が大きく進捗が停滞している。v0.1.x では先に小さい後続フェーズを閉じ、score 改善（score improvement）は v0.1.7 の最終フェーズへ送る。

## Goals

- Draw.io の既知 score 未達 case を原因別に分類する
- 修正可能な差分は renderer / resource resolver / SVG postprocess 側で直す
- baseline は現在値を追認するためではなく、改善後の下限として更新する
- score report と contact sheet を Jules / PR review が確認できる形で残す

## Non-Goals

- Mermaid ZenUML の描画対応はしない
- unsupported fixture を暗黙 skip しない
- score 閾値を下げて合格扱いにしない
- fallback SVG や stub PNG で比較を通さない

## Score Policy

v0.1.7 は「最低点を 99 以上へ上げる最終フェーズ」である。

完了基準は明示的に `99` とする。実装者は compare command を省略形で実行せず、次のように `99` を渡して確認する。

- `just drawio-compare-ci 99`
- `just drawio-compare-full 99`
- `just mermaid-compare-ci 99`
- `just mermaid-compare-full 99`

対象は全 supported pattern である。代表比較（CI）は高速な確認用であり、完了判定は full compare で行う。

| 種別 | 範囲 | 実行手順 | 要求値 | 主な出力 |
| --- | --- | --- | --- | --- |
| Draw.io | CI 代表 | `just drawio-compare-ci 99` | 全 case 99 以上 | `tmp/kcf-drawio-ci/comparison` |
| Draw.io | full 全対象 | `just drawio-compare-full 99` | 全 supported case 99 以上 | `tmp/kcf-drawio-full/*/comparison` |
| Mermaid | CI 代表 | `just mermaid-compare-ci 99` | 全 case 99 以上 | `tmp/kcf-mermaid-ci/comparison` |
| Mermaid | full 全対象 | `just mermaid-compare-full 99` | 全 supported case 99 以上 | `tmp/kcf-mermaid-full/*/comparison` |

Draw.io full は `Justfile` の `drawio-compare-full` に列挙された `basic`、`official/diagrams`、`official/examples`、`official/blog`、`official/templates/*` を対象にする。

Mermaid full は `tests/fixtures/mermaid/en` と `tests/fixtures/mermaid/ja` を対象にする。

supported fixture は、代表比較（CI）と full compare のどちらでも score 99 未満を残してはならない。99 未満が残る場合、その case は完了扱いにしない。`tests/fixtures/drawio/representative/score-baseline.json` と `scripts/mermaid/reference_score_floors.ts` に 99 未満の許容値が残る場合も、supported fixture の完了扱いにしない。

unsupported fixture は暗黙 skip しない。Mermaid ZenUML / unsupported fixture handling は v0.1.2 で固定し、v0.1.7 はその metadata を前提に supported pattern の score 改善へ集中する。

まず `drawio-compare-ci` と full compare の early-fail 箇所を確認し、score 未達の原因を次に分類する。

- runtime / resource 解決漏れ
- SVG postprocess の差分
- browser rasterize の差分
- 公式 reference 側の特殊ケース
- 現時点では修正しない既知差分

修正しない既知差分は、理由と現在 score を report に残す。ただし、その case が supported fixture である限り、99 未満のまま v0.1.4 完了にはしない。どうしても v0.1.7 で直せない場合は、ユーザー確認を受けて別 change へ明示的に送る。

## Jules 作業プロトコル

Jules は一度に全 fixture を直そうとしない。必ず次の小さい cycle を繰り返す。

1. `just drawio-compare-ci 99` を実行し、最初に失敗した fixture 名、score、出力先 `tmp/kcf-drawio-ci/comparison` を記録する
2. 失敗 fixture を含む最小 fixture directory だけを `just drawio-compare <fixture-dir> 99 tmp/kcf-v0.1.7-reference-score-improvement/<case>` で再実行する
3. `tmp/kcf-v0.1.7-reference-score-improvement/<case>/comparison` の official PNG、kcf PNG、diff / report を見て差分を一種類だけ分類する
4. 分類に対応する最小ファイルだけを修正する
5. 同じ case を再実行し、score が上がったことを確認する
6. score が 99 以上になったら次の case に進む
7. 失敗が full compare でだけ出る場合は、該当 fixture を含む最小 fixture directory だけを `just drawio-compare <fixture-dir> 99 tmp/kcf-v0.1.7-reference-score-improvement/full-<slug>` で切り出す

Jules は原因が分からない場合、推測で broad refactor をしない。report に「見えている差分」「触ったファイル」「次に疑う場所」を書いて cycle を止める。

## Verification

- `just drawio-compare-ci 99`
- `just drawio-compare-full 99`
- `just mermaid-compare-ci 99`
- `just mermaid-compare-full 99`
- `just check`
- `just VERSION=v0.1.7 release-check`
- `npx -y @fission-ai/openspec validate "v0-1-7-reference-score-improvement" --strict`

Mermaid ZenUML 由来の failure も supported fixture の score failure として扱い、v0.1.7 の score 改善完了条件に含める。
