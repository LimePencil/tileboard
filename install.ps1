#Requires -Version 5.1
<#
.SYNOPSIS
Install Tileboard for Windows x64 without administrator privileges.
.PARAMETER Version
Release version, such as 0.1.0 or v0.1.0. Defaults to the latest release.
.PARAMETER BinDir
Absolute installation directory. Defaults to %LOCALAPPDATA%\Tileboard\bin.
.PARAMETER NoModifyPath
Leave the current process PATH and persistent Windows User PATH unchanged.
.EXAMPLE
irm https://raw.githubusercontent.com/LimePencil/tileboard/main/install.ps1 | iex
#>
[CmdletBinding()]
param(
    [string] $Version,
    [string] $BinDir,
    [switch] $NoModifyPath
)

# Keep preferences and helper functions out of the caller's interactive scope.
& {
    param([string] $Version, [string] $BinDir, [switch] $NoModifyPath)
    Set-StrictMode -Version Latest
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue'
    $temporaryDirectory = $null
    $stagedBinary = $null
    $previousTls = [Net.ServicePointManager]::SecurityProtocol

    function Get-TileboardFile([string] $Uri, [string] $Destination) {
        for ($attempt = 1; $attempt -le 3; $attempt++) {
            try {
                Invoke-WebRequest -UseBasicParsing -Uri $Uri -OutFile $Destination -TimeoutSec 120
                return
            } catch {
                if ($attempt -eq 3) { throw }
                Start-Sleep -Seconds $attempt
            }
        }
    }

    function Add-TileboardPath([string] $CurrentPath, [string] $Directory) {
        $entries = @($CurrentPath -split ';' | Where-Object { $_ })
        $expanded = @($entries | ForEach-Object {
            [Environment]::ExpandEnvironmentVariables($_).TrimEnd('\')
        })
        if ($expanded -contains $Directory.TrimEnd('\')) { return $CurrentPath }
        return ((@($Directory) + $entries) -join ';')
    }

    try {
        if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
            throw 'Use install.sh on Linux, macOS, or WSL.'
        }
        $architecture = $env:PROCESSOR_ARCHITEW6432
        if (-not $architecture) { $architecture = $env:PROCESSOR_ARCHITECTURE }
        if ($architecture -ne 'AMD64') {
            throw "Windows releases require x64; detected $architecture. Build from source on other architectures."
        }
        if (-not $BinDir) { $BinDir = Join-Path $env:LOCALAPPDATA 'Tileboard\bin' }
        if ($BinDir -notmatch '^(?:[a-zA-Z]:[\\/]|\\\\[^\\]+\\[^\\]+)' -or $BinDir -match '[;\x00-\x1f]') {
            throw 'BinDir must be an absolute Windows path without semicolons or control characters.'
        }
        $BinDir = [IO.Path]::GetFullPath($BinDir)
        [Net.ServicePointManager]::SecurityProtocol = $previousTls -bor [Net.SecurityProtocolType]::Tls12
        if (-not $Version) {
            $release = Invoke-RestMethod -UseBasicParsing -Uri 'https://api.github.com/repos/LimePencil/tileboard/releases/latest' -TimeoutSec 30
            $Version = $release.tag_name
        }
        $Version = $Version -creplace '^v', ''
        if ($Version -cnotmatch '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$') {
            throw 'Invalid release version; use a version such as 0.1.0.'
        }
        $archiveRoot = "tileboard-v$Version-x86_64-pc-windows-msvc"
        $archiveName = "$archiveRoot.zip"
        $releaseUrl = "https://github.com/LimePencil/tileboard/releases/download/v$Version"
        $temporaryDirectory = Join-Path ([IO.Path]::GetTempPath()) ("tileboard-install-" + [guid]::NewGuid().ToString('N'))
        [IO.Directory]::CreateDirectory($temporaryDirectory) | Out-Null
        $archivePath = Join-Path $temporaryDirectory $archiveName
        $manifestPath = Join-Path $temporaryDirectory 'SHA256SUMS'
        Write-Host "Installing Tileboard v$Version for Windows x64..."
        Get-TileboardFile "$releaseUrl/$archiveName" $archivePath
        Get-TileboardFile "$releaseUrl/SHA256SUMS" $manifestPath
        $checksums = @(Get-Content -LiteralPath $manifestPath | Where-Object {
            $_ -cmatch ('^[0-9a-fA-F]{64}\s+\*?' + [regex]::Escape($archiveName) + '$')
        })
        if ($checksums.Count -ne 1) { throw 'Expected exactly one SHA-256 checksum for the release archive.' }
        $expectedHash = ($checksums[0] -split '\s+')[0]
        if ((Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash -ne $expectedHash) {
            throw 'Checksum mismatch; the existing installation was not changed.'
        }

        # Extract only the expected executable, never arbitrary archive paths.
        Add-Type -AssemblyName System.IO.Compression.FileSystem
        $archive = [IO.Compression.ZipFile]::OpenRead($archivePath)
        $downloadedBinary = Join-Path $temporaryDirectory 'tileboard.exe'
        try {
            $members = @($archive.Entries | Where-Object { $_.FullName -ceq "$archiveRoot/tileboard.exe" })
            if ($members.Count -ne 1) { throw 'Expected exactly one tileboard.exe in the release archive.' }
            [IO.Compression.ZipFileExtensions]::ExtractToFile($members[0], $downloadedBinary)
        } finally { $archive.Dispose() }
        $reportedVersion = & $downloadedBinary --version
        if ($LASTEXITCODE -ne 0 -or $reportedVersion -cne "tileboard $Version") {
            throw 'The downloaded binary failed its version check; existing installation unchanged.'
        }
        [IO.Directory]::CreateDirectory($BinDir) | Out-Null
        $destination = Join-Path $BinDir 'tileboard.exe'
        $stagedBinary = Join-Path $BinDir ('.tileboard-' + [guid]::NewGuid().ToString('N') + '.exe')
        [IO.File]::Copy($downloadedBinary, $stagedBinary)
        try {
            if ([IO.File]::Exists($destination)) {
                [IO.File]::Replace($stagedBinary, $destination, [System.Management.Automation.Language.NullString]::Value)
            } else {
                [IO.File]::Move($stagedBinary, $destination)
            }
        } catch {
            throw "Could not install $destination. Close any running Tileboard instance and check directory permissions. $($_.Exception.Message)"
        }
        $stagedBinary = $null
        if (-not $NoModifyPath) {
            $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
            $updatedPath = Add-TileboardPath $userPath $BinDir
            try {
                if ($updatedPath -cne $userPath) {
                    [Environment]::SetEnvironmentVariable('Path', $updatedPath, 'User')
                }
            } catch {
                throw "Installed $destination, but could not update Windows User PATH. Add $BinDir to PATH manually. $($_.Exception.Message)"
            }
            $env:Path = Add-TileboardPath $env:Path $BinDir
        }
        Write-Host "Installed $destination"
        if ($NoModifyPath) {
            Write-Host "Add $BinDir to PATH to run tileboard by name."
        } else {
            Write-Host 'Run: tileboard'
            Write-Host 'PATH is ready in this PowerShell session. Restart other terminal applications to pick up the change.'
        }
    } finally {
        [Net.ServicePointManager]::SecurityProtocol = $previousTls
        if ($stagedBinary -and [IO.File]::Exists($stagedBinary)) { [IO.File]::Delete($stagedBinary) }
        if ($temporaryDirectory -and [IO.Directory]::Exists($temporaryDirectory)) {
            [IO.Directory]::Delete($temporaryDirectory, $true)
        }
    }
} @PSBoundParameters
