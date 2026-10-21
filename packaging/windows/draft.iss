; Draft per-user installer.
;
; Build with scripts/build-installer.ps1. The app installs under
; %LOCALAPPDATA%\Programs\Draft so it never needs an administrator prompt, and
; user data stays in %LOCALAPPDATA%\Draft4 where the app expects it.

#ifndef AppVersion
  #define AppVersion "0.2.0"
#endif
#ifndef SourceExe
  #define SourceExe "..\..\dist\draft.exe"
#endif
#ifndef RepoRoot
  #define RepoRoot "..\.."
#endif

[Setup]
AppId={{B6F3A2C1-7D64-4E9A-9B3F-2C8E5A1D4F70}
AppName=Draft
AppVersion={#AppVersion}
AppVerName=Draft {#AppVersion}
AppPublisher=Tirup Mehta
AppPublisherURL=https://github.com/TirupMehta/draft-rs
AppSupportURL=https://github.com/TirupMehta/draft-rs/issues
LicenseFile={#RepoRoot}\LICENSE
DefaultDirName={localappdata}\Programs\Draft
DefaultGroupName=Draft
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputDir={#RepoRoot}\dist
OutputBaseFilename=Draft-Setup-{#AppVersion}
SetupIconFile={#RepoRoot}\assets\draft.ico
UninstallDisplayIcon={app}\draft.exe
UninstallDisplayName=Draft
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no
VersionInfoVersion={#AppVersion}.0
VersionInfoDescription=Draft installer
VersionInfoProductName=Draft
VersionInfoProductVersion={#AppVersion}
VersionInfoCompany=Tirup Mehta
VersionInfoCopyright=Copyright (c) 2026 Tirup Mehta

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: checkedonce

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RepoRoot}\LICENSE"; DestDir: "{app}\licenses"; DestName: "LICENSE.txt"; Flags: ignoreversion
Source: "{#RepoRoot}\NOTICE"; DestDir: "{app}\licenses"; DestName: "NOTICE.txt"; Flags: ignoreversion
Source: "{#RepoRoot}\CREDITS.md"; DestDir: "{app}\licenses"; Flags: ignoreversion
Source: "{#RepoRoot}\THIRD_PARTY_NOTICES.md"; DestDir: "{app}\licenses"; Flags: ignoreversion
Source: "{#RepoRoot}\assets\fonts\Inter-OFL.txt"; DestDir: "{app}\licenses"; DestName: "Inter-OFL.txt"; Flags: ignoreversion
Source: "{#RepoRoot}\assets\fonts\JetBrainsMono-OFL.txt"; DestDir: "{app}\licenses"; DestName: "JetBrainsMono-OFL.txt"; Flags: ignoreversion
#ifdef IncludeModel
Source: "{#ModelPath}"; DestDir: "{localappdata}\Draft4\models"; Flags: onlyifdoesntexist
#endif

[InstallDelete]
Type: files; Name: "{app}\licenses\LICENSE-MIT.txt"
Type: files; Name: "{app}\licenses\LICENSE-APACHE.txt"

[Icons]
Name: "{autoprograms}\Draft"; Filename: "{app}\draft.exe"; WorkingDir: "{app}"
Name: "{autodesktop}\Draft"; Filename: "{app}\draft.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Registry]
; Sign-in launches stay in the tray. The app also rewrites this value after an
; in-place update so it always points at the current executable.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Draft"; ValueData: """{app}\draft.exe"" --background"; Flags: uninsdeletevalue

[Run]
Filename: "{app}\draft.exe"; Description: "Launch Draft"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; The app writes logs and models under %LOCALAPPDATA%\Draft4, which is user
; data and intentionally survives uninstall.
