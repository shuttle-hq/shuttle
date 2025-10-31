function Install-Impulse {
    # Anonymous telemetry
    $TELEMETRY = "1"
    $PLATFORM = "windows"
    $ARCH = "$env:PROCESSOR_ARCHITECTURE"
    $NEW_INSTALL = "true"
    $INSTALL_METHOD = ""
    $OUTCOME = ""
    $STEP_FAILED = "N/A"
    $STARTED_AT = (Get-Date -Format "o")

    # Disable telemetry if any opt-out vars are set
    if ($env:DO_NOT_TRACK -eq "1" -or $env:DO_NOT_TRACK -eq "true" -or
        $env:DISABLE_TELEMETRY -eq "1" -or $env:DISABLE_TELEMETRY -eq "true" -or
        $env:SHUTTLE_DISABLE_TELEMETRY -eq "1" -or $env:SHUTTLE_DISABLE_TELEMETRY -eq "true" -or
        $env:CI -eq "1" -or $env:CI -eq "true") {
        $TELEMETRY = "0"
    }

    Write-Host "    ____                      __        "
    Write-Host "   /  _/___ ___  ____  __  __/ /_______ "
    Write-Host "   / // __ `__ \/ __ \/ / / / / ___/ _ \"
    Write-Host " _/ // / / / / / /_/ / /_/ / (__  )  __/"
    Write-Host "/___/_/ /_/ /_/ .___/\__,_/_/____/\___/ "
    Write-Host "             /_/              by Shuttle"
    Write-Host
    Write-Host @"
https://docs.shuttle.dev
https://discord.gg/shuttle
https://github.com/shuttle-hq/shuttle

Please open an issue if you encounter any problems!
"@
    if ($TELEMETRY -eq "1") {
        Write-Host "Anonymous telemetry enabled. More info and opt-out:" -ForegroundColor Gray
        Write-Host "https://docs.shuttle.dev/install-script" -ForegroundColor Gray
    }
    Write-Host "==================================================="
    Write-Host

    function Send-Telemetry {
        if ($TELEMETRY -eq "1") {
            $ENDED_AT = (Get-Date -Format "o")
            $telemetry_data = @{
                api_key = "phc_cQMQqF5QmcEzXEaVlrhv3yBSNRyaabXYAyiCV7xKHUH"
                distinct_id = "install_script"
                event = "install_script_completion"
                properties = @{
                    platform = $PLATFORM
                    new_install = $NEW_INSTALL
                    install_method = $INSTALL_METHOD
                    started_at = $STARTED_AT
                    ended_at = $ENDED_AT
                    outcome = $OUTCOME
                    step_failed = $STEP_FAILED
                    dont_track_ip = $true
                }
            } | ConvertTo-Json -Depth 3

            if ($env:SHUTTLE_DEBUG) {
                Write-Host "DEBUG: Sending telemetry data:`n$telemetry_data"
            }
            Invoke-RestMethod -Uri "https://console.shuttle.dev/ingest/i/v0/e" -Method Post -ContentType "application/json" -Body $telemetry_data -ErrorAction SilentlyContinue | Out-Null
        }
    }

    function Exit-Success {
        $OUTCOME = "success"
        Send-Telemetry
        Write-Host ""
        Write-Host "Thanks for installing Impulse CLI!" -ForegroundColor Green
    }

    function Exit-Neutral {
        $OUTCOME = "neutral"
        Write-Host ""
        Write-Host "If you have any problems, please open an issue on GitHub or visit our Discord!"
        Send-Telemetry
    }

    function Exit-Failure {
        param($StepFailed)
        $STEP_FAILED = $StepFailed
        $OUTCOME = "failure"
        Write-Host ""
        Write-Host "Impulse installation script failed with reason: $STEP_FAILED" -ForegroundColor Red
        Write-Host "If you have any problems, please open an issue on GitHub or visit our Discord!"
        Send-Telemetry
    }

    $RepoUrl = "https://github.com/shuttle-hq/shuttle"
    (Invoke-WebRequest "$RepoUrl/releases/latest" -Headers @{ "Accept" = "application/json" }).Content -match '"tag_name":"([^"]*)"' | Out-Null
    if (!$?) { return Exit-Failure "check-latest-release" }
    $LatestRelease = $Matches.1
    if ($LatestRelease -eq $null) { return Exit-Failure "parse-latest-version" }

    if (Get-Command -CommandType Application -ErrorAction SilentlyContinue impulse.exe) {
        $NEW_INSTALL = "false"
        $LatestReleaseStripped = $LatestRelease -replace '^v', ''
        $CurrentVersion = & impulse.exe -V
        $CurrentVersionStripped = $CurrentVersion -replace '^cargo-shuttle ', ''
        if ($LatestReleaseStripped -eq $CurrentVersionStripped) {
            Write-Host "Impulse CLI is already at the latest version!" -ForegroundColor Green
            return
        }
        else {
            Write-Host "Updating Impulse CLI to $LatestRelease"
        }
    }

    $TempDir = $Env:TEMP

    if ($ARCH -ne "AMD64") {
        Write-Host "Unsupported Architecture: Binaries are not currently built for $ARCH" -ForegroundColor Red
        return Exit-Failure "unsupported-architecture"
    }
    if {
        Write-Host "Could not find tar.exe (required to extract the release asset)" -ForegroundColor Red
        return Exit-Failure "tar-not-found"
    }

    $INSTALL_METHOD = "binary-download"
    $BinaryUrl = "$RepoUrl/releases/download/$LatestRelease/cargo-shuttle-$LatestRelease-x86_64-pc-windows-msvc.tar.gz"
    Invoke-WebRequest $BinaryUrl -OutFile "$TempDir\cargo-shuttle.tar.gz"
    if (!$?) { return Exit-Failure "download-binary" }
    New-Item -ItemType Directory -Force "$TempDir\cargo-shuttle" | Out-Null
    if (!$?) { return Exit-Failure "temp-folder" }
    tar.exe -xzf "$TempDir\cargo-shuttle.tar.gz" -C "$TempDir\cargo-shuttle"
    if (!$?) { return Exit-Failure "tar-extract-binary" }
    $CargoInstallBinDir = "$env:USERPROFILE\bin"
    New-Item -ItemType Directory -Force "$CargoInstallBinDir" | Out-Null
    if (!$?) { return Exit-Failure "binary-folder" }
    Write-Host "Installing to $CargoInstallBinDir\impulse.exe"
    Move-Item -Force "$TempDir\cargo-shuttle\cargo-shuttle-x86_64-pc-windows-msvc-$LatestRelease\impulse.exe" "$CargoInstallBinDir\impulse.exe"
    if (!$?) { return Exit-Failure "move-binary" }
    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue "$TempDir\cargo-shuttle.tar.gz", "$TempDir\cargo-shuttle"

    return Exit-Success
}


$OldErrorAction = $ErrorActionPreference
$ErrorActionPreference = "Stop"
Install-Impulse
$ErrorActionPreference = $OldErrorAction
