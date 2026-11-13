; Inno Setup script. Build with:
;   set ROOMTONE_VERSION=0.5.1 && set ROOMTONE_SRC=<repo root> && iscc packaging\windows\roomtone.iss
; (or pass /DAppVersion=... /DSourceDir=... instead of the two variables)
; The installer is per-user by default (no administrator rights needed) and not code-signed,
; so Windows SmartScreen may ask you to confirm the first run ("More info" › "Run anyway").
#ifndef AppVersion
  #define AppVersion GetEnv("ROOMTONE_VERSION")
  #if AppVersion == ""
    #undef AppVersion
    #define AppVersion "0.0.0"
  #endif
#endif
#ifndef SourceDir
  #define SourceDir GetEnv("ROOMTONE_SRC")
  #if SourceDir == ""
    #undef SourceDir
    #define SourceDir "..\.."
  #endif
#endif

[Setup]
AppId={{6B0F2C2E-5D0B-4E7B-9B7A-3C1A8F1E7A52}
AppName=Roomtone
AppVersion={#AppVersion}
AppPublisher=Roomtone
AppPublisherURL=https://github.com/ChrispyGee/Roomtone
AppSupportURL=https://github.com/ChrispyGee/Roomtone/issues
DefaultDirName={autopf}\Roomtone
DefaultGroupName=Roomtone
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
LicenseFile={#SourceDir}\LICENSE-MIT
OutputDir={#SourceDir}\dist
OutputBaseFilename=roomtone-v{#AppVersion}-windows-x64-setup
SetupIconFile={#SourceDir}\dist\roomtone.ico
UninstallDisplayIcon={app}\roomtone.ico
Compression=lzma2
SolidCompression=yes
WizardStyle=modern

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; Flags: unchecked

[Files]
Source: "{#SourceDir}\target\release\roomtone.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\dist\roomtone.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\LICENSE-MIT"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\LICENSE-APACHE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Roomtone"; Filename: "{app}\roomtone.exe"; IconFilename: "{app}\roomtone.ico"
Name: "{autodesktop}\Roomtone"; Filename: "{app}\roomtone.exe"; IconFilename: "{app}\roomtone.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\roomtone.exe"; Description: "Start Roomtone"; Flags: nowait postinstall skipifsilent
