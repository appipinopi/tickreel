# Tickreel

**一日の積み重ねを、一本のタイムラプスに。**

<a href="https://snapcraft.io/tickreel">
  <img alt="tickreel" src="https://snapcraft.io/tickreel/badge.svg" />
</a>
<script type="module" src="https://get.microsoft.com/badge/ms-store-badge.bundled.js"></script>
<ms-store-badge productid="9P6Z0TV3JVRR" theme="auto" language="ja-jp"></ms-store-badge>

お絵描き、プログラミング、毎日の制作作業を記録し、テロップ・自分の声・BGMを添えた動画にするデスクトップアプリです。Rustで動作し、画面と音声は自分のPCに保存します。

[ダウンロード](https://github.com/appipinopi/tickreel/releases/latest) · [不具合報告・要望](https://github.com/appipinopi/tickreel/issues) · [プライバシー](docs/privacy.md)

Made by [appipinopi](https://github.com/appipinopi) · [公式サイト](https://tickreel.ycccompany.jp/)。アプリ内の開発者表示・リンクには、他のアプリでも再利用できる [App Signature](https://github.com/appipinopi/app-signature) を使用しています。

## 対応OSとダウンロード

| OS | 配布形式 | 必要なもの・対応範囲 |
| --- | --- | --- |
| Windows x64 / x86 | 自動選択インストーラー・CPU別ZIP | Windows 10/11、WebView2 Runtime |
| Windows ARM64 | ARM64専用インストーラー・ZIP | Windows 11 ARM64、WebView2 Runtime |
| macOS Apple Silicon | .app入りZIP（プレビュー） | macOS 12以降、画面収録・マイクの許可 |
| macOS Intel | .app入りZIP（プレビュー） | macOS 12以降、画面収録・マイクの許可 |
| Linux x64 / ARM64 | .deb・.rpm・tar.gz（プレビュー） | Ubuntu 24.04相当以降、X11 / Wayland、GTK 3、WebKitGTK 4.1、ALSA |

ReleasesのAssetsから、自分のOS・CPUに合うパッケージを選んでください。Windowsは `Tickreel-1.0.2-windows-x64_x86.exe` が64bit / 32bit版を自動選択します。ARM64 PCは `Tickreel-1.0.2-windows-arm64.exe` を使ってください。インストール不要のCPU別ZIPも用意しています。ZIPは展開して `Tickreel.exe` を起動します。macOSは展開した `Tickreel.app` をApplicationsへ移動します。Linuxは展開先の `tickreel` を実行します。同梱のFFmpegをアプリと同じ場所に置いたまま使ってください。

Linuxの.debは `sudo apt install ./Tickreel-1.0.2-linux-amd64.deb`（ARM64は `arm64.deb`）、.rpmは `sudo dnf install ./Tickreel-1.0.2-linux-x86_64.rpm`（ARM64は `aarch64.rpm`）で導入します。アプリはメニューまたは `tickreel` コマンドから起動できます。.rpmはglibc 2.39以降とGTK 3 / WebKitGTK 4.1を提供する環境向けで、各ディストリビューションでの実機検証は継続中です。

`*-portable.exe` は更新用のアプリ本体です。初回はインストーラーまたはZIPを使ってください。Windowsの旧v1.0.0を使っている場合は、今回だけ新しい配布物で入れ替えてください。記録データは引き継ぎます。

macOS版はAppleによる公証を行っていないプレビュー版です。macOS・Linuxの実機確認は継続中です。LinuxのWaylandでは「共有する画面を選ぶ」からOSの画面共有ダイアログを開き、記録する画面を許可します。PipeWireとxdg-desktop-portal、お使いのデスクトップに対応したPortalバックエンド（GNOME / KDE / wlr等）が必要です。Waylandではアプリ別の黒塗りと無操作の自動早送りは利用できません。画面共有は「画面共有を終了」またはアプリ終了で解除できます。

Microsoft Store・Snap Storeへの配布は準備中です。Snap名 `tickreel` は登録済みです。試作SnapはGitHub Releasesから取得できますが、Snap Storeへのパッケージ公開はまだ行っていません。

## OBSプラグイン版（v1.0.2）

Windows x64・OBS Studio 31以降向けに `Tickreel-OBS-1.0.2-windows-x64.zip` を追加しました。OBSの「ソース」に **Tickreel Timelapse** を追加し、プロパティからOBSの最終出力をタイムラプス記録できます。停止すると本体v1.0.2のライブラリへ自動で取り込み、テロップ・BGMの編集と書き出しができます。OBSの「録画開始・停止」に自動連動し、画像やロゴは映像に追加しません。

[OBS版の導入・使い方](obs-plugin/README.md)。macOS・Linux・ARM64ネイティブOBS向けのプラグイン配布物は今回含みません。

## 本体でできること

- **ワンクリック生成**：一日の記録をタイムラプスへ。
- **1分 → 約4秒**：15倍速をワンクリックで設定（1秒間隔・15fps・無操作の追加早送りなし）。
- **GUIテロップ編集**：文章を入力し、ドラッグで配置。表示時間・文字サイズ・色も調整。
- **自分の声でアフレコ**：録音し、開始位置と音量を指定して合成。
- **BGMとテーマ**：Calm / Focus、自分の音楽、4種類のテーマ演出。
- **複数モニター**：最大8画面をグリッドまたは横並びで記録。
- **プライバシーの黒塗り**：選んだアプリのウィンドウを保存前に黒塗り。
- **無操作区間の早送り**：マウス・キーボードの無操作区間を追加で圧縮。
- **作業スタッツ**：作業時間・フレーム数・画面数を映像に重ね表示。
- **週間・月間まとめ**：複数日の記録を一本に。不要な記録も削除可能。

無料版の動画・GIFには、末尾に3秒のTickreelエンドカードが入ります。

## はじめかた

1. 制作の種類、記録する画面、キャプチャ間隔を選びます。
2. 必要に応じて黒塗り対象のアプリや追加モニターを選びます。
3. **記録を開始**して、いつもどおり作業します。
4. 記録を終了し、**編集**からテロップ・BGM・アフレコを追加します。
5. 出力形式を選び、**ワンクリック生成**。標準の保存先は、OSの動画フォルダー内の「Tickreel」です（動画フォルダーが未設定の場合は記録フォルダー内）。

テロップは「現在位置に追加」で作成します。編集画面を閉じる前に「編集内容を保存」を押してください。書き出し時にも編集内容を保存します。

## 出力形式

| 種類 | 対応形式 |
| --- | --- |
| 動画 | MP4、MOV、WebM、MKV、AVI |
| アニメーション | GIF（960px・12fps・音声なし） |
| 音声 | WAV、MP3、FLAC、M4A、OGG、AAC |

動画はフルHD、24 / 30 / 60fpsに対応。音声出力は、早送り後の作業動画の長さに合わせたBGMとアフレコのミックスです。声だけを書き出す場合は「BGMなし」を選んでください。WAVは48kHz・16bit・ステレオ、OGGはOpusです。

## 保存とプライバシー

記録した画面・マイク音声・テロップを外部サーバーへ送信しません。キー入力の内容も収集しません。Windows版の更新チェック・取得にはGitHubへのネット接続を使います。

| OS | 記録の標準保存先 |
| --- | --- |
| Windows | `%APPDATA%/TickreelRust/recordings` |
| macOS | `~/Library/Application Support/TickreelRust/recordings` |
| Linux | `~/.local/share/TickreelRust/recordings`（XDG設定に従います） |
| Snap | `$SNAP_USER_COMMON/recordings` |

黒塗りはウィンドウの位置をもとに処理します。瞬間的な移動、通知、特殊な描画を完全に隠せるとは限らないため、共有前に生成結果を確認してください。対象の取得に失敗した場合は記録を止めます。保存済みフレームへの後付け黒塗りは行いません。

## 更新

- **WindowsのGitHub版**：起動時に確認。サイドバーから署名検証付きの更新を取得し、終了時に適用します。
- **macOS・LinuxのGitHub版**：Releasesから新しいパッケージを取得してください。アプリ内の自動差し替えは未対応です。
- **Store版**：公開後は、それぞれのStoreの仕組みで更新します。

## 開発

Rust、各OSの開発ツール、FFmpegを使用します。UIは共通のHTML/CSS/JavaScriptで表示し、画面取得・録音・保存・出力処理はRustが担当します。

```sh
cargo test --locked
cargo run
```

[ビルド手順・動作要件](docs/platforms.md) · [配布・署名付き更新](docs/distribution.md)

不具合報告には、OS・アプリのバージョン・再現手順を添えてください。画像やログを公開する際は、個人情報が含まれていないことを確認してください。
