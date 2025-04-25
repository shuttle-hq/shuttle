function Install-Cargo-Shuttle {
    Write-Host @"
       ___
      /   \    _           _   _   _
   __/    /___| |__  _   _| |_| |_| | ___
  /_     // __| '_ \| | | | __| __| |/ _ \
   _|_  | \__ \ | | | |_| | |_| |_| |  __/
  |_| |/  |___/_| |_|\__,_|\__|\__|_|\___|

https://docs.shuttle.dev
https://discord.gg/shuttle
https://github.com/shuttle-hq/shuttle

Please open an issue if you encounter any problems!
===================================================

"@

    # Anonymous telemetry
    $TELEMETRY = "1"
    $PLATFORM = "windows"
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

    if ($TELEMETRY -eq "1") {
        Write-Host "Anonymous telemetry enabled. More info and opt-out:" -ForegroundColor Gray
        Write-Host "https://docs.shuttle.dev/getting-started/installation#install-script" -ForegroundColor Gray
        Write-Host ""
    }

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
                Write-Host "Sending telemetry data:`n$telemetry_data"
            }
            Invoke-RestMethod -Uri "https://console.shuttle.dev/ingest/i/v0/e" -Method Post -ContentType "application/json" -Body $telemetry_data -ErrorAction SilentlyContinue | Out-Null
        }
    }

    function Exit-Success {
        $OUTCOME = "success"
        Send-Telemetry
        Write-Host ""
        Write-Host "Thanks for installing Shuttle CLI! 🚀" -ForegroundColor Green
    }

    function Exit-Neutral {
        Write-Host ""
        Write-Host "If you have any problems, please open an issue on GitHub or visit our Discord!"
    }

    function Exit-Failure {
        param($StepFailed)
        $STEP_FAILED = $StepFailed
        $OUTCOME = "fail"
        Write-Host ""
        Write-Host "Shuttle installation script failed with reason: $STEP_FAILED" -ForegroundColor Red
        Write-Host "If you have any problems, please open an issue on GitHub or visit our Discord!"
        Send-Telemetry
    }

    $Arch = [Environment]::GetEnvironmentVariable("PROCESSOR_ARCHITECTURE", [EnvironmentVariableTarget]::Machine)
    $TempDir = $Env:TEMP

    if (!(Get-Command -CommandType Application -ErrorAction SilentlyContinue cargo.exe)) {
        Write-Host "Could not find cargo.exe, Rust may not be installed." -ForegroundColor Red
        if ($Arch -inotin "AMD64", "x86") {
            Write-Host "Rustup is only provided for x86 and x86_64, not $Arch" -ForegroundColor Red
            Write-Host "Please install Rust manually, more info at: https://rust-lang.github.io/rustup/installation/other.html" -ForegroundColor Red
            return Exit-Failure "unsupported-arch"
        }
        $Confirm = Read-Host -Prompt "Install Rust and Cargo via rustup? [Y/n]"
        if ($Confirm -inotin "y", "Y", "yes", "") {
            Write-Host "Skipping rustup install, cargo-shuttle not installed"
            return Exit-Neutral
        }
        $RustupUrl = if ($Arch -eq "AMD64") { "https://win.rustup.rs/x86_64" } else { "https://win.rustup.rs/i686" }
        Invoke-WebRequest $RustupUrl -OutFile "$TempDir\rustup-init.exe"
        & "$TempDir\rustup-init.exe" # toolchain install stable
        # if ($LASTEXITCODE -ne 0) { return Exit-Failure "get-rustup" }
        if ($?) {
            Remove-Item -ErrorAction SilentlyContinue "$TempDir\rustup.exe"
            Write-Host "Rust installed via Rustup, please re-run this script, you may need reopen your terminal" -ForegroundColor Green
            return Exit-Neutral
        }
        else {
            Remove-Item -ErrorAction SilentlyContinue "$TempDir\rustup.exe"
            Write-Host "Rust install via Rustup failed, please install Rust manually: https://rustup.rs/" -ForegroundColor Red
            return Exit-Failure "install-rust"
        }
    }

    if (Get-Command -CommandType Application -ErrorAction SilentlyContinue cargo-binstall.exe) {
        Write-Host "Installing Shuttle CLI using cargo-binstall"
        cargo-binstall.exe -y --force --locked cargo-shuttle
        if ($?) {
            return Exit-Success
        }
        else {
            Write-Host "Could not install from release using cargo-binstall" -ForegroundColor Red
            return Exit-Failure "cargo-binstall"
        }
    }
    else {
        Write-Host "Cargo binstall not found, trying manual binary download" -ForegroundColor Red
    }

    $RepoUrl = "https://github.com/shuttle-hq/shuttle"
    $CargoHome = if ($null -ne $Env:CARGO_HOME) { $Env:CARGO_HOME } else { "$HOME\.cargo" }

    if (($Arch -eq "AMD64") -and (Get-Command -CommandType Application -ErrorAction SilentlyContinue tar.exe)) {
        (Invoke-WebRequest "$RepoUrl/releases/latest" -Headers @{ "Accept" = "application/json" }).Content -match '"tag_name":"([^"]*)"' | Out-Null
        if ($LASTEXITCODE -ne 0) { return Exit-Failure "check-latest-release" }
        $LatestRelease = $Matches.1
        $BinaryUrl = "$RepoUrl/releases/download/$LatestRelease/cargo-shuttle-$LatestRelease-x86_64-pc-windows-msvc.tar.gz"
        Invoke-WebRequest $BinaryUrl -OutFile "$TempDir\cargo-shuttle.tar.gz"
        if ($LASTEXITCODE -ne 0) { return Exit-Failure "download-binary" }
        New-Item -ItemType Directory -Force "$TempDir\cargo-shuttle"
        if ($LASTEXITCODE -ne 0) { return Exit-Failure "temp-folder" }
        tar.exe -xzf "$TempDir\cargo-shuttle.tar.gz" -C "$TempDir\cargo-shuttle"
        if ($LASTEXITCODE -ne 0) { return Exit-Failure "tar-extract-binary" }
        Move-Item -Force "$TempDir\cargo-shuttle\cargo-shuttle-x86_64-pc-windows-msvc-$LatestRelease\cargo-shuttle.exe" "$CargoHome\bin\cargo-shuttle.exe"
        if ($LASTEXITCODE -ne 0) { return Exit-Failure "move-binary" }
        Move-Item -Force "$TempDir\cargo-shuttle\cargo-shuttle-x86_64-pc-windows-msvc-$LatestRelease\shuttle.exe" "$CargoHome\bin\shuttle.exe"
        if ($LASTEXITCODE -ne 0) { return Exit-Failure "move-binary" }
        Remove-Item -Recurse -Force -ErrorAction SilentlyContinue "$TempDir\cargo-shuttle.tar.gz", "$TempDir\cargo-shuttle"
        return Exit-Success
    }
    elseif ($Arch -ne "AMD64") {
        Write-Host "Unsupported Architecture: Binaries are not currently built for $Arch, skipping manual binary download" -ForegroundColor Red
    }
    else {
        Write-Host "Could not find tar.exe, skipping manual binary download (required to extract the release asset)" -ForegroundColor Red
    }


    if (Get-Command -CommandType Application -ErrorAction SilentlyContinue cargo.exe) {
        Write-Host "Installing cargo-shuttle using cargo install (from source)"
        cargo.exe install --locked cargo-shuttle
        if ($LASTEXITCODE -ne 0) { return Exit-Failure "cargo-install" }
        return Exit-Success
    }
    else {
        return Exit-Failure "cargo-not-found"
    }
}


$OldErrorAction = $ErrorActionPreference
$ErrorActionPreference = "Stop"
Install-Cargo-Shuttle
$ErrorActionPreference = $OldErrorAction
