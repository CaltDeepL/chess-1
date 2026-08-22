# task-09 コード構成のモジュール分割リファクタリング

## ゴールと完了条件
- `main.rs` 単一構成から責務ごとのモジュールに分割する(ロジック変更なし、移動のみ)
- 完了条件: `cargo build` が警告ゼロで通り、E2Eでリファクタ前と同一挙動であること

## 最終構成
```
chess/src/
├── main.rs        # 起動処理のみ(dotenv, tracing, DB接続, Router組み立て)
├── state.rs        # AppState(games, db, jwt_secret, game_channels)、GameEvent型
├── models.rs        # リクエスト/レスポンス型、DB行型
├── auth.rs          # register/login/issue_token/verify_token/extract_user_id
└── routes/
    ├── mod.rs        # サブモジュールのre-export
    ├── health.rs     # health_check
    ├── game.rs        # create_game/join_game/get_game/make_move/resign_game/
    │                  #   determine_outcome/position_to_fen
    └── ws.rs         # ws_handler
```

## 設計判断の根拠
- **1ステップずつビルド確認しながら進める**: 全ファイルを一括で差し替えると、ビルドが壊れたときに原因箇所の特定が困難になる。`state.rs`・`models.rs` → `auth.rs` → `routes/` → `main.rs` スリム化、の順に分割した
- **`pub` は必要最小限**: クロスモジュール参照が必要な型・関数(`AppState` とそのフィールド、`models` の各構造体、`extract_user_id`、各ハンドラ)だけに付与。`verify_token` / `issue_token` / `determine_outcome` / `position_to_fen` はモジュール内限定のため非公開のまま
- **task-01 の設計案から簡略化**: 当初想定した `app.rs` / `game/{session,manager,message}.rs` / `error.rs` は作らず、`state.rs` / `models.rs` / `auth.rs` に統合。実装が出揃った後だと「実際にどれだけの量があるか」がわかるため、過剰な分割を避けられた

## つまずいた点と教訓
- 特筆すべき詰まりなし。**機能実装を先に進め、構造の整理は後でまとめて行う**という task-01 の方針が、この段階で狙いどおり機能した
- **教訓**: リファクタリングは「動く状態」を保ちながら小刻みに進める。1ステップごとにビルドが通ることを確認すれば、壊れた瞬間に原因が確定する

## 次タスクへの引き継ぎ
- 以降の新規エンドポイントは `routes/` 配下に追加する(task-11 の `list_games`、task-18 の `user.rs` など)
- 型は `models.rs` に、共有状態は `state.rs` に集約する規約が確立した

## 再現コマンド
```bash
cargo build            # 警告ゼロを確認
docker compose up -d --build
# リファクタ前と同じE2Eを再実行
curl http://localhost:3000/health
curl -X POST http://localhost:3000/auth/register -H "Content-Type: application/json" -d '{"username":"...","password":"..."}'
curl -X POST http://localhost:3000/games -H "Authorization: Bearer <token>"
```