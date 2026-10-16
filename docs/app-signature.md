# 共通の開発者表示と公式リンク

開発者の共通部品は [appipinopi/app-signature](https://github.com/appipinopi/app-signature) で管理しています。Tickreel本体とOBS版は、同じバージョンの部品と `assets/app-signature.json` を使用します。

本体のサイドバーに **Made by appipinopi ↗** と **公式サイト ↗** を表示します。開発者名はGitHubへ、公式サイトは `https://tickreel.ycccompany.jp/` へ移動します。OBS版ではプロパティに同じリンクを表示します。

開発者名・GitHub URLは共通部品の `creator.json`、アプリ名・公式サイトはTickreel側の `assets/app-signature.json` で管理します。プロフィールページの作成は不要です。全OS・全配布版で、起動時と1時間ごとにGitHubの設定を取得します。

**私が作ったもの ↗** は [アプリのポータル](https://portal.88soundboard.workers.dev/) を開きます。共通部品の `creator.json` 内の `worksUrl` で管理し、本体とOBS版に表示します。

## 他のアプリへ使う

Rustでは `app-signature` をGit依存として追加します。HTML / Electron / WebViewでは、同じリポジトリの `web/app-signature.js` と `.css` を組み込みます。設定例と利用方法は [共通部品のREADME](https://github.com/appipinopi/app-signature#readme) を参照してください。共通部品の取得先はコミットで固定し、各アプリの配布時に同梱します。

TickreelのWebViewは登録したリンクIDだけをRust側へ渡し、共通部品が既定のブラウザーで開きます。アプリの画面を外部サイトへ置き換えません。リンク設定はHTTPSでGitHubから取得し、接続失敗・不正な設定の場合は前回の有効なキャッシュ、初回は同梱設定を使用します。本体の実行ファイルの更新とは独立した仕組みです。

変更する場所: [GitHubでcreator.jsonを編集](https://github.com/appipinopi/app-signature/edit/main/creator.json)。`worksUrl` を変更してmainへコミットしてください。各アプリが次回取得した時点で反映します。Windows Store版・Snap版でもこのリンク設定だけを取得します。
