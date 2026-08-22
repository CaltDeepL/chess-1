# task-10 フロントエンド設計と土台実装

## ゴールと完了条件
- 画面構成・状態管理方針・ディレクトリ構成・データフローを設計する
- 型定義・APIラッパー・認証状態・ルーティングの「土台」を実装する
- 完了条件: 未認証で `/lobby` にアクセスすると `/login` にリダイレクトされ、トークン保持状態では保護ルートを通過できること

## 画面構成
| パス | 画面 | 概要 |
|---|---|---|
| `/login` | ログイン | ユーザー名/パスワード → JWT取得 |
| `/register` | 新規登録 | ユーザー作成 |
| `/lobby` | ロビー | 対局一覧、新規対局作成 |
| `/games/:id` | 対局画面 | 盤面、指し手、WebSocket、投了 |

## 設計判断の根拠
| 判断 | 根拠 |
|---|---|
| Redux等の外部stateライブラリは不使用 | 画面が4枚、共有すべき状態は認証情報のみ。React Context + useState で足りる |
| 認証状態は `AuthContext` + localStorage | リロードでログアウトされると開発中の確認が煩雑。永続化する |
| 対局状態は対局画面内にローカライズ | 他画面から参照しない。グローバルに置くと不要な再描画と結合を生む |
| 合法手判定はフロントで行わない | バックエンド(shakmaty)が権威。フロントは見た目のドラッグ操作を許可し、サーバーが拒否したら戻す方針(後に task-19 で表示専用のハイライトのみ追加) |

## ディレクトリ構成
```
src/
  api/       client.ts / auth.ts / games.ts
  context/   AuthContext.tsx
  hooks/     useGameSocket.ts
  pages/     LoginPage / RegisterPage / LobbyPage / GamePage
  components/ ChessBoard.tsx / GameList.tsx
  types/     index.ts
  App.tsx    main.tsx
```

## つまずいた点と教訓
| # | 症状 | 原因 | 対応 |
|---|---|---|---|
| 1 | バックエンドと型が合わない | `AuthResponse` を `{token, user}` と想定していたが実際は `{user_id, token}`(**usernameを含まない**)。`GameEvent` もキャメルケース想定だったが実際はsnake_case | 実バックエンドのレスポンスに合わせて型定義を修正 |
| 2 | `verbatimModuleSyntax` エラー | `ReactNode` を型でなく値としてインポートしていた | `import type` に分離 |
| 3 | **エントリーポイントが機能しない** | `main.tsx` の `createRoot(...).render(<App/>)` が丸ごと消え、代わりに**4ページ分のコンポーネント定義が誤って書き込まれていた** | 各ページファイルへ戻し `main.tsx` を復元 |

- **教訓(型の突き合わせ)**: フロントの型定義は「こうあるべき」ではなく「実際に返ってくるJSON」に合わせる。最初に実物を1回curlで叩いて確認するほうが速い
- **教訓(#3)**: バックエンドの `state.rs` ↔ `ws.rs`(task-08)と**同じファイル誤混入がフロントでも発生**した。環境をまたいで繰り返すパターンだと認識する
- `AuthResponse` にusernameが無いため、表示名はフォーム入力値をそのまま `AuthContext` に保持する設計とした

## 次タスクへの引き継ぎ
- 型のみのインポートは**最初から `import type` で書く**(このあとも同じエラーを踏む)
- ロビー画面には対局一覧APIが必要だが、バックエンドに `GET /games` が存在しない → task-11 で追加する

## 再現コマンド
```bash
npm install react-router-dom
npx tsc -b --force
npm run build
npm run dev
```