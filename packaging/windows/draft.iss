; Draft per-user installer.
;
; Build with scripts/build-installer.ps1. The app installs under
; %LOCALAPPDATA%\Programs\Draft so it never needs an administrator prompt, and
; user data lives in %LOCALAPPDATA%\Draft4 where the app expects it.
;
; Uninstall removes the app, its license texts, and the shortcuts this project
; has used over time. It deliberately keeps %LOCALAPPDATA%\Draft4 (settings,
; history, logs, and the model) so reinstalling does not destroy local data.

#ifndef AppVersion
  #define AppVersion "0.3.1"
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
; The model lands in the app's data folder, not the install folder, so the
; uninstaller must leave it alone: the confirmation promises to keep it, and a
; reinstall should not have to re-extract 1.5 GB. Inno otherwise registers every
; file it copies for removal, which deleted the model on uninstall.
Source: "{#ModelPath}"; DestDir: "{localappdata}\Draft4\models"; Flags: onlyifdoesntexist uninsneveruninstall
#endif

[InstallDelete]
Type: files; Name: "{app}\licenses\LICENSE-MIT.txt"
Type: files; Name: "{app}\licenses\LICENSE-APACHE.txt"
; An older update path left the previous executable beside the new one. Clear it
; so an upgrade never leaves a second copy of Draft in the install folder.
Type: files; Name: "{app}\draft.prev.exe"
; Earlier builds put a single shortcut at the top of the Start menu. The app
; and its uninstaller now share one Draft folder.
Type: files; Name: "{autoprograms}\Draft.lnk"

[Icons]
Name: "{group}\Draft"; Filename: "{app}\draft.exe"; WorkingDir: "{app}"
Name: "{group}\Uninstall Draft"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Draft"; Filename: "{app}\draft.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Registry]
; Sign-in launches stay in the tray. The app also rewrites this value after an
; in-place update so it always points at the current executable.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Draft"; ValueData: """{app}\draft.exe"" --background"; Flags: uninsdeletevalue

[Run]
Filename: "{app}\draft.exe"; Description: "Launch Draft"; Flags: nowait postinstall skipifsilent

[Messages]
; Be explicit about what stays. Local data is never part of the uninstall.
ConfirmUninstall=Draft and its installed files will be removed. Your settings, history, and the speech model in %LOCALAPPDATA%\Draft4 will be kept. Do you want to continue?

[UninstallDelete]
; Local data in %LOCALAPPDATA%\Draft4 is intentionally preserved. Only remove
; leftovers from earlier layouts and the top-level Start-menu shortcut that an
; older build created outside the Draft folder. Inno tracks everything it
; installed itself, including the Draft group and the desktop shortcut.
Type: files; Name: "{autoprograms}\Draft.lnk"
Type: files; Name: "{app}\draft.prev.exe"
