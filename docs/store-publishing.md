# Microsoft Store・Snap Storeへの配布と自動更新

## 現在の初回提出物

- Microsoft Store：`dist/Tickreel-Store-1.0.0.msix`（x64、Identity `appipinopi.Tickreel`、製品ID `9P6Z0TV3JVRR`）。
- Snap Store：`artifacts/snap-final/tickreel_1.0.0_amd64.snap`（core24・amd64・strict・grade devel）。初回はedgeへ配信します。
- Storeの商品説明・サイトURL・権限の用途説明：`packaging/store-listing-ja.json`。

Microsoft Storeは初回公開をPartner Centerで完了してからSubmission APIによる更新を使います。
MSIXのアップロード、商品情報、年齢区分、審査などの初回手続きを済ませてください。
ローカルMSIXはStore提出用の未署名パッケージです。
Microsoft Storeの初回提出は2026-10-15に審査へ提出済みです。審査通過後に自動公開される設定です。
Snapの配布用認証情報はGitHub Secretsへ設定済みです。配布先は`TICKREEL_SNAP_CHANNEL=edge`で、認証情報の有効期限は約90日です。
2026-10-15にSnap Storeへ`1.0.0`（amd64、リビジョン1）をアップロードし、`latest/edge`へ公開しました。
導入は`sudo snap install tickreel --edge`、以後はsnapdが同じチャンネルの更新を管理します。

## ストア掲載素材（2026-10-15）

Microsoft Storeの日本語掲載には、スクリーンショット4枚と各説明、正方形・縦長の紹介画像、文字なしのヒーロー画像、20秒の1080p紹介動画・サムネイル・日本語字幕を追加し、審査へ再提出しました。公式サイト・プライバシー・サポートURL、検索キーワード7件も登録済みです。
Snap Storeには、ロゴ・バナー・スクリーンショット4枚、英語と日本語の説明、カテゴリ2件、公式サイト・プライバシー・ソースコード・問い合わせ・不具合報告先を登録済みです。埋め込み用YouTube/Vimeo URLは未提供のため、紹介動画は説明欄のMP4リンクから開く形式です。

紹介動画： https://github.com/appipinopi/tickreel/releases/download/v1.0.0/tickreel-trailer-1080p.mp4

`node scripts/prepare-store-media.mjs`で、既存の`artifacts/x-promo-v2`の紹介動画からストア用の横長動画・字幕を作り、SVGで描いたブランド画像をPNGへ出力します。出力先は`artifacts/store-media/`です。実行には既存のsharpとffmpeg-static、および元動画・ナレーションJSONが必要です。

## 新版からの自動配布

`.github/workflows/store-distribution.yml` はGitHub Releaseの公開時に動きます。
先にCargo.toml・Cargo.lock・画面表示のバージョンを更新し、対応するタグをpushします。
Windows/Linuxの通常リリースをビルドし、Releaseの配布物を確認してから公開してください。

1. MSIXジョブはStoreチャネルでテスト・ビルドし、正しい製品IDとStore管理の更新状態を検証します。
2. MSIXをActions Artifactに保存し、Submission APIから新版を提出します。既存の商品情報と他CPUのパッケージは引き継ぎます。
3. Snapジョブは同じ版の公開Linux tar.gzを取得し、URL・バージョン・SHA-256をSnapcraftのレシピへ設定します。
4. SnapをビルドしてActions Artifactに保存し、指定チャンネルにアップロードします。Storeのレビューが通ると配信されます。

並列実行による競合を防ぎ、既存のMicrosoft Store提出物は削除・上書きしません。
同じ版のMSIX再提出やダウングレードも拒否します。失敗時は既存の提出状況を確認してから再実行してください。
Microsoftの審査完了は非同期です。スクリプトの「提出済み」は「公開済み」を意味しません。
資格情報がない場合はパッケージをArtifactに残し、提出ステップをエラーとして報告します。

手動実行ではrelease_tagとsource_refを指定できます。Cargoの版はrelease_tagと一致する必要があります。
古いv1.0.0タグには今回追加した配布スクリプトがないため、初回手動実行にはスクリプトを含むmainをsource_refに指定します。
今後の通常公開では、対応タグからビルドします。

## GitHub Secrets

Microsoft EntraアプリをPartner Centerに登録し、提出操作に必要な権限を付与します。
次の3つをリポジトリのActions Secretsに保存してください。

- `MS_STORE_TENANT_ID`
- `MS_STORE_CLIENT_ID`
- `MS_STORE_CLIENT_SECRET`

`./scripts/configure-store-secrets.ps1 -MicrosoftStore` で対話的に保存できます。
クライアントシークレットは画面・コマンド引数・ソースに出しません。

Snapは、SnapcraftをインストールしたLinux/WSLのターミナルで次を実行します。
Ubuntu Oneへのログインが求められます。資格情報ファイルはGit管理・配信しないでください。

```sh
mkdir -p .store-credentials
snapcraft export-login .store-credentials/snapcraft.txt --snaps tickreel --channels edge --acls package_access,package_push,package_release
```

生成したファイルをWindows側から指定して保存します。

```powershell
./scripts/configure-store-secrets.ps1 -SnapCredentialsFile .store-credentials/snapcraft.txt
```

GitHub Secret名は `SNAPCRAFT_STORE_CREDENTIALS` です。資格情報の有効期限が切れる前に更新してください。
WSLの既定PATHにsnapcraftがない場合は `/snap/bin/snapcraft` を使用します。

## 初回Snapのアップロード

認証情報を設定したLinux/WSLで実行します。

```sh
export SNAPCRAFT_STORE_CREDENTIALS="$(cat .store-credentials/snapcraft.txt)"
PATH="/snap/bin:$PATH" bash scripts/publish-snap.sh artifacts/snap-final/tickreel_1.0.0_amd64.snap edge
```

GitHub側の配信先はRepository Variable `TICKREEL_SNAP_CHANNEL` で指定します。未設定ではedgeです。
candidate/stableへ進める前に、実機で導入・X11記録・音声権限・書き出しを検証し、gradeをstableへ変更してください。
edge限定の認証情報ではstableへ配信できないため、資格情報のチャンネル範囲も更新が必要です。

## 利用者側の更新

Microsoft Storeから導入したMSIXの更新はMicrosoft Storeが管理します。更新設定や組織ポリシーに従います。
Snap Storeから導入したSnapはsnapdが追跡しているチャンネルから更新します。
Store版に、EXEを直接差し替える別の常駐更新スクリプトは入れません。
`snap install --dangerous` でローカルファイルを導入しただけではStore配信を追跡しません。

## ローカル検証

```powershell
python scripts/test-store-publishing.py
python scripts/publish-store-update.py --package dist/Tickreel-Store-1.0.0.msix --validate-only
```

参考：
- https://learn.microsoft.com/en-us/windows/uwp/monetize/manage-app-submissions
- https://learn.microsoft.com/en-us/windows/apps/publish/msstore-dev-cli/github-actions
- https://ubuntu.com/docs/snapcraft/latest/how-to/publishing/authenticate/
- https://documentation.ubuntu.com/snapcraft/stable/how-to/publishing/manage-revisions-and-releases/
