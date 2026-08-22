# task-02 最小サーバー構築とチェスロジック統合

## ゴールと完了条件
- Axumで `/health` が応答する
- `shakmaty` を組み込み、対局の作成・盤面取得・指し手適用ができる
- 完了条件: `POST /games` → `POST /games/:id/move` → `GET /games/:id` がcurlで一通り通ること

## 実装したエンドポイント
| メソッド | パス | 説明 |
|---|---|---|
| GET | `/health` | 疎通確認 |
| POST | `/games` | 新規対局作成、対局IDと初期FENを返す |
| GET | `/games/:id` | 盤面・チェック状態・終局判定 |
| POST | `/games/:id/move` | UCI形式の指し手を適用 |

## 設計判断の根拠
- **対局状態は `Arc<RwLock<HashMap<Uuid, Chess>>>` でメモリ管理**: この段階ではDBがない。進行中の局面は揮発してよいという整理(後の task-03 でも、進行中はメモリ・確定情報のみDBという方針として引き継がれる)
- **エラーハンドリングを `.expect()` から `?` 演算子に変更**: `main() -> Result<(), Box<dyn Error>>` にすることで、起動失敗時にパニックではなくエラーメッセージで終了する

## 指し手判定の流れ
1. UCI文字列をパース → 形式不正なら400
2. 対局IDから局面を取得 → 存在しなければ404
3. `UciMove::to_move` でその局面における合法性を検証 → 不正なら400
4. `Position::play` で局面を更新

## つまずいた点と教訓
| 症状 | 原因 | 対応 |
|---|---|---|
| `uuid` 最新版がビルドできない | サンドボックス環境のRustが1.75と古く、依存の `getrandom v0.4` が `edition2024` を要求 | `uuid = "=1.10.0"` に固定(ローカルMacはRust 1.96のため本来は不要) |
| `is_check()` / `is_game_over()` が見つからない | `shakmaty::Position` トレイトのメソッドであり `use` し忘れていた | トレイトを `use` に追加 |
| `Position::play` で型エラー | `&Move`(参照)を取る仕様のところに値を渡していた | 参照渡しに修正 |

- **教訓**: Rustのトレイトメソッドは「型を持っていれば呼べる」わけではなく、トレイトをスコープに入れる必要がある。`method not found` が出たらまずトレイトの `use` を疑う

## 次タスクへの引き継ぎ
- この時点では認証がなく誰でも対局を操作できる。JWT必須化は task-05・task-06 で行う
- `position_to_fen` ヘルパーはこの段階で作成。後のタスクで定義が消失する事故が起きるので存在を意識しておく

## 再現コマンド
```bash
cargo run
curl http://localhost:3000/health
curl -X POST http://localhost:3000/games
curl -X POST http://localhost:3000/games/<game_id>/move \
  -H "Content-Type: application/json" -d '{"uci":"e2e4"}'
curl http://localhost:3000/games/<game_id>
```