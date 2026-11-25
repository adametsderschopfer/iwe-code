# Test installation, window startup, and removal on an isolated runner.
param([Parameter(Mandatory)][string]$Installer)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true') { throw 'Installer smoke tests require an isolated CI runner.' }
$directory = Join-Path $env:RUNNER_TEMP ('iwe-install-test-' + [guid]::NewGuid())
$log = "$directory.log"
$process = Start-Process $Installer -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/LANG=english', "/DIR=`"$directory`"", "/LOG=`"$log`"") -Wait -PassThru
if ($process.ExitCode -ne 0) { throw "Installer failed: $($process.ExitCode). See $log" }
if (-not (Test-Path "$directory/iwe.exe")) { throw 'Installed executable is missing.' }
$entries = @(Get-ChildItem 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall', 'HKCU:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall' -ErrorAction SilentlyContinue |
    Get-ItemProperty | Where-Object { $_.DisplayName -eq 'IWE Code' -and $_.InstallLocation.TrimEnd('\') -eq $directory })
if ($entries.Count -ne 1) { throw 'Installer registration is missing or duplicated.' }

$data = "$directory-data"
$start = [System.Diagnostics.ProcessStartInfo]::new("$directory/iwe.exe")
$start.UseShellExecute = $false
$start.Environment['IWE_DATA_DIR'] = $data
# Exercise the installed window process with the CPU renderer on the CI runner.
$start.Environment['IWE_GRAPHICS_CHILD'] = '1'
$start.Environment['ICED_BACKEND'] = 'tiny-skia'
try {
    $app = [System.Diagnostics.Process]::Start($start)
    try {
        $timer = [System.Diagnostics.Stopwatch]::StartNew()
        do {
            $app.Refresh()
            if ($app.HasExited) { throw "Application exited before opening a window: $($app.ExitCode)." }
            if ($app.MainWindowHandle -ne [IntPtr]::Zero) { break }
            if ($timer.Elapsed.TotalSeconds -ge 30) { throw 'Application did not open a window.' }
            Start-Sleep -Milliseconds 100
        } while ($true)

        $second = [System.Diagnostics.Process]::Start($start)
        try {
            if (-not $second.WaitForExit(10000)) { throw 'The second launch did not exit after forwarding.' }
            if ($second.ExitCode -ne 0) { throw "The second launch failed: $($second.ExitCode)." }
        } finally {
            if (-not $second.HasExited) {
                $second.Kill()
                $second.WaitForExit()
            }
            $second.Dispose()
        }
        if ($app.HasExited) { throw 'The first instance exited during the second launch.' }
    } finally {
        if (-not $app.HasExited) {
            if (-not $app.CloseMainWindow() -or -not $app.WaitForExit(5000)) {
                $app.Kill()
                $app.WaitForExit()
            }
        }
        $app.Dispose()
    }
} finally {
    $process = Start-Process "$directory/unins000.exe" -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART') -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw 'Uninstallation failed.' }
    if (Test-Path $data) { Remove-Item $data -Recurse -Force }
}
for ($attempt = 0; $attempt -lt 100 -and (Test-Path $directory); $attempt++) { Start-Sleep -Milliseconds 100 }
if (Test-Path $directory) { throw 'Uninstallation left application files behind.' }
