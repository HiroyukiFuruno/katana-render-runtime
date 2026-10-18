# build成果物の容量整理

Justfileのbuildは容量guardを通す。最初に成功したbuild後の実target容量を基準として保存し、その約2倍を超えたときだけ、guardが所有権を確認できる有限集合の成果物を整理する。毎回の全削除は行わない。失敗buildやcheck/testの結果を容量基準として採用しない。

容量metricは `unique-inode-allocated-bytes` version 1である。regular fileごとの論理サイズではなく、同一inodeを一度だけ数え、filesystemのallocated block (`st_blocks * 512`) を合計するため、hardlinkの二重計上とsparse fileの過大計上を避ける。stateにはmetricとmetric_versionを必須保存し、旧logical-size stateは拒否する。`st_blocks`を提供しない環境ではmetric unavailableとしてcleanupをskipし、baselineを記録しない。基準の校正はgarbageを含む既存targetから行わず、fresh canonical buildの成功後だけ行う。

標準の容量整理は `scripts/build/cargo_capacity_guard.py` と `cargo_capacity_guard_lock.py` が実行する。`just build-capacity-check` で独立projectを使った容量・所有権・lock・中断時の回帰を実行する。手動の `sweep` / `clean` レシピは、このguardの復旧手順ではない。

基準は無視対象の `tmp/.cargo-capacity-guard.json` に保存する。`CARGO_CAPACITY_FACTOR` で倍率を指定できるが、有限値かつ1より大きい値に限る。初回build時点のtarget全体を測るため、増分build一回ごとの増加量を示す値ではない。

v0.4.24の今回の検証では、既に固定した基準2,363,047,936 bytesと倍率2、容量閾値4,726,095,872 bytesを維持する。追加のビルドを通すための再校正や閾値引き上げは行わず、保護された参照と使用中の成果物を整理候補から守る。

対象はCargo metadataと一致する、このrepo内のtargetだけである。保護参照、使用中process、Cargo lock、対象の物理identityと入力を照合する。使用中または検査不能なら整理せず、容量超過が残る状態で子Cargoを起動しない。Windowsのprocess検査不能も整理成功として扱わない。symlinkや対象不一致などの不正な設定は操作を拒否する。容量判定、整理結果、skip理由は標準エラーへ記録し、Cargo JSONの標準出力は保持する。

所有権を確認できる候補がない場合や、保護対象・未知のaliasを保持した結果として容量が下がらない場合は、未達として記録する。稼働中の他repoやtarget全体の強制削除へ切り替えない。

整理後のallocated容量を再計測し、閾値以下になったことを確認する。`--require-capacity` の事前検査では、metric・baseline・状態identity・整理結果も受入条件になる。過去の成功、退避しただけの容量、削除予定量を実際の解放量に代用しない。

fingerprintの無効化後に整理が中断されると、`.capacity-invalidated-*` が残る。guardは `interrupted fingerprint invalidation requires bounded recovery; artifacts retained` と拒否し、通常の再試行では削除を進めない。旧fingerprintを戻さず、既存の復旧手順がなければ対象・identity・保護参照・具体的な操作を整理して明示承認を得る。領域名の変更による検知回避や容量閾値の変更で通過させない。

管理済みCLIの入力manifestは `rtk proxy bun scripts/mermaid/native-source-manifest.ts --root "$PWD"` で実行ごとに新しく取得する。出力JSONはその実行の無視対象の証跡ディレクトリに保存する。Git管理対象と無視されていない未追跡ファイルだけを列挙し、既存のsource verifierで内容と集合を照合する。過去manifestのコピーや、`tmp`・`target`を再帰走査する収集処理で代用しない。この取得だけでは、その後のbuild・画像比較・性能検査の成功を示さない。

保護refを子プロセスへ渡す場合は `just protected-capacity-admission <registry>` を使い、既存 `capacity_admission` が固定state、registry schema、全refの物理identityを検証したJSONの `environmentValue` をそのまま `CARGO_CAPACITY_PROTECTED_REFS` に渡す。`mode` やstampを手作業で再構成しない。この出力は事前admissionであり、buildやcleanupの成功を示さない。
