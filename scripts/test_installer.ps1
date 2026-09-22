#Requires -Version 5.1
# Run on disposable Windows CI hosts; restore process and User PATH after testing.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$installer = Join-Path (Split-Path $PSScriptRoot) 'install.ps1'
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ('tileboard-tests-' + [guid]::NewGuid().ToString('N'))
$originalUserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$originalProcessPath = $env:Path
$originalArchitecture = $env:PROCESSOR_ARCHITECTURE
$originalNativeArchitecture = $env:PROCESSOR_ARCHITEW6432
$originalTls = [Net.ServicePointManager]::SecurityProtocol
$binDirectory = Join-Path $testRoot "bin with spaces and ' [brackets]"
$fixtureDirectory = Join-Path $testRoot 'fixtures'
$testFailure = ''
$defaultDirectory = $null

function Assert-True($Condition, [string] $Message) {
    if (-not $Condition) { throw "Assertion failed: $Message" }
}
function Assert-InstallFails([scriptblock] $Action, [string] $Message) {
    $failed = $false
    try { & $Action } catch { $failed = $true; Write-Host "Expected failure: $($_.Exception.Message)" }
    Assert-True $failed $Message
}

try {
    [IO.Directory]::CreateDirectory($fixtureDirectory) | Out-Null
    [Net.ServicePointManager]::SecurityProtocol = $originalTls -bor [Net.SecurityProtocolType]::Tls12
    $archiveName = 'tileboard-v0.1.0-x86_64-pc-windows-msvc.zip'
    foreach ($name in @($archiveName, 'SHA256SUMS')) {
        Invoke-WebRequest -UseBasicParsing -Uri "https://github.com/LimePencil/tileboard/releases/download/v0.1.0/$name" -OutFile (Join-Path $fixtureDirectory $name)
    }
    $originalManifest = [IO.File]::ReadAllText((Join-Path $fixtureDirectory 'SHA256SUMS'))

    # Mock only the network boundary; hash, unzip, binary execution, and PATH changes are real.
    function Invoke-WebRequest {
        param($Uri, $OutFile, $TimeoutSec, [switch] $UseBasicParsing)
        if ($testFailure -eq 'network') { throw 'Simulated download failure' }
        [IO.File]::Copy((Join-Path $fixtureDirectory ([uri]$Uri).Segments[-1]), $OutFile, $true)
    }
    function Invoke-RestMethod {
        param($Uri, $TimeoutSec, [switch] $UseBasicParsing)
        return [pscustomobject]@{ tag_name = 'v0.1.0' }
    }

    $tlsBefore = [Net.ServicePointManager]::SecurityProtocol
    & ([scriptblock]::Create([IO.File]::ReadAllText($installer))) -BinDir $binDirectory -NoModifyPath
    Assert-True ([Environment]::GetEnvironmentVariable('Path', 'User') -ceq $originalUserPath) 'NoModifyPath preserves User PATH'
    Assert-True ($env:Path -ceq $originalProcessPath) 'NoModifyPath preserves session PATH'
    Assert-True ([Net.ServicePointManager]::SecurityProtocol -eq $tlsBefore) 'TLS settings are restored'
    Assert-True ($ErrorActionPreference -eq 'Stop') 'caller preferences are preserved'
    $binary = Join-Path $binDirectory 'tileboard.exe'
    Assert-True ((& $binary --version) -eq 'tileboard 0.1.0') 'installed binary runs'

    & $installer -BinDir $binDirectory -Version v0.1.0
    & $installer -BinDir $binDirectory -Version 0.1.0
    $userEntries = @([Environment]::GetEnvironmentVariable('Path', 'User') -split ';' | Where-Object { $_ -eq $binDirectory })
    $processEntries = @($env:Path -split ';' | Where-Object { $_ -eq $binDirectory })
    Assert-True ($userEntries.Count -eq 1) 'User PATH is idempotent'
    Assert-True ($processEntries.Count -eq 1) 'process PATH is idempotent'
    Assert-True ((Get-Command tileboard).Source -eq $binary) 'tileboard is immediately on PATH'
    $installedHash = (Get-FileHash -LiteralPath $binary).Hash
    $installedPath = $env:Path

    foreach ($manifest in @('', ($originalManifest + $originalManifest), ($originalManifest -replace '[0-9a-f]{64}', ('0' * 64)))) {
        [IO.File]::WriteAllText((Join-Path $fixtureDirectory 'SHA256SUMS'), $manifest)
        Assert-InstallFails { & $installer -BinDir $binDirectory } 'invalid checksum is rejected'
        Assert-True ((Get-FileHash -LiteralPath $binary).Hash -eq $installedHash) 'failure preserves existing binary'
        Assert-True ($env:Path -ceq $installedPath) 'failure preserves PATH'
    }
    [IO.File]::WriteAllText((Join-Path $fixtureDirectory 'SHA256SUMS'), $originalManifest)
    $testFailure = 'network'
    Assert-InstallFails { & $installer -BinDir $binDirectory -Version 0.1.0 } 'network failure is rejected'
    $testFailure = ''
    Assert-InstallFails { & $installer -BinDir $binDirectory -Version '../../bad' } 'unsafe version is rejected'
    Assert-InstallFails { & $installer -BinDir 'C:relative' } 'drive-relative path is rejected'
    Assert-InstallFails { & $installer -BinDir 'C:\bad;path' } 'PATH separator is rejected'
    $env:PROCESSOR_ARCHITEW6432 = 'ARM64'
    Assert-InstallFails { & $installer -BinDir $binDirectory } 'unsupported native architecture is rejected'
    $env:PROCESSOR_ARCHITEW6432 = $originalNativeArchitecture

    # A verified archive without the exact executable must not modify the installation.
    $zipPath = Join-Path $fixtureDirectory $archiveName
    [IO.File]::Delete($zipPath)
    $zip = [IO.Compression.ZipFile]::Open($zipPath, [IO.Compression.ZipArchiveMode]::Create)
    $zip.CreateEntry('../unwanted.txt') | Out-Null
    $zip.Dispose()
    [IO.File]::WriteAllText((Join-Path $fixtureDirectory 'SHA256SUMS'), ((Get-FileHash -LiteralPath $zipPath).Hash + '  ' + $archiveName))
    Assert-InstallFails { & $installer -BinDir $binDirectory } 'archive without expected binary is rejected'
    Assert-True ((Get-FileHash -LiteralPath $binary).Hash -eq $installedHash) 'archive failure preserves binary'
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $testRoot 'unwanted.txt'))) 'unexpected member is never extracted'
    Assert-True (@(Get-ChildItem -LiteralPath $binDirectory -Filter '.tileboard-*').Count -eq 0) 'staged files are cleaned'

    Remove-Item Function:Invoke-WebRequest
    Remove-Item Function:Invoke-RestMethod
    # Exercise real latest-release resolution and downloads, too.
    & $installer -BinDir $binDirectory
    Assert-True ((& $binary --version) -match '^tileboard \d+\.\d+\.\d+') 'latest release smoke test'
    # Test the documented pipe-to-iex form and default location on disposable CI hosts.
    if ($env:CI -eq 'true') {
        $candidate = Join-Path $env:LOCALAPPDATA 'Tileboard\bin'
        Assert-True (-not [IO.Directory]::Exists($candidate)) 'default test destination must be unused'
        $defaultDirectory = $candidate
        [IO.File]::ReadAllText($installer) | Invoke-Expression
        Assert-True ((Get-Command tileboard).Source -eq (Join-Path $defaultDirectory 'tileboard.exe')) 'pipe-to-iex defaults work'
    }
    Write-Host "All PowerShell installer tests passed on $($PSVersionTable.PSVersion)."
} finally {
    [Environment]::SetEnvironmentVariable('Path', $originalUserPath, 'User')
    $env:Path = $originalProcessPath
    $env:PROCESSOR_ARCHITECTURE = $originalArchitecture
    $env:PROCESSOR_ARCHITEW6432 = $originalNativeArchitecture
    [Net.ServicePointManager]::SecurityProtocol = $originalTls
    if ([IO.Directory]::Exists($testRoot)) { [IO.Directory]::Delete($testRoot, $true) }
    if ($defaultDirectory -and [IO.Directory]::Exists($defaultDirectory)) { [IO.Directory]::Delete($defaultDirectory, $true) }
}
