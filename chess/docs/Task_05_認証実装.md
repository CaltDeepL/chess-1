# task-05 認証実装(register / login / JWT)

## ゴールと完了条件
- `POST /auth/register` と `POST /auth/login` が動作する
- JWTを発行し、`Authorization: Bearer <token>` から user_id を取り出せる
- 完了条件: 登録→ログイン→トークン取得がcurlで通り、パスワードがハッシュ化されてDBに入っていること

## 設計判断の根拠
| 判断 | 根拠 |
|---|---|
| パスワードは Argon2 でハッシュ化(ランダムソルト) | 現在推奨されるパスワードハッシュ方式。bcryptより新しくメモリハード |
| ユーザー名重複は 409 Conflict | 400ではなく409にすることで、クライアント側が「重複」だけを専用メッセージにできる |
| ログイン失敗は「ユーザー不在」と「パスワード不一致」を同一の401メッセージにする | 応答を区別すると、どのユーザー名が存在するかを外部から列挙できてしまう(ユーザー列挙攻撃) |
| JWTの有効期限は24時間 | 学習用アプリとして、頻繁な再ログインを強いない範囲で妥当な長さ |
| `extract_user_id` ヘルパー方式を採用 | AxumのExtractorパターン(`FromRequestParts`)も検討したが、既存の `State + Path + Json` の書き味に揃えるほうが読みやすいと判断 |

## AppStateの拡張
```rust
struct AppState {
    games: Arc<RwLock<HashMap<Uuid, Chess>>>,
    db: PgPool,
    jwt_secret: Arc<String>,
}
```
起動時に `.env` から `DATABASE_URL` / `JWT_SECRET` を読む。`JWT_SECRET` 未設定時は開発用固定値で警告ログを出しつつ起動する(本番では必ず設定)。

## つまずいた点と教訓
| 症状 | 原因 | 対応 |
|---|---|---|
| `E0425: cannot find function 'verify_token'` | エディタ編集時に関数定義そのものが誤って消え、**呼び出し側だけが残っていた** | JWT検証ロジックを復元 |

- **教訓**: このプロジェクトで最頻出になるバグパターンの1件目。`cannot find function` が出たら、まず「その関数の定義がファイルに存在するか」を目視で確認する。呼び出し側だけが残るケースは、タイプミスではなく**定義の消失**を疑う

## 次タスクへの引き継ぎ
- `extract_user_id` は以降のすべての保護エンドポイントで使う
- `JWT_SECRET` は本番デプロイ時に `openssl rand -hex 32` で生成した値を環境変数として設定する

## 再現コマンド
```bash
curl -X POST http://localhost:3000/auth/register \
  -H "Content-Type: application/json" \
  -d '{"username":"alice","password":"password123"}'

curl -X POST http://localhost:3000/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"alice","password":"password123"}'

docker compose exec db psql -U chess -d chess_db \
  -c "SELECT id, username, left(password_hash, 20) FROM users;"
```