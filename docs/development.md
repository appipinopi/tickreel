# 開発・既存Windows版の技術資料

一日のPC作業をタイムラプスとして保存するWindows用アプリです。**記録・黒塗り・編集データ保存・マイク録音・動画生成をRustで実装**しています。以前のHTML/CSSと操作画面を再利用し、RustのWry + Windows WebView2で表示します。JavaScriptは表示と操作の橋渡しを担当し、マイク録音はRustのCPALが担当します。画面取得・ウィンドウ検出はWindows API、動画のエンコードは同梱FFmpegです。

## 起動

`release-rust-1.0.1\Tickreel.exe` を起動してください。`ffmpeg.exe` を同じフォルダーに置いたまま使います。配布用ZIPは展開してから起動します。開いている旧版は先に閉じてください。Electron・Node.jsは実行時に不要です。WindowsのWebView2 Runtimeが必要です。

1. 制作の種類（お絵描き / 開発）、記録するモニター、撮影間隔、fpsを選びます。
2. 黒塗りするアプリ、複数画面の並べ方、無操作判定と早送り倍率を選びます。
3. 記録を開始し、作業終了後に記録を終了します。
4. 「ワンクリック生成」で `ビデオ\Tickreel` に選んだ形式で保存します。「編集」からテロップ・テーマ・BGM・アフレコ・スタッツを調整できます。

## 機能

- 1 / 5 / 10 / 30 / 60秒間隔で記録。15 / 24 / 30 / 60fpsのフルHD動画を生成。
- 「1分 → 約4秒（15倍速）」で1秒間隔・15fps・無操作の追加早送りなしを一括選択できます。エンドカードは別途3秒です。
- 最大8モニターをグリッドまたは横並びで同時記録。異なる画面比率には余白を入れます。
- 指定アプリの表示中のウィンドウを、フレームを保存する前に黒塗り。位置取得に失敗すると記録を停止します。
- マウス・キーボードの無操作が1 / 2 / 5分以上の区間を識別し、生成時に追加で4 / 8 / 16倍に圧縮します。倍率はあとから変更できます。
- 日本語テロップ、作業スタッツ（日付・作業時間・無操作時間・フレーム数・画面数）。
- Paper（お絵描き）、Terminal（開発）、Cinema、Cleanの4テーマ。タイトルのフェード、配色、フレーム装飾、レターボックスなど。
- ローカルで生成するCalm / Focus BGM。自動選択、音量調整、自分の音楽ファイルも利用可能（300MBまで）。
- Rustのマイク録音（最大10分・100MB）。開始位置と音量を指定して動画に合成。録音の再生・削除・再録音も可能です。
- 週間（月曜〜日曜）・月間のまとめを時系列で生成。異なるfpsの記録も長さを保って結合します。
- 過去の記録を削除。生成済みファイルは残します。
- 無料版はすべての動画の最後にTickreelのエンドカードを3秒表示。有料プラン・決済・ライセンス機能は未実装です。

テロップは編集画面の「現在位置に追加」から作成します。文章・表示時間・文字サイズ・色を調整し、プレビュー上でドラッグして配置します。時間は無操作を圧縮した後の動画に合わせます。動画本編より長いアフレコは本編の終わりで切ります。

プレビューは保存フレームを時間に合わせて切り替え、テロップと録音済みのアフレコを確認できます。BGM・フェードなどの最終確認は生成した動画で行ってください。

## 保存・旧版の取り込み

Rust版の保存先は `%APPDATA%\TickreelRust\recordings` です。画面・音声の外部送信はしません。記録中は最小化できます。画面ロック・スリープ復帰時には一時停止し、解除後に再開ボタンを押します。アプリを閉じる際は記録を保存します。異常終了時も最後に保存できたフレームを復元します。

起動時に旧版の `%APPDATA%\tickreel\recordings` から、保存済みの記録をコピーして取り込みます。元データは残します。記録中・一時停止中の旧版セッションは取り込みません。旧版で記録を終了し、Rust版を再起動すると取り込めます。取り込み済みの同じIDは重複コピーしません。

黒塗りはWindowsのウィンドウ位置を撮影前後に取得して適用します。瞬間的な移動、特殊な描画、別プロセスの通知などの完全な遮蔽は保証できません。公開前に動画を確認してください。個人情報の文字認識や、過去の未保護フレームへの後付け黒塗りは行いません。無操作時間は撮影間隔ごとの判定から推定し、ビルド・描画待ちも含みます。キー入力の内容は収集しません。

## 開発・ビルド

Windows x64とRustのMSVCツールチェーンを使用します。

```powershell
cargo run
cargo test
cargo run -- --self-test
cargo build --release
powershell -ExecutionPolicy Bypass -File scripts/build-rust.ps1 -FfmpegPath C:\tools\ffmpeg.exe
```

ビルドスクリプトは `release-rust-1.0.1` にアプリ、FFmpeg、説明・ライセンスを配置し、`Tickreel-1.0.1-windows-x64.zip` を作ります。ビルド先のアプリを閉じてから実行してください。開発時は `TICKREEL_FFMPEG` 環境変数でFFmpegのパスを指定できます。初回の依存ライブラリ取得にはネットワークが必要です。

`--self-test` はWindowsの画面取得をメモリ上で検証し、合成画像とテスト音声で黒塗り・動画生成・音声合成・エンドカード・期間まとめ・データ取り込み・削除を確認します。実際の画面画像や会話をテスト用記録に保存しません。`--ui-test` / `--ui-test-editor` は隔離した合成画面・音声で実際のUIボタンからRust処理を検証し、WebView2の描画を直接取得して確認画像を保存します。

実マイクの音質、長時間の連続記録、混在DPI環境の網羅確認は未実施です。ソフトは未署名です。

## ソース構成

- `frontend/`: 元のHTML/CSS・操作画面とRustへの橋渡し
- `src/web_ui.rs`: Rustのウィンドウ、WebView2、コマンド接続、音声プレビュー
- `src/ui.rs`: 旧egui画面（参照用。通常は使用しません）
- `src/native.rs`: Windowsのモニター・ウィンドウ検出、画面取得、黒塗り
- `src/engine.rs`: 記録・一時停止・復元・バックグラウンド生成
- `src/store.rs`: データ保存、旧版取り込み、安全な削除
- `src/export.rs`: テロップ・スタッツ・テーマ・動画・音声の合成
- `src/audio.rs`: マイク録音、音声再生、内蔵BGM生成
- `src/model.rs`: データ形式、無操作圧縮、期間集計
- `legacy-electron`: 旧実装の保存用。Rust版の実行には使用しません。

参考: [eframe](https://docs.rs/eframe/0.33.3/eframe/)、[CPAL](https://docs.rs/cpal/0.16.0/cpal/)、[EnumDisplayMonitors](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-enumdisplaymonitors)、[FFmpeg filters](https://www.ffmpeg.org/ffmpeg-filters.html)。

## v1.0 の編集・出力

動画: MP4 / MOV / WebM / MKV / AVI。アニメーション: GIF（960px・12fps・音声なし）。音声: WAV（48kHz・16bit・ステレオ）/ MP3 / FLAC / M4A / OGG（Opus）/ AAC。ライブラリの各記録、編集画面、週間・月間のまとめで形式を選べます。標準の保存先はビデオ/Tickreelです。

テロップは「現在位置に追加」で作成し、文章・開始/終了秒・スライダー・文字サイズ・色をGUIで調整できます。プレビューの文字をドラッグすると位置が変わります。編集内容を保存してから閉じてください。書き出し時にも自動保存します。以前のテロップはそのまま読み込めます。

音声出力にはBGMとアフレコのミックスを収録します。長さは早送り後の作業動画に合わせ、動画の3秒エンドカードは含めません。アフレコだけを出力したい場合は「BGMなし」を選びます。BGMもアフレコもない場合は音声出力できません。動画・GIFには無料版のエンドカードが入ります。

## 公開・自動更新

ソースと配布: https://github.com/appipinopi/tickreel

GitHub版は起動時にGitHub Releasesへ更新の有無を問い合わせます。画面・録音・記録データは送信しません。公開リリースのバージョン情報取得と更新EXEのダウンロードにネット接続を使います。サイドバーから更新を確認・ダウンロードでき、終了時に署名検証済みの新版へ差し替えます。Store版はMicrosoft Storeが更新を管理します。

配布方法・署名鍵・Storeパッケージ作成手順は [docs/distribution.md](docs/distribution.md) を参照してください。


## v1.0.1: Waylandの画面共有

LinuxのWaylandセッション（`XDG_SESSION_TYPE=wayland` または `WAYLAND_DISPLAY`）では、ScreenCast Portalで許可されたPipeWireストリームを使用します。Xwaylandの有無には依存しません。「共有する画面を選ぶ」で選択し、許可された画面だけをプレビュー・記録します。選択のキャンセル時には既存の共有を維持します。共有は「画面共有を終了」またはアプリ終了で解除します。許可を取り消した場合・ストリームの停止時には、古いフレームを保存せず記録を一時停止します。

- 必須: PipeWire、xdg-desktop-portal、デスクトップに対応したScreenCastバックエンド。GNOMEは `xdg-desktop-portal-gnome`、KDEは `xdg-desktop-portal-kde`、wlroots系は `xdg-desktop-portal-wlr` 等。共有画面数はバックエンドの対応範囲で最大8枚です。
- Waylandは他アプリのウィンドウ位置や全体の入力情報を提供しないため、アプリ別黒塗りと無操作の自動早送りを無効にします。以前の黒塗り設定を指定した記録要求も拒否します。
- フレームはメモリ上で最新の1枚だけ保持します。ピクセルサイズはPipeWireの交渉結果を使用し、行のパディング・オフセット・RGB/BGR変換を処理します。
- 検証: `cargo test --locked`、`bash scripts/test-wayland.sh target/debug/tickreel-rust`。後者は専用のheadless SwayとD-Busセッション内で実際のPortal許可・PipeWire画像取得・共有解除を確認し、普段の画面を記録しません。
- GNOME / KDE、複数モニター、HDR、各ディストリビューションでの実機確認は別途必要です。

参照: [ScreenCast Portal仕様](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html)。
