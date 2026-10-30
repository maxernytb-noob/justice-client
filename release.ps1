<#
  Publishes a new Justice Client version to https://justiceclient.online/ (also served at ertenix.net/justice/ for older launchers).
  Every installed launcher sees it within a few hours (or on the next start) and offers "Update".

  Usage:
    .\release.ps1 -Version 0.3.0 -Notes "- New thing`n- Fixed thing"
    .\release.ps1 -Notes "..."            # re-release the version already in tauri.conf.json
    .\release.ps1 -Version 0.3.0 -DryRun  # build + prepare files, don't upload

  Needs: the update signing key in D:\dev\justice-signing\justice.key (never commit or share it —
  losing it means installed launchers can't be updated anymore), and SSH access to the VPS.
#>
param(
  [string]$Version,
  [string]$Notes = "",
  [switch]$DryRun
)
$ErrorActionPreference = "Stop"
$root = $PSScriptRoot
$key = "D:\dev\justice-signing\justice.key"
$sshKey = "$env:USERPROFILE\.ssh\claude_bythorn"
$remote = "claude@178.95.164.26"
$baseUrl = "https://justiceclient.online"
$utf8 = New-Object Text.UTF8Encoding $false

$env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('Path', 'User') + ';D:\dev\cargo\bin'
$env:CARGO_HOME = 'D:\dev\cargo'
$env:RUSTUP_HOME = 'D:\dev\rustup'

# ---- 1. Version bump (package.json, tauri.conf.json, Cargo.toml) ----
$confPath = "$root\src-tauri\tauri.conf.json"
if ($Version) {
  node -e "const fs=require('fs');for(const f of ['package.json','src-tauri/tauri.conf.json']){const p='$($root.Replace('\','/'))/'+f;const j=JSON.parse(fs.readFileSync(p,'utf8'));j.version='$Version';fs.writeFileSync(p,JSON.stringify(j,null,2)+'\n')}"
  $cargo = [IO.File]::ReadAllText("$root\src-tauri\Cargo.toml")
  $cargo = [regex]::Replace($cargo, '(?m)^version = "[^"]+"', "version = `"$Version`"", 1)
  [IO.File]::WriteAllText("$root\src-tauri\Cargo.toml", $cargo, $utf8)
}
$Version = (Get-Content $confPath -Raw | ConvertFrom-Json).version
Write-Host "== Justice Client $Version" -ForegroundColor Red

# ---- 2. Signed build ----
if (-not (Test-Path $key)) { throw "Missing signing key $key" }
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $key -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = (Get-Content "D:\dev\justice-signing\password.txt" -Raw).Trim()
Push-Location $root
npx tauri build
if ($LASTEXITCODE -ne 0) { Pop-Location; throw "Build failed" }
Pop-Location

$nsis = "$root\src-tauri\target\release\bundle\nsis"
$setup = Get-ChildItem $nsis -Filter "*_${Version}_x64-setup.exe" | Select-Object -First 1
$sig = "$($setup.FullName).sig"
if (-not $setup -or -not (Test-Path $sig)) { throw "Installer or its .sig not found in $nsis" }

# ---- 3. Release folder: installer, stable-name copy, latest.json, website ----
$out = "$root\packaging\release\$Version"
Remove-Item -Recurse -Force $out -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $out | Out-Null
$versioned = "JusticeClient_${Version}_x64-setup.exe"
Copy-Item $setup.FullName "$out\$versioned"
Copy-Item $setup.FullName "$out\JusticeClient-setup.exe"
Copy-Item "$root\packaging\web\index.html" "$out\index.html"
Copy-Item -Recurse "$root\packaging\web\img" "$out\img"
Copy-Item "$root\src-tauri\icons\128x128@2x.png" "$out\icon.png"

$latest = [ordered]@{
  version   = $Version
  notes     = $Notes
  pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
  platforms = [ordered]@{
    "windows-x86_64" = [ordered]@{
      signature = (Get-Content $sig -Raw).Trim()
      url       = "$baseUrl/$versioned"
    }
  }
}
[IO.File]::WriteAllText("$out\latest.json", ($latest | ConvertTo-Json -Depth 5), $utf8)

# Arch: source bundle for the build on the VPS (not published — it only feeds the binary package).
$archBuild = "$root\packaging\release\$Version-arch"
Remove-Item -Recurse -Force $archBuild -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $archBuild | Out-Null
$bash = "C:\Program Files\Git\bin\bash.exe"
$toPosix = { param($p) "/" + $p.Substring(0, 1).ToLower() + $p.Substring(2).Replace('\', '/') }
& $bash (& $toPosix "$root\packaging\arch\make-arch.sh") $Version (& $toPosix $archBuild)
if ($LASTEXITCODE -ne 0) { throw "Arch package failed" }

Get-ChildItem $out | Format-Table Name, Length
if ($DryRun) { Write-Host "Dry run: nothing uploaded. Files are in $out"; return }

# ---- 4. Upload: installers first, latest.json last so clients never see a version without its file ----
$stage = "/home/claude/justice-up-$Version-$(Get-Random)"
ssh -i $sshKey $remote "mkdir -p $stage"
scp -q -r -i $sshKey "$out\*" "${remote}:$stage/"
if ($LASTEXITCODE -ne 0) { throw "Upload failed" }

# Prebuilt Arch package, compiled in an Arch container on the VPS (~15 min); ready before latest.json goes live.
$archStage = "$stage-arch"
ssh -i $sshKey $remote "mkdir -p $archStage"
scp -q -i $sshKey "$archBuild\justice-client-arch.tar.gz" "$root\packaging\arch\Dockerfile.arch" "$root\packaging\arch\vps-build.sh" "${remote}:$archStage/"
if ($LASTEXITCODE -ne 0) { throw "Arch upload failed" }
ssh -i $sshKey $remote "cd $archStage && tar -xzf justice-client-arch.tar.gz && sed -i 's/\r`$//' vps-build.sh && bash vps-build.sh $archStage && cp arch/*.pkg.tar.zst $stage/ && cp arch/*.pkg.tar.zst $stage/justice-client.pkg.tar.zst"
if ($LASTEXITCODE -ne 0) { throw "Arch build on the VPS failed" }
# .deb and AppImage from the same source tarball, in an Ubuntu 22.04 container (~15 min).
scp -q -i $sshKey "$root\packaging\linux\Dockerfile.deb" "$root\packaging\linux\vps-build-deb.sh" "${remote}:$archStage/"
if ($LASTEXITCODE -ne 0) { throw "Linux build upload failed" }
ssh -i $sshKey $remote "cd $archStage && sed -i 's/\r`$//' vps-build-deb.sh Dockerfile.deb && cp arch/justice-client-$Version.tar.gz . && bash vps-build-deb.sh $archStage $Version && cp out/justice-client_${Version}_amd64.deb out/JusticeClient-$Version-x86_64.AppImage $stage/ && cp out/justice-client_${Version}_amd64.deb $stage/justice-client.deb && cp out/JusticeClient-$Version-x86_64.AppImage $stage/JusticeClient.AppImage"
if ($LASTEXITCODE -ne 0) { throw ".deb/AppImage build on the VPS failed" }
# Installers first, latest.json last: clients must never see a version whose file isn't there yet.
ssh -i $sshKey $remote "cd $stage && sudo install -d -o www-data -g www-data /var/www/justice/img && sudo install -o www-data -g www-data -m 644 img/* /var/www/justice/img/ && for f in *; do [ -f `"`$f`" ] || continue; [ `"`$f`" = latest.json ] || sudo install -o www-data -g www-data -m 644 `"`$f`" /var/www/justice/`"`$f`"; done && sudo install -o www-data -g www-data -m 644 latest.json /var/www/justice/latest.json && ls -la /var/www/justice"
if ($LASTEXITCODE -ne 0) { throw "Publishing on the server failed" }
Write-Host "Published $Version -> $baseUrl/" -ForegroundColor Green
