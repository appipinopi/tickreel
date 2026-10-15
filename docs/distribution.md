# 配布と更新

GitHub: https://github.com/appipinopi/tickreel

## GitHub版

インストーラーで導入するか、CPU別ZIPを展開して起動します。起動時に公開Releasesの `latest/download/update-windows-x64.json` / `update-windows-x86.json` / `update-windows-arm64.json` のうち、自分のCPU用の情報を確認します。新版がある場合はサイドバーからダウンロードできます。記録・アフレコ・書き出し中は更新のダウンロードを開始できません。準備完了後に通常終了すると差し替え・再起動します。自動ダウンロードや強制終了は行いません。

更新対象はアプリ本体のEXEです。HTML/CSS/JavaScriptもEXEに埋め込まれています。FFmpegを変更するリリースではインストーラーの再実行またはZIPの再展開が必要です。初回はインストーラーまたはZIPを使います。`*-portable.exe` は更新用の本体です。

メタデータはEd25519で署名し、アプリに埋め込んだ公開鍵で検証します。ダウンロードしたEXEはSHA-256・サイズを照合します。秘密鍵はソースや配布ZIPに含めません。このPCでは `%LOCALAPPDATA%\TickreelPublisher\update-signing-key.bin` に保存しています。安全な場所にバックアップしてください。GitHub Actionsは `TICKREEL_UPDATE_SIGNING_KEY` SecretのBase64形式の鍵を使います。これはWindowsのコード署名証明書とは別です。

リリース手順:

1. Cargo.tomlと画面表示のバージョンを上げ、Cargo.lockを更新します。
2. コミットをpushし、対応する `v1.0.1` 等のタグをpushします。
3. 先に対象タグのReleaseを下書きで作成します。Actionsが各CPU向けのテスト・ビルド・署名を実行します。
4. Releaseに添付されたインストーラー・ZIP・CPU別portable.exeとupdate-windows-*.jsonを確認し、公開すると更新対象になります。

既存タグのReleaseを同じバージョンのまま差し替えても、そのバージョンのユーザーには更新を配信しません。必ずバージョンを上げます。ネットワーク障害時も記録・編集は継続できます。署名鍵のローテーションには旧鍵で署名した、次の公開鍵を含む移行リリースが必要です。

## Microsoft Store版

Microsoft StoreのMSIX更新を利用します。パッケージIDを持つアプリ、または `TICKREEL_CHANNEL=store` でビルドしたアプリではGitHub更新を無効にします。Store配布ではGitHub版のEXE差し替えを使いません。

Partner CenterでWindowsアプリの製品「Tickreel」を予約した後、製品ID画面の **Package/Identity/Name**、**Package/Identity/Publisher**、**Package/Properties/PublisherDisplayName** を使用します。

Tickreelの実際の製品情報は `packaging/store-identity.json` に保存しています。Store IDは `9P6Z0TV3JVRR`、Identity Nameは `appipinopi.Tickreel` です。この製品向けのビルドは `./scripts/build-tickreel-store.ps1` で再実行できます。

```powershell
./scripts/build-store.ps1 -IdentityName '<Partner CenterのName>' -Publisher '<Partner CenterのPublisher>' -PublisherDisplayName '<PublisherDisplayName>'
```

Windows SDKのMakeAppxで `dist/Tickreel-Store-1.0.0.msix` を生成します。Partner Centerへのアップロード用の未署名パッケージです。ローカルへのインストールには適切な証明書による署名が別途必要です。製品IDが未発行の段階では実際の提出用パッケージを確定できません。

Storeでの公開には商品説明・スクリーンショット・プライバシーポリシーURL・年齢区分・runFullTrust権限の用途説明・審査が必要です。WebView2 Runtimeの利用可能性、パッケージ環境での画面取得・マイク・FFmpeg起動、保存先を提出前に実機確認してください。MSIXアプリのデータパスは通常のGitHub版と異なる場合があります。インストール方式間の録画データ移行は自動化していません。

参考: [Microsoft Storeの更新](https://learn.microsoft.com/en-us/windows/msix/store-developer-package-update)、[GitHubから組織へ移行](https://docs.github.com/en/account-and-profile/how-tos/account-management/moving-your-work-to-an-organization)。
