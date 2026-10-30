#ifndef PayloadDir
  #error PayloadDir must name the staged application directory
#endif
#ifndef AppVersion
  #error AppVersion must match Cargo.toml
#endif
#ifndef BuildRevision
  #error BuildRevision must identify the source commit
#endif

[Setup]
AppId={{6AF5C441-7858-4D2F-A489-21831A014B9F}
AppName=Circadia
AppVersion={#AppVersion}
VersionInfoVersion={#BaseVersion}
AppVerName=Circadia {#AppVersion} ({#BuildRevision}, unsigned)
AppPublisher=kvnxiao
AppPublisherURL=https://github.com/kvnxiao/circadia
AppSupportURL=https://github.com/kvnxiao/circadia/issues
DefaultDirName={localappdata}\Programs\Circadia
DisableDirPage=yes
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64os
ArchitecturesInstallIn64BitMode=x64os
MinVersion=10.0.22000
AppMutex=Local\Circadia.Instance.v1
CloseApplications=no
RestartApplications=no
SetupIconFile={#PayloadDir}\circadia.ico
UninstallDisplayIcon={app}\circadia.exe
OutputBaseFilename=Circadia-{#AppVersion}-{#BuildRevision}-windows-x64-unsigned-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
SignedUninstaller=no

[Files]
Source: "{#PayloadDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{userprograms}\Circadia"; Filename: "{app}\circadia.exe"; WorkingDir: "{app}"

[Messages]
SetupAppRunningError=Circadia is running. Choose Quit from its tray menu, then click OK to continue, or Cancel to exit.
UninstallAppRunningError=Circadia is running. Choose Quit from its tray menu, then click OK to continue, or Cancel to exit.

[Code]
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Command: String;
begin
  if CurUninstallStep = usUninstall then
  begin
    if RegQueryStringValue(HKCU64,
      'Software\Microsoft\Windows\CurrentVersion\Run', 'Circadia', Command) then
    begin
      if CompareText(Command, '"' + ExpandConstant('{app}\circadia.exe') + '"') = 0 then
        RegDeleteValue(HKCU64,
          'Software\Microsoft\Windows\CurrentVersion\Run', 'Circadia');
    end;
  end;
end;
