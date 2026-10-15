param([string]$FfmpegPath = '', [ValidateSet('github','store')][string]$Channel = 'github')
$ErrorActionPreference = 'Stop'
$taskWorkspace = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Push-Location -LiteralPath $taskWorkspace
$taskPreviousChannel = $env:TICKREEL_CHANNEL
try {
  $env:TICKREEL_CHANNEL = $Channel
  cargo build --release --locked
  if ($LASTEXITCODE -ne 0) { throw 'Rust build failed.' }
  $taskMetadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
  $taskBinary = Join-Path $taskMetadata.target_directory 'release\tickreel-rust.exe'
  $taskVersion = ($taskMetadata.packages | Where-Object name -EQ 'tickreel-rust').version
  $taskOutput = [System.IO.Path]::GetFullPath((Join-Path $taskWorkspace ($(if ($Channel -eq 'store') { 'release-store-' } else { 'release-rust-' }) + $taskVersion)))
  if (-not $taskOutput.StartsWith($taskWorkspace + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) { throw 'Invalid output directory.' }
  New-Item -ItemType Directory -Path $taskOutput -Force | Out-Null
  if (-not $FfmpegPath) {
    foreach ($taskCandidate in @((Join-Path $taskWorkspace 'node_modules\ffmpeg-static\ffmpeg.exe'), (Join-Path $taskOutput 'ffmpeg.exe'), (Join-Path $taskWorkspace 'release-rust\ffmpeg.exe'))) {
      if (Test-Path -LiteralPath $taskCandidate -PathType Leaf) { $FfmpegPath = $taskCandidate; break }
    }
  }
  if (-not $FfmpegPath) { throw 'Provide -FfmpegPath pointing to ffmpeg.exe.' }
  $taskFfmpeg = (Get-Item -LiteralPath $FfmpegPath).FullName
  # Remove the previous public executable name when reusing a build directory.
  $taskLegacyExecutable = Join-Path $taskOutput 'Tickreel-Rust.exe'
  if (Test-Path -LiteralPath $taskLegacyExecutable -PathType Leaf) { Remove-Item -LiteralPath $taskLegacyExecutable -Force }
  Copy-Item -LiteralPath $taskBinary -Destination (Join-Path $taskOutput 'Tickreel.exe') -Force
  if ($taskFfmpeg -ne (Join-Path $taskOutput 'ffmpeg.exe')) { Copy-Item -LiteralPath $taskFfmpeg -Destination (Join-Path $taskOutput 'ffmpeg.exe') -Force }
  Copy-Item -LiteralPath (Join-Path $taskWorkspace 'README.md') -Destination $taskOutput -Force
  New-Item -ItemType Directory -Path (Join-Path $taskOutput 'licenses') -Force | Out-Null
  foreach ($taskLicense in @($taskFfmpeg + '.LICENSE', $taskFfmpeg + '.README')) {
    if (Test-Path -LiteralPath $taskLicense) { Copy-Item -LiteralPath $taskLicense -Destination (Join-Path $taskOutput 'licenses') -Force }
  }
  $taskDependencies = cargo metadata --format-version 1 --filter-platform x86_64-pc-windows-msvc | ConvertFrom-Json
  $taskDependencies.packages | Select-Object name,version,license,repository | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $taskOutput 'licenses\rust-dependencies.json') -Encoding utf8
  foreach ($taskPackage in $taskDependencies.packages) {
    $taskPackageDirectory = Split-Path -Parent $taskPackage.manifest_path
    foreach ($taskLicenseFile in (Get-ChildItem -LiteralPath $taskPackageDirectory -File | Where-Object Name -Match '^(LICENSE|COPYING|NOTICE)([-.]|$)')) {
      $taskLicenseDirectory = Join-Path $taskOutput ('licenses\' + $taskPackage.name + '-' + $taskPackage.version)
      New-Item -ItemType Directory -Path $taskLicenseDirectory -Force | Out-Null
      Copy-Item -LiteralPath $taskLicenseFile.FullName -Destination $taskLicenseDirectory -Force
    }
  }
  Get-ChildItem -LiteralPath $taskOutput -Recurse -File | Where-Object { $_.LastWriteTime.Year -lt 1980 -or $_.LastWriteTime.Year -gt 2107 } | ForEach-Object { $_.LastWriteTime = [DateTime]::Now }
  Compress-Archive -LiteralPath $taskOutput -DestinationPath (Join-Path $taskWorkspace ($(if ($Channel -eq 'store') { 'release-store-' } else { 'Tickreel-' }) + $taskVersion + $(if ($Channel -eq 'github') { '-windows-x64' } else { '' }) + '.zip')) -CompressionLevel Fastest -Force
  Write-Output (Join-Path $taskOutput 'Tickreel.exe')
} finally { $env:TICKREEL_CHANNEL = $taskPreviousChannel; Pop-Location }
