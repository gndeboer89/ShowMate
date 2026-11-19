; Inno Setup-script voor ShowMate. Bouwen (vanaf de repo-root, na `cargo build --release -p showmate-app`):
;   iscc /DAppVersion=0.1.0 installer\showmate.iss
; Resultaat: dist\ShowMate-Setup-<versie>.exe
;
; mpv (voor video) wordt meegeleverd als dist\mpv\mpv.exe; zie installer\fetch-mpv.ps1.
; Ontbreekt die map, dan wordt de installer zonder mpv gebouwd (video werkt dan alleen als
; mpv elders staat: SHOWMATE_MPV of op het PATH).

#ifndef AppVersion
  #define AppVersion "0.2.0"
#endif

[Setup]
AppId={{6D3B2B5E-7A57-4F7B-9C6C-5E3C0B2D8A11}
AppName=ShowMate
AppVersion={#AppVersion}
AppPublisher=ShowMate
AppCopyright=Copyright (C) 2026 Geert de Boer. GPL-3.0-or-later.
DefaultDirName={autopf}\ShowMate
DefaultGroupName=ShowMate
DisableProgramGroupPage=yes
OutputDir=..\dist
OutputBaseFilename=ShowMate-Setup-{#AppVersion}
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequiredOverridesAllowed=dialog
UninstallDisplayName=ShowMate
UninstallDisplayIcon={app}\showmate.exe
SetupIconFile=..\assets\showmate.ico

[Languages]
Name: "nl"; MessagesFile: "compiler:Languages\Dutch.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\target\release\showmate.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\THIRD-PARTY.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\mpv\*"; DestDir: "{app}\mpv"; Flags: ignoreversion recursesubdirs skipifsourcedoesntexist

[Icons]
Name: "{autoprograms}\ShowMate"; Filename: "{app}\showmate.exe"
Name: "{autodesktop}\ShowMate"; Filename: "{app}\showmate.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\showmate.exe"; Description: "{cm:LaunchProgram,ShowMate}"; Flags: nowait postinstall skipifsilent

; Showfiles en instellingen staan in %APPDATA%\ShowMate en blijven bij verwijderen bewaard.
