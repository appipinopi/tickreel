param(
  [Parameter(Mandatory=$true)][string]$IdentityName,
  [Parameter(Mandatory=$true)][string]$Publisher,
  [Parameter(Mandatory=$true)][string]$PublisherDisplayName,
  [string]$FfmpegPath = ''
)
$ErrorActionPreference = 'Stop'
$taskWorkspace = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Push-Location -LiteralPath $taskWorkspace
$taskPreviousChannel = $env:TICKREEL_CHANNEL
try {
  # Use the exact identity from Partner Center > Product identity.
  $env:TICKREEL_CHANNEL = 'store'
  & (Join-Path $PSScriptRoot 'build-rust.ps1') -FfmpegPath $FfmpegPath -Channel store
  if ($LASTEXITCODE -ne 0) { throw 'Store build failed.' }
  $taskMetadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
  $taskVersion = ($taskMetadata.packages | Where-Object name -EQ 'tickreel-rust').version
  $taskInput = Join-Path $taskWorkspace ('release-store-' + $taskVersion)
  $taskStage = Join-Path $taskWorkspace ('dist\store-' + [guid]::NewGuid().ToString('N'))
  New-Item -ItemType Directory -Path $taskStage -Force | Out-Null
  Copy-Item -LiteralPath (Join-Path $taskInput 'Tickreel-Rust.exe'), (Join-Path $taskInput 'ffmpeg.exe'), (Join-Path $taskInput 'README.md') -Destination $taskStage
  Copy-Item -LiteralPath (Join-Path $taskInput 'licenses') -Destination $taskStage -Recurse
  $taskAssets = Join-Path $taskStage 'Assets'
  New-Item -ItemType Directory -Path $taskAssets -Force | Out-Null
  Add-Type -AssemblyName System.Drawing
  foreach ($taskSize in @(44,50,150)) {
    $taskBitmap = New-Object System.Drawing.Bitmap($taskSize,$taskSize)
    $taskGraphics = [System.Drawing.Graphics]::FromImage($taskBitmap)
    $taskGraphics.Clear([System.Drawing.Color]::FromArgb(20,123,112))
    $taskFont = New-Object System.Drawing.Font('Segoe UI',($taskSize * 0.50),[System.Drawing.FontStyle]::Bold,[System.Drawing.GraphicsUnit]::Pixel)
    $taskFormat = New-Object System.Drawing.StringFormat
    $taskFormat.Alignment = [System.Drawing.StringAlignment]::Center
    $taskFormat.LineAlignment = [System.Drawing.StringAlignment]::Center
    $taskGraphics.DrawString('T',$taskFont,[System.Drawing.Brushes]::White,(New-Object System.Drawing.RectangleF(0,0,$taskSize,$taskSize)),$taskFormat)
    $taskBitmap.Save((Join-Path $taskAssets ('Logo' + $taskSize + '.png')),[System.Drawing.Imaging.ImageFormat]::Png)
    $taskFormat.Dispose(); $taskFont.Dispose(); $taskGraphics.Dispose(); $taskBitmap.Dispose()
  }
  $taskName = [System.Security.SecurityElement]::Escape($IdentityName)
  $taskPublisher = [System.Security.SecurityElement]::Escape($Publisher)
  $taskDisplay = [System.Security.SecurityElement]::Escape($PublisherDisplayName)
  $taskXml = @"
<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10" xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10" xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities" IgnorableNamespaces="uap rescap">
  <Identity Name="$taskName" Publisher="$taskPublisher" Version="$taskVersion.0" ProcessorArchitecture="x64" />
  <Properties><DisplayName>Tickreel</DisplayName><PublisherDisplayName>$taskDisplay</PublisherDisplayName><Logo>Assets\Logo50.png</Logo></Properties>
  <Dependencies><TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.19041.0" MaxVersionTested="10.0.26100.0" /></Dependencies>
  <Resources><Resource Language="ja-JP" /></Resources>
  <Applications><Application Id="Tickreel" Executable="Tickreel-Rust.exe" EntryPoint="Windows.FullTrustApplication"><uap:VisualElements DisplayName="Tickreel" Description="一日の作業をタイムラプスに" BackgroundColor="#147b70" Square150x150Logo="Assets\Logo150.png" Square44x44Logo="Assets\Logo44.png" /></Application></Applications>
  <Capabilities><rescap:Capability Name="runFullTrust" /><DeviceCapability Name="microphone" /></Capabilities>
</Package>
"@
  [System.IO.File]::WriteAllText((Join-Path $taskStage 'AppxManifest.xml'),$taskXml,(New-Object System.Text.UTF8Encoding($false)))
  $taskSdk = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin' -Filter makeappx.exe -Recurse | Where-Object FullName -Match '\\x64\\makeappx.exe$' | Sort-Object FullName -Descending | Select-Object -First 1
  if (-not $taskSdk) { throw 'Windows SDK (MakeAppx.exe) is required.' }
  $taskPackage = Join-Path $taskWorkspace ('dist\Tickreel-Store-' + $taskVersion + '.msix')
  & $taskSdk.FullName pack /d $taskStage /p $taskPackage /o
  if ($LASTEXITCODE -ne 0) { throw 'MSIX packaging failed.' }
  Write-Output $taskPackage
} finally { $env:TICKREEL_CHANNEL = $taskPreviousChannel; Pop-Location }
