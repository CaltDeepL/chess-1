# task-12 認証画面(ログイン・新規登録)

## ゴールと完了条件
- `/login` と `/register` のフォームが動作し、成功時に `/lobby` へ遷移する
- 完了条件: ログイン成功、ユーザー名重複(409)、パスワード不一致の各シナリオがブラウザで期待どおり動くこと

## 実装
### LoginPage.tsx
- フォーム送信 → `login` API → `AuthContext` に保存 → `/lobby` へ遷移
- 401時は「ユーザー名またはパスワードが違います」の専用メッセージ

### RegisterPage.tsx
- LoginPageと同構造で `register` API に差し替え
- パスワード確認欄を追加し、**不一致はAPI呼び出し前にフロント側でブロック**
- 409時は「このユーザー名はすでに使われています」

## 設計判断の根拠
- **パスワード一致チェックはフロントで先行実施**: サーバーに送る必要のないリクエストを減らせる。ネットワークログでAPIが呼ばれていないことも確認した
- **`AuthResponse` にusernameが無いため、フォーム入力値を表示名として保持**: 表示のためだけに `GET /users/:id` を追加するのは過剰と判断(後に対戦相手名表示のため task-18 で結局追加することになる)
- **`minLength={8}`**: バックエンドの `password.len() < 8` 要件と一致していることを確認済み。ブラウザ側の簡易バリデーションとして先に弾く

## つまずいた点と教訓
| 症状 | 原因 | 対応 |
|---|---|---|
| `error TS1484: 'FormEvent' is a type and must be imported using a type-only import` | task-10 の `ReactNode` と**同じ `verbatimModuleSyntax` パターンが再発** | `import type` に分離 |

- **教訓**: 同じルール違反を2度踏んだ。以降のページでは最初から `import type` で書くことを癖にする。それでも task-18 の `ToastContext.tsx` でまた踏むことになる

## E2E検証結果
| シナリオ | 結果 |
|---|---|
| 新規登録 | `POST /auth/register` → 200 OK → `/lobby` へリダイレクト |
| 同じユーザー名で再登録 | 409 Conflict → 「このユーザー名はすでに使われています」表示 |
| パスワード不一致 | **APIリクエストが発生せず**フロント側でブロック |
| ログイン | 200 OK → `/lobby` → `localStorage` に token/user 保存を確認 |

## 次タスクへの引き継ぎ
- 認証が通るようになったので、以降の画面はログイン済み前提で実装できる
- CSSクラス名(`auth-page` / `auth-form` 等)は仮。スタイリングは task-17 でまとめて行う

## 再現コマンド
```bash
npx tsc -b --force
npm run dev
# ブラウザで /register → /login → /lobby の遷移と localStorage を確認
```