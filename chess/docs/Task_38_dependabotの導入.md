# task-38 Dependabot の導入

## ゴールと完了条件
- Cargo / npm / GitHub Actions の依存更新を自動で検知し、PR として受け取る
- 完了条件: `.github/dependabot.yml` を置いて PR が立ち、CI が走ること

## 追加するもの

| ファイル | 内容 |
|---|---|
| `.github/dependabot.yml` | 3つのエコシステムの設定 |

コードには一切手を入れない。

## 設計判断の根拠

### patch / minor をグループ化する
Dependabot はデフォルトで**依存ごとに PR を立てる**。Cargo と npm を合わせると週に10本以上になる。

数が多いと結局まとめて放置することになり、**「更新の自動化」を入れたのに更新が滞る**という本末転倒が起きる。破壊的変更を含まない patch / minor は1本にまとめ、レビューする対象を減らす。

### major は個別の PR にする
グループに含めると、1本の PR に破壊的変更が複数混ざる。落ちたときにどれが原因か分からず、片方だけ戻すこともできない。major は単独で見る。

### ESLint 関連をまとめる理由
`eslint` 本体とプラグイン群はバージョンが連動する。別々の PR で片方だけ上がると、**設定ファイルが読めなくなって lint 全体が落ちる**。同じ PR で上がれば CI がその組み合わせを検証してくれる。

`@types/*` も同様に、本体の更新と一緒に上がることが多いのでまとめる。

### 自動マージしない
CI が緑でも、依存の更新は挙動が変わりうる。テストが網羅していない部分（見た目、パフォーマンス、エラーメッセージの文言）は検知できない。

このプロジェクトの規模なら週1でレビューできるので、自動マージの利点は小さい。

### 週1・月曜の朝にする
毎日にすると PR が溜まってレビューしなくなる。週の初めにまとめて処理する形にした。

### GitHub Actions も対象にする
更新頻度は低いが、**古いまま放置すると Node のランタイム廃止である日突然 CI が動かなくなる**。`actions/checkout@v3` のような指定が非推奨になり、警告のあと停止する。定期的に上げておけば、その日に慌てずに済む。

## 確認すべき前提

| # | 項目 | 結果 |
|---|---|---|
| 1 | `chess/Cargo.toml` | あり → `directory: "/chess"` |
| 2 | `package.json` の場所 | **リポジトリ直下** → `directory: "/"` |
| 3 | ラベルの存在 | 未作成のラベルは Dependabot が作る |

### フロントエンドの構成が想定と違った

```
package.json          ← リポジトリ直下
vite.config.ts        ← リポジトリ直下
index.html            ← リポジトリ直下（<script src="/frontend/main.tsx">）
frontend/             ← ソースのみ（main.tsx, pages/, components/...）
chess/
  Cargo.toml
  src/
```

`frontend/` は**ソースディレクトリであってパッケージのルートではない**。`directory: "/frontend"` にすると `package.json` が見つからず、**エラーにならないまま npm の PR が1本も立たない**。

`github-actions` も `"/"` を指定しているが、Dependabot は `package-ecosystem` と `directory` の組で識別するので衝突しない。

**あわせて確認したいこと**: この構成では、フロントエンドのビルドはリポジトリ直下を作業ディレクトリとして `npm install && npm run build` を実行する必要がある。Render / Vercel の Root Directory 設定が `frontend` になっていると `package.json` が見つからず失敗する。`vercel.json` の設定とも矛盾がないか見ておくとよい。

README のディレクトリ構成の記述も、この実態に合わせて直す必要がある（`frontend/` の下に `package.json` があるかのように書いていた）。

## Dependabot PR と CI の関係

```
Dependabot が PR を作成
        ↓
pull_request トリガーで CI が走る
        ↓
green ならレビューしてマージ
        ↓
main への push で CI → workflow_run で Deploy
```

**PR の段階ではデプロイは起きない。** Deploy ワークフローは `workflow_run` で main の CI 成功を受けているため。

なお **Dependabot の PR にはリポジトリの Secrets が渡らない**（フォークからの PR と同じ扱い）。このプロジェクトの CI は Postgres をサービスコンテナで起動しており Secrets を使っていないので影響しない。将来 CI で Secrets を使うようになったら、Dependabot PR だけ落ちるので注意。

## 導入手順

```bash
mkdir -p .github
# dependabot.yml を配置
git add .github/dependabot.yml
git commit -F docs/commits/task-38-chore-add-dependabot.txt
git push
```

配置後、GitHub の **Insights → Dependency graph → Dependabot** から状態を確認できます。初回の実行を待たずに **Check for updates** で手動実行できます。

### 最初の PR で見るところ

初回は溜まっていた更新がまとめて来るはずです。

- グループ化が効いて PR が数本に収まっているか（10本以上ならグループ設定が効いていない）
- CI が走っているか
- `directory` の指定が間違っていると、**そのエコシステムの PR が1本も立たない**。エラーにならず静かに何も起きないので、3系統すべてから PR が来ているかを確認する

## 導入結果

初回スキャンで3系統すべてから PR が作成された。

### グループ化の効果

| エコシステム | 個別なら | 実際 |
|---|---|---|
| npm | 11本 | **3本**（minor/patch 9件が1本 + typescript + @types/node） |
| Cargo | 6本 | **5本**（minor/patch の uuid が1本 + major 4本） |
| GitHub Actions | 2本 | **1本** |
| 合計 | 19本 | **9本** |

**npm の効果が大きい。** 9件が1本にまとまった。個別に11本来ていたら、この時点でレビューを諦めていた可能性が高い。

Cargo は major が4本あるため圧縮率は低いが、これは設計どおり。破壊的変更を1本にまとめないという判断が、そのまま本数に出ている。

### CI の結果

| PR | CI | 種別 |
|---|---|---|
| npm-minor-patch（9件） | 緑 | minor/patch |
| @types/node 24.13.3 → 26.4.1 | 緑 | major だが型定義のみ |
| uuid 1.10.0 → 1.26.0（cargo-minor-patch） | 緑 | minor |
| actions group（2件） | 緑 | — |
| typescript 6.0.3 → 7.0.2 | 赤 | major |
| axum 0.7.9 → 0.8.9 | 赤 | 破壊的変更 |
| utoipa-axum 0.1.3 → 0.2.0 | 赤 | axum と連動 |
| tokio-tungstenite 0.24 → 0.30 | 赤 | axum と連動 |
| jsonwebtoken 9.3.1 → 11.0.0 | 赤 | major |

**緑の4本を先にマージする。** 赤を後回しにしても実害はなく、緑を取り込んでおけば残りの差分が小さくなる。

### 分かったこと: 連動する依存は個別 PR では通らない

`axum` / `utoipa-axum` / `tokio-tungstenite` の3つは、**一緒に上げないとどれも通らない**。axum 0.8 では `Router` の型や WebSocket 周りが変わり、他の2つもそれに追随したバージョンが必要になる。

Dependabot は依存関係のバージョン制約からグループを推測できないため、個別に PR を立てる。`groups` の `patterns` で「axum 系」としてまとめる案もあるが、**major を1本にまとめないという方針とは両立しない**。

現実的には、これらは Dependabot の PR を閉じて、ローカルで1本のブランチにまとめて対応する。移行作業そのものがコード変更を伴うため、依存更新というより**移行タスク**として扱うほうが実態に合う（task-39）。

なお ESLint 関連を最初から `patterns` でグループ化しておいたのは同じ理由による。**バージョンが連動する組は、あらかじめグループに入れておくしかない。**

## 次タスクへの引き継ぎ
- **task-39: axum 0.8 系への移行**（axum / utoipa-axum / tokio-tungstenite の3つを同時に上げる）
- 単独で対応できる major: `typescript` 7、`jsonwebtoken` 11
- Future Work の残り: MFA（TOTP）、K 値の可変化、再接続時のイベント補完、レーティング推移のグラフ
- 運用してみて PR が多すぎる・少なすぎると感じたら `interval` と `open-pull-requests-limit` を調整する
- セキュリティ更新（Dependabot alerts）は別機能。リポジトリの Settings → Code security から有効にすると、脆弱性が見つかったときはスケジュールを待たずに PR が立つ