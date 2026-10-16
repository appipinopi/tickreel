# Tickreel OBS v1.0.2

OBS Studioの最終出力（Program）を一定間隔でJPEGに記録するネイティブプラグインです。シーン切り替え・画像・テキスト・カメラを含むOBSの合成結果を記録します。配信やOBSの通常録画を開始する必要はありません。

## 対応環境

Windows 10/11 x64、OBS Studio 31以降の64bit版。OBS 32.2.2で読み込み・保存・停止・再開始・ソース削除時の終了を検証しています。ARM64ネイティブOBS、macOS、Linuxのプラグイン配布物は今回含みません。

## インストール

1. OBS Studioを終了します。
2. `Tickreel-OBS-1.0.2-windows-x64.zip` を展開します。
3. ZIP内の `obs-plugins` フォルダーをOBSのインストール先（通常 `C:\Program Files\obs-studio`）へコピーし、既存のフォルダーへ結合します。必要に応じてWindowsの管理者許可を承認します。
4. OBSを起動します。**ソース → ＋ → Tickreel Timelapse** が表示されれば導入完了です。

PowerShellでも導入できます。OBSのフォルダーへの書き込み権限が必要です。標準のProgram Files配下へ導入する場合は管理者として開いて実行してください。

```powershell
powershell -ExecutionPolicy Bypass -File .\install-windows.ps1
# ポータブル版・独自のインストール先
powershell -ExecutionPolicy Bypass -File .\install-windows.ps1 -ObsDirectory 'D:\OBS'
```

削除する場合はOBSを終了し、導入先の `obs-plugins\64bit\tickreel-obs.dll` を削除します。記録データは残ります。

## 使い方

1. **Tickreel Timelapse** ソースを1個追加し、プロパティを開きます。このソース自体は映像に表示されません。
2. タイトル、撮影間隔（1 / 5 / 10 / 30 / 60秒）、書き出しfps（15 / 24 / 30 / 60）を選びます。初期値は1秒間隔・15fpsで、1分の作業が約4秒になります（本体で書き出すエンドカードは別途3秒）。
3. **記録を開始**を押します。記録開始は手動で、OBS起動時に自動再開しません。
4. **停止して保存**を押します。ソース削除・OBSの正常終了でも保存を終了します。
5. Tickreel **v1.0.2以降**を開きます。ライブラリに記録が自動で取り込まれ、テロップ・BGM・アフレコの編集と動画/GIF書き出しができます。本体を開いたままの場合も、次のライブラリ更新で取り込みます。

記録するのは常に **OBSのProgram全体**です。スタジオモードのPreviewや、このソースを置いた特定のシーンだけではありません。ソースを非表示にしても記録は停止しません。別シーンへの切り替え後も記録が続くので、停止は必ずプロパティから行ってください。

## 保存と制限

- 保存先は `%APPDATA%\TickreelRust\obs-recordings`。完了した記録を本体が `%APPDATA%\TickreelRust\recordings` へ移動します。本体とプラグインを同じWindowsユーザーで使ってください。Store版との連携は未検証です。
- 記録中のファイルは本体に取り込みません。同時に記録できるTickreel OBSソースは1個です。
- OBSの出力解像度で保存します。保存が追いつかない場合はフレームを間引き、OBSの映像処理を待たせません。ディスクエラー時は保存を中断します。プロパティを開き直すと状態を確認できます。
- 音声の同時収録・無操作の自動早送り・Tickreel本体のアプリ別黒塗りは適用しません。OBS側で見せたいソースやマスクを設定してください。HDRはSDRへ変換して保存します。
- 異常終了した記録は、次回プラグインで記録を開始した際に中断済みとして回復します。
- 画面データを外部へ送信しません。通常録画とは別にJPEGを保存するため、長時間の記録では空き容量を確保してください。

## 開発

```powershell
cargo test --locked --manifest-path obs-plugin/Cargo.toml
cargo build --release --locked --manifest-path obs-plugin/Cargo.toml
python scripts/package-obs-plugin.py --binary <target-directory>/release/tickreel_obs.dll
python scripts/test-obs-plugin.py <portable-OBS-root> <target-directory>/release/tickreel_obs.dll
```

Rustのみでビルドでき、OBS SDK・Qt・FFmpegの同梱は不要です。実行中のOBSが提供するlibobsのC ABIを使用します。最小ABI定義の参照元は [OBS 31.0.0のヘッダー](https://github.com/obsproject/obs-studio/tree/31.0.0/libobs) です。

プラグインと同梱の共有RustソースはGPL-2.0-or-laterです。対応ソースは `Tickreel-OBS-1.0.2-source.zip`、依存ライブラリのライセンスは配布ZIPの `licenses` に含みます。
