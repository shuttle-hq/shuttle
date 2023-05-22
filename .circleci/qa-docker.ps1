# Would actually like to error on all errors, but `Enable-ExperimentalFeature`
# does not work for this version of Windows
# https://github.com/PowerShell/PowerShell/issues/3415#issuecomment-1354457563
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# Add cargo to PATH
$env:Path += [IO.Path]::PathSeparator + "$env:USERPROFILE/.cargo/bin"

# Start locally
$job = Start-Job -Name "local-run" -ScriptBlock { cd examples/rocket/postgres; cargo shuttle run }
Start-Sleep -Seconds 300

echo "Testing local docker endpoint"
$postParams = @{note='test'}
$output=curl http://localhost:8000/todo -Method Post -Body $postParams | Select-Object -ExpandProperty Content
if ( $output -ne '{"id":1,"note":"test"}')
{
    echo "Did not expect POST output: $output"
    exit 1
}

$output=curl http://localhost:8000/todo/1 | Select-Object -ExpandProperty Content
if ( $output -ne '{"id":1,"note":"test"}')
{
    echo "Did not expect output: $output"
    exit 1
}

Stop-Job $job

exit 0
