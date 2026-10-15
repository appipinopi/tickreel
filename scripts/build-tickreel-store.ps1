param([string]$FfmpegPath = '')
$ErrorActionPreference = 'Stop'
$taskIdentityPath = Join-Path $PSScriptRoot '..\packaging\store-identity.json'
$taskIdentity = Get-Content -LiteralPath $taskIdentityPath -Raw -Encoding utf8 | ConvertFrom-Json
& (Join-Path $PSScriptRoot 'build-store.ps1') -IdentityName $taskIdentity.identityName -Publisher $taskIdentity.publisher -PublisherDisplayName $taskIdentity.publisherDisplayName -FfmpegPath $FfmpegPath
