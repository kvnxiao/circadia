param([switch]$InspectOnly, [switch]$NativeControlsOnly)
$ErrorActionPreference = 'Stop'
$reader = Join-Path $PSScriptRoot 'target/debug/circadia-nightlight-timestamps-probe.exe'
$controllers = @(Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -match '^(circadia|flux|f\.lux|lightbulb|gammy|redshift|wnl|circadia-nightlight-runtime-probe|circadia-nightlight-timestamps-probe)$' } | Select-Object ProcessName,Id)
$metadata = [ordered]@{
    event='host';utc=[DateTime]::UtcNow.ToString('O')
    osBuild=[Environment]::OSVersion.Version.ToString();library='win-nightlight-lib 0.4.0'
    libraryCommit='96fdd61b06ca4fb74dbc2b8368ddc9bd4b77ab03'
    compiler=(& rustc +stable --version);profile='debug';controllers=$controllers
    existingSettings=@(Get-Process -Name SystemSettings -ErrorAction SilentlyContinue | Select-Object Id,ProcessName)
}
$metadata | ConvertTo-Json -Depth 4 -Compress
if ($controllers.Count) { throw 'A known temperature controller is running; the probe did not stop it or change Night Light' }
if ($InspectOnly) {
    & $reader
    if ($LASTEXITCODE -ne 0) { throw 'Read-only Night Light inspection failed' }
    return
}
$report = Join-Path $PSScriptRoot ('result-' + [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfff') + '.jsonl')
$metadata | ConvertTo-Json -Depth 4 -Compress | Set-Content -LiteralPath $report -Encoding UTF8
$operation = if ($NativeControlsOnly) { '--native-controls-only' } else { '--exercise' }
& $reader $operation | ForEach-Object {
    Add-Content -LiteralPath $report -Value $_ -Encoding UTF8
    $record = $_ | ConvertFrom-Json
    if ($record.event -ne 'sample') { $_ }
}
$code = $LASTEXITCODE
[ordered]@{event='report';path=$report;exitCode=$code} | ConvertTo-Json -Compress
if ($code -ne 0) { throw ('Timestamp probe failed; inspect restoration in ' + $report) }
