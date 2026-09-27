# task-44 全体監査と整合性改善

## 目的

最新のコード、設定、過去のタスク記録を横断して、現在も残っている課題と、実装済みなのに古い docs だけに残っている引き継ぎを分離する。監査で見つかった問題のうち、仕様変更を伴わず効果が高いものは同じタスクで修正する。

## 監査時点

- 日付: 2026-09-10
- GitHub `main`: `ca5f49b`（PR #51 のマージ）
- ローカルの元コミット: `8be9d3b`
- GitHub の比較結果: ファイル差分なし（マージコミットだけが先行）
- Open PR / Issue: 0件
- PR #51 の CI run #85: success

過去の `Task_XX` にある「次タスクへの引き継ぎ」は当時のスナップショットであり、現在の残タスク一覧としては扱わない。現在の優先順位は README の Future Work を正本とする。

## 監査で見つかり、修正した項目

| 重要度 | 問題 | 対応 |
|---|---|---|
| 高 | 指し手の INSERT と終局 UPDATE の失敗をログだけに残し、HTTP 200 と WebSocket イベントを返し得た | 同一トランザクションで確定し、コミット後にだけメモリと WebSocket を更新 |
| 高 | サーバー再起動後にメモリ上の局面が消えると、進行中の対局で次の手を指せなかった | `moves.fen_after` の最新値から局面を復元してキャッシュへ戻す |
| 中 | `move_number` に shakmaty の fullmove 値を使い、黒の手と次の白の手が同じ番号になった | 保存値を ply の連番にし、取得順は主キー `id` を正本として `row_number()` で返す |
| 中 | ユーザー登録時のすべての DB エラーを「ユーザー名重複」409として返していた | unique violation だけ409、それ以外は内部エラー500に分離 |
| 中 | Docker はビルドできても起動できない回帰を CI が検出できなかった | Docker `HEALTHCHECK` と Compose のDB待機を追加し、CIで起動・マイグレーション・`/health`まで検証 |
| 中 | `.env.example` のPostgreSQLポートが Compose と不一致で、`SWEEP_TOKEN` も欠落していた | 5434へ統一し、Composeへ必要な開発用設定を明示的に渡す |
| 低 | 対局画面と棋譜再生画面がUCI→SAN変換を別々に実装していた | `lib/uciToSan.ts` に一本化し、不正データ時のフォールバックも統一 |
| 低 | 未使用のVite初期CSS、試作チェス駒、空コンポーネントが残っていた | 参照がないことを確認して削除 |
| 低 | 初期生成後に更新されていない `docs/openapi.json` が、実際の動的OpenAPI仕様と重複・乖離していた | 静的スナップショットを削除し、`/openapi.json` と生成元テストに一本化 |
| 低 | `.gitignore` に重複行と誤記があった | パターンを整理し、Docker build contextから `.env` を除外 |

## 追加した回帰テスト

- 3 ply の `move_number` が `1, 2, 3` になる
- 対戦相手の参加前は作成者も指せず、棋譜が保存されない
- 対戦相手の参加前は投了できず、待機中の対局状態が維持される
- メモリキャッシュ消失後も棋譜の最終FENから復元して次の手を指せる
- チェックメイト後に局面キャッシュを削除し、追加の指し手を409で拒否する
- ユーザー登録の非重複DB障害を409ではなく500に分類する

## 残課題

### P1: 次に実施

1. 認証エンドポイントのレート制限と、存在しないユーザーでも同程度のArgon2処理を行うタイミング差対策
2. Vitest + Testing Library と Playwright によるフロントエンドの自動テスト
3. WebSocket再接続時のスナップショット同期（切断中のイベント欠落をRESTで補完）
4. 終局更新後にレーティング適用だけ失敗した場合の再試行、または終局処理との同一トランザクション化
5. Renderのデプロイ完了・失敗の検知と通知。Deploy Hookの受付成功だけでは本番反映を保証しない
6. GitHub Ruleset の必須チェックに `Container smoke test` を追加

### P2: 規模や利用者が増える前に実施

1. JWTのHttpOnly Cookie化とCSP導入。現在のlocalStorage方式ではXSS時にトークンを読み取られる
2. 単一プロセス前提の局面キャッシュを見直し、複数インスタンス対応時はDBロックまたは外部ストアで直列化
3. MFA（TOTP）
4. 初期対局のK値を大きくする暫定レーティング
5. CodeQL / dependency review / コンテナイメージ監査

### P3: UX・可観測性

1. レーティング推移グラフ
2. Problem Details の `instance` とリクエストID
3. 履歴APIの総件数または次ページカーソル

## 検証結果

- `cargo fmt --all`: 成功
- `cargo clippy --all-targets -- -D warnings`: 成功（警告0件）
- `cargo test --all-targets`: 164件成功（ユニット63 / 統合101）
- `npm run lint`: 成功
- `npm run build`: 成功（TypeScript型チェックとVite本番ビルド）
- `cargo audit`: 既知の脆弱性0件。許容済み警告は間接依存の `paste 1.0.15` に対する unmaintained advisory のみ
- `npm audit`: 脆弱性0件
- `docker compose config --quiet`: 成功
- `docker compose up --build --wait`: PostgreSQL / API ともに healthy
- `curl --fail --silent http://localhost:3000/health`: `{"status":"ok"}`
- `git diff --check`: 成功
- tracked files の秘密情報パターン検査: 検出0件
