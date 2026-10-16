Unicode true
!include "MUI2.nsh"
!include "x64.nsh"
!include "LogicLib.nsh"
!define MUI_ICON "..\..\assets\tickreel.ico"
!define MUI_UNICON "..\..\assets\tickreel.ico"
!ifndef VERSION
  !error "VERSION is required"
!endif
!ifndef PAYLOAD
  !error "PAYLOAD is required"
!endif
!ifndef OUTPUT
  !error "OUTPUT is required"
!endif
Name "Tickreel"
OutFile "${OUTPUT}"
RequestExecutionLevel user
InstallDir "$LOCALAPPDATA\Programs\Tickreel"
InstallDirRegKey HKCU "Software\Tickreel" "InstallDir"
SetCompressor /SOLID lzma
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "Tickreel"
VIAddVersionKey "FileDescription" "Tickreel installer"
VIAddVersionKey "FileVersion" "${VERSION}"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\Tickreel.exe"
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "Japanese"
!insertmacro MUI_LANGUAGE "English"

Function .onInit
!ifdef ARM64
  ${IfNot} ${IsNativeARM64}
    MessageBox MB_ICONSTOP "This package requires Windows ARM64."
    Abort
  ${EndIf}
!else
  ${If} ${IsNativeARM64}
    MessageBox MB_ICONSTOP "Please download the Windows ARM64 installer."
    Abort
  ${EndIf}
!endif
FunctionEnd

Function un.onInit
  IfFileExists "$INSTDIR\Tickreel.exe" 0 uninstall_ready
  ClearErrors
  FileOpen $0 "$INSTDIR\Tickreel.exe" a
  IfErrors uninstall_busy
  FileClose $0
  Goto uninstall_ready
uninstall_busy:
  IfSilent +2
  MessageBox MB_ICONSTOP "Close Tickreel before uninstalling."
  SetErrorLevel 1
  Abort
uninstall_ready:
FunctionEnd

Section "Tickreel" Main
  ; Detect an active executable before touching any existing installation.
  IfFileExists "$INSTDIR\Tickreel.exe" 0 ready
  ClearErrors
  FileOpen $0 "$INSTDIR\Tickreel.exe" a
  IfErrors busy
  FileClose $0
  Goto ready
busy:
  MessageBox MB_ICONSTOP "Close Tickreel before installing."
  Abort
ready:
  SetOutPath "$INSTDIR"
!ifdef ARM64
  File /r "${PAYLOAD}\windows-arm64\*"
!else
  ${If} ${RunningX64}
    File /r "${PAYLOAD}\windows-x64\*"
  ${Else}
    File /r "${PAYLOAD}\windows-x86\*"
  ${EndIf}
!endif
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateShortcut "$SMPROGRAMS\Tickreel.lnk" "$INSTDIR\Tickreel.exe"
  WriteRegStr HKCU "Software\Tickreel" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Tickreel" "DisplayName" "Tickreel"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Tickreel" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Tickreel" "Publisher" "appipinopi"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Tickreel" "DisplayIcon" "$INSTDIR\Tickreel.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Tickreel" "UninstallString" '$\"$INSTDIR\Uninstall.exe$\"'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Tickreel" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Tickreel" "NoRepair" 1
SectionEnd

Section "Uninstall"
  Delete "$SMPROGRAMS\Tickreel.lnk"
  Delete "$INSTDIR\Tickreel.exe"
  Delete "$INSTDIR\ffmpeg.exe"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\Uninstall.exe"
  ; Delete only files installed by this installer; recordings and other files remain.
  !include "${PAYLOAD}\uninstall-licenses.nsh"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Tickreel"
  DeleteRegKey HKCU "Software\Tickreel"
SectionEnd
