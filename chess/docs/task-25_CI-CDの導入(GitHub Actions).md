# task-25 CI/CDの導入(GitHub Actions)

## ゴールと完了条件
- push / PR で自動的にフォーマット・lint・ビルド・テストが走る
- CIが成功したときだけ本番デプロイされる
- 完了条件: PRでCIが緑になり、mainマージ後にDeployワークフローがRenderのDeploy Hookを起動すること

## 構成
### `.github/workflows/ci.yml`
| ジョブ | 内容 |
|---|---|
| backend | `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test --all-targets` |
| frontend | `npm ci` / `npx tsc -b --force` / `npm run lint` / `npm run build` |

### `.github/workflows/deploy.yml`
`workflow_run` イベントでCIの完了を受け取り、**成功かつ `main` ブランチのとき**だけ Render の Deploy Hook を起動する。

```yaml
on:
  workflow_run:
    workflows: [CI]
    types: [completed]

jobs:
  deploy:
    if: >
      github.event.workflow_run.conclusion == 'success' &&
      github.event.workflow_run.head_branch == 'main'
```

## 設計判断の根拠

### なぜ Render の auto-deploy を無効にするのか
Renderのauto-deployは `main` への push を検知して即座にビルドを始めるため、**テストの結果を待ちません**。テストが落ちるコードでもデプロイされてしまいます。

auto-deployを切り、GitHub Actions の `workflow_run` でCIの完了と結果を受け取ってから Deploy Hook を叩く構成にしました。これで「CIが green のときだけデプロイが走る」が保証されます。

### なぜCIにPostgresを立てないところから始めたのか
このプロジェクトは `sqlx::query!` マクロ(コンパイル時SQL検証)を使っておらず、`sqlx::query()` / `query_as::<_, T>()` の実行時版のみです。そのため `.sqlx` オフラインキャッシュも `cargo sqlx prepare` も不要で、**コンパイルにDBは要りません**。

導入当初はテストコードも空だったため、DBサービスを立てても何も検証しないまま起動コストだけがかかる状態でした。統合テストを書いた task-26 の時点で追加しています。

### なぜ `workflow_dispatch` を入れたのか
`push: branches: [main]` と `pull_request` だけだと、feature ブランチでワークフロー自体をデバッグする手段がありません。Actionsタブから任意のブランチで手動実行できるようにしておくと、CI設定の試行錯誤が速くなります。

## つまずいた点と教訓

| # | 症状 | 原因 | 対応 |
|---|---|---|---|
| 1 | Actionsタブに「Get started with GitHub Actions」のテンプレート選択画面が出たまま | **ワークフローを `chess/.github/workflows/` に置いていた。** GitHub Actions はリポジトリルートの `.github/workflows/` しか見ない | `git mv chess/.github .github` でルートへ移動 |
| 2 | ブラウザで見ているリポジトリにワークフローが無い | GitHub上で `test1` → `chess-1` にリネームしていたが、ローカルの remote が旧名のまま。リネーム後も旧URLはリダイレクトされるため **push自体は成功してしまう** | `git remote set-url origin https://github.com/CaltDeepL/chess-1.git` |
| 3 | ワークフローは認識されたが実行されない | 作業ブランチが `fix/getrandom-version` で、`push` トリガーの `main` に該当していなかった | PRを作成して `pull_request` トリガーで発火させた |
| 4 | Deploy ワークフローが5秒で失敗 | `RENDER_DEPLOY_HOOK_BACKEND` シークレットが未登録。**未定義のシークレットは空文字列に展開される**ため `curl -fsS -X POST ""` となり即エラー | シークレットを登録 |
| 5 | `Invalid workflow file: ci.yml#L1` | Postgres サービス追加時に、`jobs:` 配下のインデントがずれた | ファイル全体を正しい階層で書き直し |

### 教訓
- **#1**: ワークフローの置き場所はリポジトリルート固定。サブディレクトリのプロジェクトでも例外はない。なお `git ls-files .github/` は**カレントディレクトリからの相対パス**を返すため、`chess/` にいると正しく置けているように見えてしまう
- **#2**: リポジトリのリネーム後は remote の更新を忘れがち。**旧URLがリダイレクトされて push が通ってしまう**ため、エラーで気づけない
- **#4**: 未定義シークレットが空文字列になる挙動は、設定漏れをエラーではなく「意味不明な失敗」として見せる。シークレットを使うステップは、実行前に登録済みかを確認する
- **#5**: YAMLの構文エラーは push しないと気づけず、往復が無駄になる。事前に検証する習慣をつける

```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml')); print('OK')"
```

## 導入前に解消したlintエラー(7件)
CIで `-D warnings` / `npm run lint` を通すため、事前にローカルで修正した。

### `react-hooks/set-state-in-effect`(3件)
`eslint-plugin-react-hooks` v7 の実装(`validateNoSetStateInEffects`)を読んで判定基準を特定した。**`await` の前後は無関係**で、「**setStateを含む関数を effect のトップレベルで直接コールしているか**」だけが基準。

| パターン | 判定 |
|---|---|
| `fetchGames()` を effect 本体で直接呼ぶ | ✗ 警告 |
| `setInterval(fetchGames, 5000)` のように**参照として渡す** | ✓ セーフ |
| `getGame(id).then(callback)` のようにコールバックを渡す | ✓ セーフ |

対応: `useGameSocket` を `lastEvent` の state 返却から `onEvent` コールバック方式に変更(WSの `onmessage` から直接呼ぶ「外部システムへの反応としてのsetState」という正当なパターンに沿わせた)。`LobbyPage` の初回フェッチは `queueMicrotask(fetchGames)` として、`setInterval` と同じ参照渡しパターンに統一。

### `react-refresh/only-export-components`(4件)
Context オブジェクトとフックを Provider コンポーネントと同じファイルから export していたため。`auth-context.ts` / `useAuth.ts`、`toast-context.ts` / `useToast.ts` に分離し、`AuthContext.tsx` / `ToastContext.tsx` は Provider のみを export するようにした。駒コンポーネントも同様に、内部コンポーネントを `GlassPiece.tsx` / `UnicodePiece.tsx` に切り出し、共有の型と定数を `lib/pieceSymbols.ts` にまとめた。

## 次タスクへの引き継ぎ
- CIは通るようになったが、この時点で `cargo test` の中身は空。**緑であることが実質的な意味を持たない**ため、統合テストの追加が次の課題(task-26)
- テストがDBを使うようになったら `ci.yml` に Postgres サービスの追加が必要

## 再現コマンド
```bash
# push前にローカルで同じチェックを通す
cd chess
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cd ..
npm run lint
npx tsc -b --force
npm run build

# YAML構文の事前検証
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml')); print('OK')"

# remoteの確認(リネーム後は必須)
git remote -v
git rev-parse --show-toplevel
```