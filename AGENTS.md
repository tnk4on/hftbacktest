# HftBacktest独自拡張の開発規約

このファイルはtnk4on/hftbacktestの作業ブランチに適用する。codex-cloud側の規約とは独立して、dotが直接作業するときもCodexへ委任するときも開始前に確認する。Codexの自動読込だけにdot自身の判断を委ねない。

## Gitと承認

- masterは上流追従用として保護する。独自実装は目的別featureブランチで管理し、上流固定commit・独自実装commit・規約commitを区別する。
- 開始時にrepo/remote/branch/HEAD/statusと対象remote branchを確認し、未commit変更・既存履歴を保全する。別履歴をforce push/reset/cleanで上書きしない。
- execution通知拡張は `feature/execution-events` で管理する。他目的の変更を混在させない。codex-cloudの検証済み標準submoduleを無断更新しない。
- 本AGENTS.mdは継続規約、docs/対応するcodex-cloud文書は詳細契約・検証履歴、個別指示は今回の承認・予算・停止条件。規約を理由に本体変更・依存追加・試験・外部変更の範囲を拡大しない。

## 拡張の互換性と因果関係

- matching・queue・scheduler優先順・latency・feeの意味を無断変更しない。必要なら目的・影響・独立受入条件を別承認で確定する。
- execution通知はopt-inとし、標準modeの既存動作を保護する。Order型のlayout、公開API、serialization、generic境界の互換性を明示し、互換性未検証を成功と報告しない。
- execution ID、Order/response envelope、実Local dequeueの対応を保持する。E生成とR受信を混同せず、通知前executionをstrategy/riskへ先読みさせない。
- 自作ledger/riskとの境界はfallibleにし、記録・容量・ID・保存則エラーで停止する。native操作がrollback済みとは仮定せず、部分進行・未完了IDを記録する。
- 固定ソースの既知仕様は静的精査を優先する。試験は新しい拡張と自作consumerの結合・標準mode互換性に焦点を当て、個別承認の有限範囲を守る。
- source commitの保存とコンパイル/実行受入は別物。FAIL/BLOCKED/UNPROVEN/NOT RUNを明示し、失敗後の自動修正・再build・再試験・増枠は行わない。

自作strategy/order/risk/ledger、fixture・独立期待値・有限実行結果はtnk4on/codex-cloudで管理する。開始時にそちらの最新 `docs/PROJECT_STATUS_INDEX_JA.md` と対象の契約/結果を確認し、必要なsource/artifact SHAを個別作業記録へ固定する。詳細なrun固有SHA・予算を永続規約へ複製しない。
