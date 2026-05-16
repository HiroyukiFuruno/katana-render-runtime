## Context

v0.1.0 の release-check は通過しているが、local full compare では Draw.io official diagrams に 99 点未満が残っている。

確認済みの代表例:

- `i18n`: 94.32
- `link`: 97.38

v0.1.0 はまだ KatanA に取り込まないため release する。ただし、score が低いまま KatanA 側の検証を削ぐと品質低下を見逃すため、v0.1.1 で score 改善を完了してから KatanA 側への取り込み（consume）を判断する。

この範囲は旧計画で複数フェーズに分かれていたが、粒度が大きく進捗が停滞している。初回公開を `v0.1.0` へ戻したため、score 改善（score improvement）は直後の `v0.1.1` で扱う。

## Goals

- Draw.io の既知 score 未達 case を原因別に分類する
- 修正可能な差分は renderer / resource resolver / SVG postprocess 側で直す
- baseline は現在値を追認するためではなく、改善後の下限として更新する
- score report と contact sheet を Jules / PR review が確認できる形で残す

## Non-Goals

- Mermaid ZenUML の新規文法対応や機能拡張はしない
- unsupported fixture を暗黙 skip しない
- score 閾値を下げて合格扱いにしない
- fallback SVG や stub PNG で比較を通さない

## Score Policy

v0.1.1 は「最低点を 95 より上へ上げ、公式をなるべく忠実に踏襲する release」である。

当初は 99 点以上を完了基準にしていたが、実装後の全量評価で 95 点以上なら主要な図形、文字、接続線、全体構図が機能する状態であることを確認した。
そのため v0.1.1 は 95 点以下を 0 件にした状態で release し、99 点以上は「公式を完全踏襲」と言える後続目標に分ける。

対象は全 supported pattern である。代表比較（CI）は高速な確認用であり、release 判定は full compare の全量評価で行う。

| 種別 | 範囲 | 実行手順 | 要求値 | 主な出力 |
| --- | --- | --- | --- | --- |
| Draw.io | full 全対象 | diagnostic compare | 95 点以下 0 件 | `tmp/kdr-drawio-full-after-wireframe-fix/*/comparison` |
| Mermaid | full 全対象 | diagnostic compare | 95 点以下 0 件 | `tmp/kdr-mermaid-full/*/comparison` |
| Draw.io / Mermaid | 95〜99 点未満 | 視認評価 | release 停止級なし | `docs/releases/v0.1.1-reference-score-evaluation.md` |

Draw.io full は `Justfile` の `drawio-compare-full` に列挙された `basic`、`official/diagrams`、`official/examples`、`official/blog`、`official/templates/*` を対象にする。

Mermaid full は `tests/fixtures/mermaid/en` と `tests/fixtures/mermaid/ja` を対象にする。

supported fixture は、full compare で score 95 以下を残してはならない。95 以下が残る場合、その case は release 対象にしない。
99 未満が残る場合は、公式完全踏襲へ向けた後続 task として正式 report に残す。

unsupported fixture は暗黙 skip しない。v0.1.1 は supported pattern の score 改善へ集中する。

まず `drawio-compare-ci` と full compare の early-fail 箇所を確認し、score 未達の原因を次に分類する。

- runtime / resource 解決漏れ
- SVG postprocess の差分
- browser rasterize の差分
- 公式 reference 側の特殊ケース
- 現時点では修正しない既知差分

修正しない既知差分は、理由と現在 score を report に残す。ただし、その case が supported fixture である限り、95 以下のまま v0.1.1 release 対象にはしない。99 未満の case は、ユーザー確認を受けて別 change へ明示的に送る。

## Jules 作業プロトコル

Jules は一度に全 fixture を直そうとしない。必ず次の小さい cycle を繰り返す。

1. `just drawio-compare-ci 95` を実行し、最初に失敗した fixture 名、score、出力先 `tmp/kdr-drawio-ci/comparison` を記録する
2. 失敗 fixture を含む最小 fixture directory だけを `just drawio-compare <fixture-dir> 95 tmp/kdr-v0.1.1-reference-score-improvement/<case>` で再実行する
3. `tmp/kdr-v0.1.1-reference-score-improvement/<case>/comparison` の official PNG、kdr PNG、diff / report を見て差分を一種類だけ分類する
4. 分類に対応する最小ファイルだけを修正する
5. 同じ case を再実行し、score が上がったことを確認する
6. score が 95 より上になったら release 下限は完了とし、99 未満は report に残す
7. 失敗が full compare でだけ出る場合は、該当 fixture を含む最小 fixture directory だけを `just drawio-compare <fixture-dir> 95 tmp/kdr-v0.1.1-reference-score-improvement/full-<slug>` で切り出す

Jules は原因が分からない場合、推測で broad refactor をしない。report に「見えている差分」「触ったファイル」「次に疑う場所」を書いて cycle を止める。

## Verification

- Draw.io full 再評価で 95 点以下 0 件
- Mermaid full 再評価で 95 点以下 0 件
- `docs/releases/v0.1.1-reference-score-evaluation.md` に 95〜99 点未満の判断を記録
- `just check`
- `just VERSION=v0.1.1 release-check`
- `npx -y @fission-ai/openspec validate "v0-1-1-reference-score-improvement" --strict`

Mermaid ZenUML 由来の failure も supported fixture の score failure として扱い、v0.1.1 の score 改善完了条件に含める。
