Tickreel v1.0.2

## 変更点

- **「私が作ったもの」を別ウィンドウに表示**します。アプリアイコンをクリックすると各サイトを開きます。一覧やアイコンはGitHubの共通設定から更新できます。

- **「私が作ったもの」のリンクをGitHubから自動更新**します。全OS・全配布版で起動時と1時間ごとに確認し、オフライン時は前回の設定を使用します。
- **OBSの録画開始・停止に自動連動**します。Tickreelソースを追加しておくと、OBSの通常録画に合わせてタイムラプス記録を開始・保存します。Tickreelソース自体は画像を表示しません。

- **開発者の共通表示「App Signature」を追加**しました。本体のサイドバーとOBS版のプロパティに `Made by appipinopi` を表示し、開発者のGitHubとTickreel公式サイトを既定のブラウザーで開けます。共通部品は [appipinopi/app-signature](https://github.com/appipinopi/app-signature) で管理しています。

- **OBSプラグイン版を追加**しました。OBSのソースに「Tickreel Timelapse」を追加し、プロパティから記録を開始・停止できます。OBSの最終出力を一定間隔で保存するので、シーン切り替えやカメラ・テキストを含む作業風景をタイムラプスにできます。
- **OBSの記録を本体へ自動取り込み**。プラグインで停止すると、本体v1.0.2のライブラリに取り込みます。テロップ・BGM・アフレコの編集と動画/GIF書き出しは本体で行います。
- OBSプラグインの初期値は **1秒間隔・15fps**。1分の作業が約4秒になります。本体で書き出すエンドカードは別途3秒です。

## OBS版の導入

`Tickreel-OBS-1.0.2-windows-x64.zip` を展開し、OBSを終了した状態で `obs-plugins` をOBSのインストール先へコピーしてください。再起動後、**ソース → ＋ → Tickreel Timelapse → プロパティ**から操作できます。

対応: Windows x64、OBS Studio 31以降（32.2.2で動作検証）。OBSの通常録画の開始・停止に連動します。音声は同時収録しません。OBSのProgram全体を記録し、別シーンへ切り替えても継続します。OBSの録画停止で保存します。プロパティから個別に開始・停止することもできます。

導入・使い方・保存先・制限の詳細: [OBS版の説明](https://github.com/appipinopi/tickreel/blob/v1.0.2/obs-plugin/README.md)

## 本体のダウンロード

Windowsは `Tickreel-1.0.2-windows-x64_x86.exe`（x64/x86自動選択）、ARM64は `Tickreel-1.0.2-windows-arm64.exe` を使用してください。CPU別ZIP、macOS・Linuxのプレビュー版もあります。本体にはFFmpegを同梱しています。

macOSは未公証、LinuxはUbuntu 24.04相当以降が対象です。`*-portable.exe` と `update-windows-*.json` は本体のアプリ内更新用です。OBSプラグインはZIPを再導入して更新します。
