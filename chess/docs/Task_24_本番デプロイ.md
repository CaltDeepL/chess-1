# task-24 本番デプロイ(Render + Neon)

## ゴールと完了条件
- バックエンド・DB・フロントエンドを本番環境にデプロイする
- 完了条件: 公開URLでPC・スマホ両方から新規登録〜対局までが通しで動作すること

## 構成と選定理由
| 役割 | サービス | 理由 |
|---|---|---|
| バックエンド | **Render** 無料Web Service(Docker) | 既存のDockerfileがそのまま使える。WebSocketも通る。15分アクセスがないとスリープし再起動に数十秒かかるが、学習用途として許容 |
| DB | **Neon** 無料枠 | 0.5GBストレージ・月100CU時間まで**恒久無料**。Renderの無料Postgresは**30日で自動削除**されるため別サービスに分離した |
| フロント | **Render** 静的サイト | 同じダッシュボードで管理でき、Gitプッシュで自動デプロイ |

※ Fly.io / Railway は2024年以降、常時無料枠を廃止しておりトライアルのみのため候補から除外。

## コード側の対応
- **CORS**: `tower-http` の `CorsLayer` を追加。`FRONTEND_ORIGIN` 環境変数(**カンマ区切りで複数オリジン許可**、未設定時はローカル開発用ポートにフォールバック)から構築
- **ポート**: `PORT` 環境変数を読み、未設定時は3000にフォールバック
- **Dockerfile**: マルチステージビルド化し、依存関係のビルドキャッシュを効かせる構成に変更(実際に2回ビルドしてキャッシュヒットを確認)
- **フロント**: `client.ts` の `BASE_URL` / `WS_BASE_URL` を `import.meta.env.VITE_API_URL` から読むよう変更。`.env.production` / `vite-env.d.ts` を追加
- **SPAフォールバック**: `public/_redirects` と `vercel.json` を追加(最終的にはRenderダッシュボードの Redirects/Rewrites 機能で `/*` → `/index.html` を設定)

## デプロイ手順
1. Neonでプロジェクト作成 → 接続文字列を取得(**`-pooler` の付かない直接エンドポイント**を使う)
2. RenderでバックエンドをWeb Service(Docker、Root Directory: `chess`)として作成、環境変数を設定
3. ローカルからNeonへマイグレーション適用
4. `/health` で疎通確認
5. `.env.production` にバックエンドURLを設定してコミット・push
6. Renderで静的サイトを作成(Build: `npm install && npm run build`、Publish: `dist`)、Rewriteルールを設定
7. バックエンドの `FRONTEND_ORIGIN` をフロントのURLに更新
8. PC・スマホで通しE2E

## つまずいた点と教訓
| # | 症状 | 原因 | 対応 |
|---|---|---|---|
| 1 | `TLS upgrade required by connect options but SQLx was built without TLS support` | `sqlx-cli` がTLS未対応でビルドされていた。ローカルPostgresしか使っていなかったため気づかなかった | `--features rustls,postgres` で入れ直し |
| 2 | `Root directory 'vite-project' does not exist` | **`vite-project` はローカルのフォルダ名にすぎず、GitHubリポジトリ内には存在しなかった**(リポジトリ直下に `src/` と `chess/` が並ぶ構成) | `git rev-parse --show-toplevel` で確認し、Root Directoryを空欄に修正 |
| 3 | フロントのビルドが `chess.js` 未検出・`MoveRow` 型なし・`onPromotionNeeded` 不足で失敗 | **これらの実装済み変更がPR #14マージ後に作られたコミットに含まれ、`main` ブランチに未反映だった** | 新規PRを作成して `main` にマージ |
| 4 | バックエンドが `Exited with status 1` | ログに `invalid port value`。**Render側で手動設定していた `PORT` 環境変数**が、Renderが自動注入する値と競合していた | 手動設定した `PORT` を削除 |
| 5 | CORSエラー | `FRONTEND_ORIGIN` 未設定でデフォルト(localhostのみ)が適用されていた | フロントのURLを設定 |
| 6 | スマホで `notfound` 表示 | ビルド失敗時のRenderの404ページが**ブラウザにキャッシュ**されていた | シークレットモードで解決を確認、キャッシュクリア |

### 教訓
- **#1**: ローカルで動く構成と本番で必要な構成は違う。TLSはマネージドDBでは必須
- **#2**: **ローカルのディレクトリ名とリポジトリの構造は別物**。デプロイ設定を書く前に `git rev-parse --show-toplevel` と `ls` でリポジトリの実際の構造を確認する
- **#3(最重要)**: 「ローカルで動いている」と「pushされている」と「デプロイ対象ブランチに入っている」は**すべて別**。`git status` で未コミットを、`git log` でブランチ間の差分を確認する
- **#4**: PaaSが自動で注入する環境変数を、良かれと思って手動設定すると壊れる。プラットフォームのドキュメントで「自分で設定すべきもの/されるもの」を区別する
- **#6**: デプロイ直後の「動かない」は、実際には**古いレスポンスのキャッシュ**であることが多い。必ずシークレットモードで確認する

## 次タスクへの引き継ぎ(残る課題)
- カスタムドメインの設定
- Renderの無料枠スリープ対策(定期ping等、必要になれば)
- レーティング機能・対局履歴閲覧UI

## 再現コマンド
```bash
# JWT_SECRETの生成
openssl rand -hex 32

# sqlx-cliをTLS対応で入れ直す
cargo uninstall sqlx-cli
cargo install sqlx-cli --no-default-features --features rustls,postgres

# 本番DBへマイグレーション適用
DATABASE_URL="<Neonの接続文字列>" sqlx migrate run

# 疎通確認
curl https://<バックエンドURL>/health

# リポジトリ構造・差分の確認
git rev-parse --show-toplevel
git status
# https://github.com/<owner>/<repo>/compare/main...<feature-branch>
```

### Render環境変数(バックエンド)
```
DATABASE_URL=<Neonの接続文字列>
JWT_SECRET=<openssl rand -hex 32 の出力>
FRONTEND_ORIGIN=http://localhost:5174,https://<フロントのURL>
```
※ `PORT` は**設定しない**(Renderが自動注入)

### フロント環境変数(`.env.production`)
```
VITE_API_URL=https://<バックエンドURL>
```