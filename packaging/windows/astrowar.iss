#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef SourceDir
  #define SourceDir "."
#endif
#ifndef OutputDir
  #define OutputDir "."
#endif

[Setup]
AppId={{7C2E9A14-5B38-4F0D-9E61-2A8C4D6F0B53}
AppName=AstroWar
AppVersion={#AppVersion}
AppPublisher=AstroWar
DefaultDirName={localappdata}\AstroWar
DisableProgramGroupPage=yes
DisableDirPage=no
OutputDir={#OutputDir}
OutputBaseFilename=astrowar-windows-x86_64-setup
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64
ArchitecturesInstallIn64BitMode=x64
PrivilegesRequired=lowest
UninstallDisplayName=AstroWar
WizardStyle=modern
LicenseFile={#SourceDir}\LICENSE

[Files]
Source: "{#SourceDir}\astrowar.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\AstroWar"; Filename: "{app}\astrowar.exe"

[Run]
Filename: "{app}\astrowar.exe"; Description: "Launch AstroWar"; Flags: nowait postinstall skipifsilent
