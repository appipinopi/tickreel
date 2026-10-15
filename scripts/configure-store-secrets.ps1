param(
  [string]$SnapCredentialsFile = '',
  [string]$TenantId = '',
  [string]$ClientId = '',
  [switch]$MicrosoftStore
)
$ErrorActionPreference = 'Stop'
function Set-RepositorySecret([string]$Name, [string]$Value) {
  if ([string]::IsNullOrWhiteSpace($Value)) { throw "Missing value for $Name" }
  $Value | & gh secret set $Name --repo appipinopi/tickreel
  if ($LASTEXITCODE -ne 0) { throw "Could not save $Name" }
}
if ($SnapCredentialsFile) {
  $taskCredentials = Get-Content -LiteralPath $SnapCredentialsFile -Raw
  Set-RepositorySecret 'SNAPCRAFT_STORE_CREDENTIALS' $taskCredentials
  $taskCredentials = $null
}
if ($MicrosoftStore) {
  if (-not $TenantId) { $TenantId = Read-Host 'Microsoft Entra tenant ID' }
  if (-not $ClientId) { $ClientId = Read-Host 'Partner Center application client ID' }
  $taskSecret = Read-Host 'Client secret (hidden)' -AsSecureString
  $taskPointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($taskSecret)
  try {
    Set-RepositorySecret 'MS_STORE_TENANT_ID' $TenantId
    Set-RepositorySecret 'MS_STORE_CLIENT_ID' $ClientId
    Set-RepositorySecret 'MS_STORE_CLIENT_SECRET' ([Runtime.InteropServices.Marshal]::PtrToStringBSTR($taskPointer))
  } finally {
    [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($taskPointer)
    $taskSecret.Dispose()
  }
}
if (-not $SnapCredentialsFile -and -not $MicrosoftStore) { throw 'Specify -SnapCredentialsFile or -MicrosoftStore.' }
Write-Output 'Repository secrets saved. Secret values were not printed.'
