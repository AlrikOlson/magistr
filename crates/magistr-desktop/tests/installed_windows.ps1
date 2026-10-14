# Install the real package, verify its protocol roles, and open its native window.
param([Parameter(Mandatory = $true)][string]$Installer)
$ErrorActionPreference = "Stop"
$testDirectory = Join-Path ([System.IO.Path]::GetTempPath()) ("Magistr install " + [guid]::NewGuid())
$installDirectory = Join-Path $testDirectory "Magistr"
New-Item -ItemType Directory -Path $testDirectory | Out-Null
$app = $null
try {
    # NSIS requires /D last and unquoted even when the destination contains spaces.
    $setup = Start-Process -FilePath $Installer -ArgumentList "/S /D=$installDirectory" -Wait -PassThru
    if ($setup.ExitCode -ne 0) { throw "Installer exited with code $($setup.ExitCode)." }
    $binary = Join-Path $installDirectory "magistr-desktop.exe"
    if (-not (Test-Path $binary)) { throw "The installer did not install Magistr." }
    $shortcut = Join-Path ([Environment]::GetFolderPath("StartMenu")) "Programs/Magistr/Magistr.lnk"
    if (-not (Test-Path $shortcut)) { throw "The installer did not create its Start menu shortcut." }
    & python "$PSScriptRoot/process_roles.py" $binary
    if ($LASTEXITCODE -ne 0) { throw "Installed executable protocol checks failed." }

    $app = Start-Process -FilePath $binary -WorkingDirectory $testDirectory -PassThru
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        Start-Sleep -Milliseconds 200
        $app.Refresh()
        if ($app.HasExited) { throw "Magistr exited before opening its window." }
    } while ($app.MainWindowHandle -eq [IntPtr]::Zero -and [DateTime]::UtcNow -lt $deadline)
    if ($app.MainWindowHandle -eq [IntPtr]::Zero -or $app.MainWindowTitle -ne "Magistr") {
        throw "The installed app did not open its Magistr window."
    }
    if (-not $app.CloseMainWindow() -or -not $app.WaitForExit(15000)) {
        throw "The installed app did not close normally."
    }
    if ($app.ExitCode -ne 0) { throw "The installed app exited with code $($app.ExitCode)." }
    Write-Output "PASS: per-user installation, Start menu shortcut, installed child roles, native window and Quit"
} finally {
    if ($app -and -not $app.HasExited) { Stop-Process -Id $app.Id -Force }
    $uninstaller = Join-Path $installDirectory "uninstall.exe"
    if (Test-Path $uninstaller) {
        $cleanup = Start-Process -FilePath $uninstaller -ArgumentList "/S" -Wait -PassThru
        if ($cleanup.ExitCode -ne 0) { Write-Warning "Test uninstaller failed: $($cleanup.ExitCode)" }
    }
}
