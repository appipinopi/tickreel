param([ValidateSet('x64','x86','arm64')][string]$Architecture = 'x64')
$ErrorActionPreference = 'Stop'
$taskTarget = @{ x64='x86_64-pc-windows-msvc'; x86='i686-pc-windows-msvc'; arm64='aarch64-pc-windows-msvc' }[$Architecture]
rustup target add $taskTarget
if ($LASTEXITCODE -ne 0) { throw 'Target setup failed.' }
cargo test --locked --target $taskTarget
if ($LASTEXITCODE -ne 0) { throw 'Tests failed.' }
cargo build --release --locked --target $taskTarget
if ($LASTEXITCODE -ne 0) { throw 'Build failed.' }
$taskMetadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
$taskVersion = ($taskMetadata.packages | Where-Object name -EQ 'tickreel-rust').version
$taskRoot = Join-Path (Get-Location) "dist/windows-$Architecture"
New-Item -ItemType Directory -Path $taskRoot -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $taskMetadata.target_directory "$taskTarget/release/tickreel-rust.exe") -Destination "$taskRoot/Tickreel.exe" -Force
python scripts/prepare-windows-ffmpeg.py $Architecture $taskRoot
if ($LASTEXITCODE -ne 0) { throw 'FFmpeg preparation failed.' }
python scripts/collect-licenses.py "$taskRoot/licenses"
if ($LASTEXITCODE -ne 0) { throw 'License collection failed.' }
Copy-Item README.md "$taskRoot/README.md" -Force
$taskPublicExe = "dist/Tickreel-$taskVersion-windows-$Architecture-portable.exe"
Copy-Item "$taskRoot/Tickreel.exe" $taskPublicExe -Force
python scripts/sign-update.py $taskPublicExe $taskVersion "dist/update-windows-$Architecture.json" --arch $Architecture
if ($LASTEXITCODE -ne 0) { throw 'Signing failed.' }
Compress-Archive -LiteralPath $taskRoot -DestinationPath "dist/Tickreel-$taskVersion-windows-$Architecture.zip" -Force
