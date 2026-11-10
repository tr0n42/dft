# Runs every test group one at a time with live output, timings and a summary.
# Everything shown is also saved to test-results.txt.
Set-Location (Split-Path $PSScriptRoot -Parent)
$log = 'test-results.txt'
function Say([string]$text, [string]$colour = 'Gray') {
    Write-Host $text -ForegroundColor $colour
    Add-Content -Path $log -Value $text -Encoding UTF8
}
function Run([string]$cmd) {
    cmd /c "$cmd 2>&1" | ForEach-Object { Write-Host $_; Add-Content -Path $log -Value $_ -Encoding UTF8 }
    return $LASTEXITCODE
}
"Test run started $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') on $env:COMPUTERNAME" | Set-Content -Path $log -Encoding UTF8
$all = [Diagnostics.Stopwatch]::StartNew()

Say "=== Building the tests (a minute or two after an update, seconds otherwise) ===" Cyan
if ((Run 'cargo build --release --tests') -ne 0) {
    Say "BUILD FAILED: see above." Red
    exit 1
}

$groups = @(
    @{ name = 'reference'; what = 'locked realistic models';       est = 'about 10 seconds' },
    @{ name = 'generator'; what = 'planet generator';              est = 'a few seconds' },
    @{ name = 'world';     what = 'world store, mining and saves'; est = 'a few seconds' },
    @{ name = 'sandbox';   what = 'falling sand, heat, reactions'; est = 'a few seconds' },
    @{ name = 'bodies';    what = 'loose rock and wrecks';         est = 'a few seconds' },
    @{ name = 'chemistry'; what = 'elements and phase changes';    est = 'a few seconds' },
    @{ name = 'swarm';     what = 'drones, economy, tutorials';    est = 'about 4 minutes; prints progress every 10 minutes of play' }
)
$results = @()
foreach ($g in $groups) {
    Say ""
    Say ("=== {0}: {1} (expect {2}) ===" -f $g.name, $g.what, $g.est) Cyan
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $code = Run "cargo test --release --test $($g.name) -- --nocapture"
    $secs = $sw.Elapsed.TotalSeconds
    $ok = ($code -eq 0)
    $results += [pscustomobject]@{ Group = $g.name; Result = $(if ($ok) { 'passed' } else { 'FAILED' }); Seconds = [math]::Round($secs) }
    Say ("--- {0} {1} in {2:N0} s" -f $g.name, $(if ($ok) { 'passed' } else { 'FAILED' }), $secs) $(if ($ok) { 'Green' } else { 'Red' })
}

Say ""
Say "=== Summary ===" Cyan
foreach ($r in $results) {
    Say ("  {0,-10} {1,-7} {2,5} s" -f $r.Group, $r.Result, $r.Seconds) $(if ($r.Result -eq 'passed') { 'Green' } else { 'Red' })
}
$failed = @($results | Where-Object { $_.Result -ne 'passed' }).Count
Say ("Total time {0:N0} s. {1}" -f $all.Elapsed.TotalSeconds, $(if ($failed -eq 0) { 'ALL PASSED' } else { "$failed group(s) FAILED" })) $(if ($failed -eq 0) { 'Green' } else { 'Red' })
Say "Results saved to test-results.txt"
exit $failed
