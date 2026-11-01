#define MyAppName "ctdozlite"
#ifndef MyAppVersion
  #define MyAppVersion "0.0.1"
#endif
#ifndef BuildDir
  #define BuildDir "..\..\target\release"
#endif
#define MyAppPublisher "ctdoteam1110"
#define MyAppURL "https://github.com/ctdoteam1110/ctdozlite"
#define MyAppExeName "zalo-rust.exe"
; Build with: iscc /DArch=arm64 installer.iss  (default: x64)
#ifndef Arch
  #define Arch "x64"
#endif
#if Arch == "arm64"
  #define ArchSuffix "arm64"
  #define ArchAllowed "arm64"
#else
  #define ArchSuffix "x86_64"
  #define ArchAllowed "x64compatible"
#endif

[Setup]
AppId={{8F4E2D1A-9C3B-4E7F-A6D5-1B2C3D4E5F60}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
OutputDir=..\..\dist
OutputBaseFilename=ctdozlite-setup-windows-{#ArchSuffix}
SetupIconFile=..\..\src\assets\app_icon.ico
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed={#ArchAllowed}
ArchitecturesInstallIn64BitMode={#ArchAllowed}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#BuildDir}\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE-MIT"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE-APACHE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\docs\WINDOWS_ARM64.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Parameters: "--gui"; WorkingDir: "{app}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Parameters: "--gui"; WorkingDir: "{app}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Parameters: "--gui"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; WorkingDir: "{app}"; Flags: nowait postinstall skipifsilent
