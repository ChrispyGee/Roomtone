<#
.SYNOPSIS
  Builds the Microsoft Store package (.msix) for Roomtone from a release build.

.DESCRIPTION
  Run from the repository root after `cargo build --release --locked`:

    pwsh packaging/windows/msix/build-msix.ps1 -Version 0.9.4 -OutFile dist/roomtone-v0.9.4.msix

  Without the Partner Center values it makes a package that is only good for checking that the
  package is well formed (it can't be installed: it isn't signed, and its publisher is a
  placeholder). For a Store submission pass the three values from Partner Center (Product
  management > Product identity). The Store signs the package itself, so it is uploaded unsigned.
  See docs/MICROSOFT_STORE.md.
#>
param(
  [Parameter(Mandatory = $true)] [string] $Version,
  [string] $OutFile = "dist/roomtone-v$Version.msix",
  [string] $Exe = "target/release/roomtone.exe",
  [string] $IdentityName = "Roomtone.Placeholder",
  [string] $Publisher = "CN=Placeholder",
  [string] $PublisherDisplay = "Roomtone"
)
$ErrorActionPreference = "Stop"

if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "Version must look like 0.9.4, not '$Version'." }
if (-not (Test-Path $Exe)) { throw "Build the program first (cargo build --release --locked): $Exe is missing." }

$root = Resolve-Path "."
$stage = Join-Path $root "target/msix-stage"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force -Path (Join-Path $stage "Assets") | Out-Null

# The program, its licences and README.
Copy-Item $Exe (Join-Path $stage "roomtone.exe")
foreach ($f in "README.md", "LICENSE-MIT", "LICENSE-APACHE") { Copy-Item $f $stage }

# The logos, drawn by the program itself (the same icon as the installers).
$logos = @{ "Square44x44Logo" = 44; "Square150x150Logo" = 150; "StoreLogo" = 50; "Square71x71Logo" = 71; "Square310x310Logo" = 310 }
# (The program is a window program, so PowerShell would not wait for it: start it and wait.)
foreach ($name in $logos.Keys) {
  $png = Join-Path $stage "Assets/$name.png"
  $p = Start-Process -FilePath (Resolve-Path $Exe) -ArgumentList @("--write-icon", "`"$png`"", "$($logos[$name])") -Wait -PassThru
  if ($p.ExitCode -ne 0 -or -not (Test-Path $png)) { throw "Couldn't make $name.png" }
}

# The manifest, with the values filled in.
$manifest = Get-Content "packaging/windows/msix/AppxManifest.xml" -Raw
$manifest = $manifest.Replace("@IDENTITY_NAME@", $IdentityName).Replace("@PUBLISHER@", $Publisher).Replace("@PUBLISHER_DISPLAY@", $PublisherDisplay).Replace("@VERSION4@", "$Version.0")
Set-Content -Path (Join-Path $stage "AppxManifest.xml") -Value $manifest -Encoding UTF8

# makeappx comes with the Windows SDK (it is on GitHub's Windows runners).
$makeappx = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\makeappx.exe" -ErrorAction SilentlyContinue | Sort-Object FullName | Select-Object -Last 1
if (-not $makeappx) { throw "makeappx.exe not found: install the Windows SDK." }

New-Item -ItemType Directory -Force -Path (Split-Path $OutFile) | Out-Null
if (Test-Path $OutFile) { Remove-Item -Force $OutFile }
& $makeappx.FullName pack /d $stage /p $OutFile /o
if ($LASTEXITCODE -ne 0) { throw "makeappx failed" }
Write-Host "Made $OutFile ($([math]::Round((Get-Item $OutFile).Length / 1MB, 1)) MB)"
