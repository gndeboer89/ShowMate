# Haalt een Windows-build van mpv op en zet die in dist\mpv (voor de installer).
# Gebruik: powershell -File installer\fetch-mpv.ps1
# Bron: GitHub-releases van shinchiro/mpv-winbuild-cmake (niet-officiële maar veelgebruikte builds).
# Let op: dit script is niet getest in CI-omstandigheden; controleer het resultaat (dist\mpv\mpv.exe).
$ErrorActionPreference = "Stop"
$dest = Join-Path $PSScriptRoot "..\dist\mpv"
New-Item -ItemType Directory -Force -Path $dest | Out-Null

$release = Invoke-RestMethod "https://api.github.com/repos/shinchiro/mpv-winbuild-cmake/releases/latest" `
    -Headers @{ "User-Agent" = "showmate-build" }
# Gewone 64-bit build, niet de -v3 (AVX2), -dev of -debug varianten.
$asset = $release.assets | Where-Object { $_.name -match '^mpv-x86_64-\d{8}-git-[0-9a-f]+\.7z$' } | Select-Object -First 1
if (-not $asset) { throw "Geen passende mpv-build gevonden in release $($release.tag_name)" }

$archive = Join-Path $env:TEMP $asset.name
Invoke-WebRequest $asset.browser_download_url -OutFile $archive
& 7z x $archive "-o$dest" -y | Out-Null
if (-not (Test-Path (Join-Path $dest "mpv.exe"))) { throw "mpv.exe ontbreekt na uitpakken" }
# GPL: leg vast welke build er wordt meegeleverd en waar de bron te vinden is (komt in de installer).
@(
    "mpv-build in deze installer: $($asset.name)",
    "Release: $($release.html_url)",
    "Build-scripts en bronverwijzingen: https://github.com/shinchiro/mpv-winbuild-cmake",
    "mpv broncode: https://github.com/mpv-player/mpv",
    "Licentie: GPL (zie THIRD-PARTY.md). Volledige bron op aanvraag: https://github.com/gndeboer89/ShowMate/issues"
) | Set-Content -Path (Join-Path $dest "SOURCE.txt") -Encoding UTF8
Write-Host "mpv geïnstalleerd in $dest ($($asset.name))"
