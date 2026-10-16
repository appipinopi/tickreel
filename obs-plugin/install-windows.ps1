param([string]$ObsDirectory = "$env:ProgramFiles\obs-studio")
$ErrorActionPreference = 'Stop'
$taskObsRoot = [System.IO.Path]::GetFullPath($ObsDirectory)
if (-not (Test-Path -LiteralPath (Join-Path $taskObsRoot 'bin\64bit\obs64.exe'))) {
    throw '64-bit OBS Studio was not found. Specify -ObsDirectory with your OBS installation or portable folder.'
}
if (Get-Process obs64 -ErrorAction SilentlyContinue) { throw 'Close OBS Studio before installing.' }
$taskDestination = Join-Path $taskObsRoot 'obs-plugins\64bit'
New-Item -ItemType Directory -Path $taskDestination -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'obs-plugins\64bit\tickreel-obs.dll') -Destination (Join-Path $taskDestination 'tickreel-obs.dll') -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'obs-plugins\64bit\tickreel-gallery.exe') -Destination (Join-Path $taskDestination 'tickreel-gallery.exe') -Force
Write-Host 'Installed Tickreel OBS. Open OBS, add the Tickreel Timelapse source, then open its properties.'
