; Draft per-user installer.
;
; Build with scripts/build-installer.ps1. The app installs under
; %LOCALAPPDATA%\Programs\Draft so it never needs an administrator prompt, and
; user data stays in %LOCALAPPDATA%\Draft4 where the app expects it.

#ifndef AppVersion
  #define AppVersion "0.1.0"
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
AppPublisher=Draft
AppPublisherURL=https://github.com/TirupMehta/draft-rs
AppSupportURL=https://github.com/TirupMehta/draft-rs/issues
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

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: checkedonce

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; Flags: ignoreversion
#ifdef IncludeModel
Source: "{#ModelPath}"; DestDir: "{localappdata}\Draft4\models"; Flags: onlyifdoesntexist
#endif

[Icons]
Name: "{autoprograms}\Draft"; Filename: "{app}\draft.exe"; WorkingDir: "{app}"
Name: "{autodesktop}\Draft"; Filename: "{app}\draft.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Run]
Filename: "{app}\draft.exe"; Description: "Launch Draft"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; The app writes logs and models under %LOCALAPPDATA%\Draft4, which is user
; data and intentionally survives uninstall.
