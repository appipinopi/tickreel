# ビルドとプラットフォーム

## Windows

Rust MSVC、WebView2 Runtime、FFmpegが必要です。

```powershell
cargo test --locked
./scripts/build-rust.ps1 -FfmpegPath C:\tools\ffmpeg.exe
```

## macOS

Apple Silicon / Intelをネイティブビルドします。Xcode Command Line Tools、Rust、Node.js、Python 3.11以降が必要です。

```sh
cargo test --locked
npm install --no-save --package-lock=false ffmpeg-static@5.3.0
bash scripts/build-unix.sh
```

`dist`に`.app`入りZIPを生成します。マイクの利用目的をInfo.plistに含め、画面収録とマイクはOS設定で許可します。現在はアドホック署名のプレビューです。Developer ID署名・Apple公証は未設定です。GUI・画面収録・マイクの実機検証が別途必要です。

Retinaのキャプチャは、ウィンドウ位置と一致する論理座標に正規化してから黒塗りします。

## Linux

最初の対応はX11セッションです。Waylandの権限・セッション管理は未実装のため、Waylandでは記録を開始しません。

Ubuntu 24.04以降での開発依存:

```sh
sudo apt-get install libgtk-3-dev libwebkit2gtk-4.1-dev libasound2-dev libxdo-dev libxrandr-dev libxss-dev libgbm-dev libclang-dev libdbus-1-dev libpipewire-0.3-dev libxcb-randr0-dev pkg-config build-essential xvfb fonts-noto-cjk
cargo test --locked
npm install --no-save --package-lock=false ffmpeg-static@5.3.0
bash scripts/build-unix.sh
```

tar.gzはGTK 3・WebKitGTK 4.1・ALSA・X11関連の実行時ライブラリを利用します。依存なしの形式ではありません。リリース用はUbuntu 24.04上でビルドします。Linuxのロック検知はX11 Screen Saver拡張に基づき、デスクトップ環境ごとの連動は実機での確認が必要です。macOS・Linuxでの自動スリープ抑止は未実装です。

## Snap

publisher `appipinopi` に名前 `tickreel` を登録済みです。公開リビジョンはまだありません。`snap/snapcraft.yaml`を用意しています。strict confinement / core24 / GNOME extension / amd64が最初の構成です。GitHub Releasesの検証済みLinuxパッケージをSHA-256で照合して取り込みます。新版を配信する際は、source URL・checksum・versionを一緒に更新してください。

```sh
snapcraft
snapcraft login
snapcraft upload tickreel_1.0.0_amd64.snap --release=edge
```

実機でのインストールとプラグ接続を確認するまではstableへ公開しません。録音には `sudo snap connect tickreel:audio-record` が必要です。更新はsnapdが管理します。

## GitHub Actions

`unix.yml`はLinux x64、macOS arm64、macOS Intelをビルドします。手動実行の`release_tag`で既存Releaseへ追加できます。空欄ではArtifactsのみ生成します。既存の同名ファイルは上書きしません。Windows向け署名付き更新情報はWindows専用のまま分離しています。
