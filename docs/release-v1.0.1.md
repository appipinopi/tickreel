Tickreel v1.0.1

## 変更点

- **1分の作業 → 約4秒**のプリセットを追加しました。1秒間隔・15fps・無操作の追加早送りなしをワンクリックで選べます。末尾のエンドカードは別途3秒です。
- **Windowsのアプリアイコンを修正**しました。EXE、ウィンドウ、タスクバー、インストーラーにTickreelのアイコンを反映します。
- **LinuxのWayland記録に対応**しました。「共有する画面を選ぶ」からOSの画面共有ダイアログを開き、許可した画面をPipeWire経由で記録します。画面の再選択・共有終了に対応し、共有停止時には古い画像の保存を止めます。

WaylandにはPipeWire・xdg-desktop-portal・デスクトップに対応したScreenCastバックエンド（GNOME / KDE / wlr等）が必要です。共有できる画面数はバックエンドの対応範囲で最大8枚です。Waylandではアプリ別黒塗りと無操作の自動早送りは利用できません。Xwaylandなしのheadless Swayで画面取得・記録保存・再選択・共有停止・GTK/WebKitの画面起動を検証しています。GNOME / KDEや実機の複数モニター・HDR環境は追加確認が必要です。

## ダウンロード

| 環境 | 配布物 |
| --- | --- |
| Windows 10/11 x64・x86 | `Tickreel-1.0.1-windows-x64_x86.exe`（CPUを自動選択） |
| Windows 11 ARM64 | `Tickreel-1.0.1-windows-arm64.exe` |
| Linux x64 | `Tickreel-1.0.1-linux-amd64.deb` / `Tickreel-1.0.1-linux-x86_64.rpm` / tar.gz |
| Linux ARM64 | `Tickreel-1.0.1-linux-arm64.deb` / `Tickreel-1.0.1-linux-aarch64.rpm` / tar.gz |
| macOS Apple Silicon / Intel | `Tickreel-1.0.1-macos-arm64.zip` / `Tickreel-1.0.1-macos-x86_64.zip` |

WindowsはWebView2 Runtimeが必要です。CPU別のZIPも利用できます。ZIPは展開し、FFmpegを同じフォルダーに置いたまま使用してください。`*-portable.exe` と `update-windows-*.json` はCPU別のアプリ内更新用です。初回はインストーラーかZIPを使用してください。

WindowsのGitHub版は、サイドバーから更新を確認・ダウンロードし、通常終了時に新版へ切り替えられます。更新データはEd25519署名とSHA-256で検証します。記録データは引き継ぎます。

macOS・Linuxはプレビュー版です。macOSは未公証、LinuxのリリースビルドはUbuntu 24.04相当以降のランタイム向けです。Windows版のAuthenticodeコード署名は行っていません。
