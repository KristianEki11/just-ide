; Just IDE - offline per-user installer (no UAC, no network access).
; Build: makensis /DVERSION=0.1.0 installer\just-ide.nsi
Unicode true
ManifestDPIAware true
SetCompressor /SOLID lzma

!ifndef VERSION
  !define VERSION "0.1.0"
!endif
!define APPNAME "Just IDE"
!define APPKEY "JustIDE"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPKEY}"
!ifndef SRCEXE
  !define SRCEXE "..\target\release\jide.exe"
!endif
!ifndef OUTDIR
  !define OUTDIR "..\dist"
!endif

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "nsDialogs.nsh"
!include "WordFunc.nsh"

Name "${APPNAME}"
OutFile "${OUTDIR}\JustIDE-${VERSION}-x64-setup.exe"
InstallDir "$LOCALAPPDATA\Programs\${APPKEY}"
InstallDirRegKey HKCU "Software\${APPKEY}" "InstallDir"
RequestExecutionLevel user
BrandingText "${APPNAME} ${VERSION}"

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "${APPNAME}"
VIAddVersionKey "FileDescription" "${APPNAME} installer"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "MIT OR Apache-2.0"

!define MUI_ICON "..\assets\icons\jide.ico"
!define MUI_UNICON "..\assets\icons\jide.ico"
!define MUI_ABORTWARNING
!define MUI_COMPONENTSPAGE_SMALLDESC

Var RemoveUserData
Var RemoveCheckbox

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\jide.exe"
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
UninstPage custom un.SettingsPageCreate un.SettingsPageLeave
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

; Common source/config extensions offered by the (opt-in) association section.
!macro EachExtension MACRO
  !insertmacro ${MACRO} ".txt"
  !insertmacro ${MACRO} ".md"
  !insertmacro ${MACRO} ".json"
  !insertmacro ${MACRO} ".toml"
  !insertmacro ${MACRO} ".yaml"
  !insertmacro ${MACRO} ".yml"
  !insertmacro ${MACRO} ".rs"
  !insertmacro ${MACRO} ".js"
  !insertmacro ${MACRO} ".ts"
  !insertmacro ${MACRO} ".tsx"
  !insertmacro ${MACRO} ".py"
  !insertmacro ${MACRO} ".php"
  !insertmacro ${MACRO} ".html"
  !insertmacro ${MACRO} ".css"
  !insertmacro ${MACRO} ".xml"
  !insertmacro ${MACRO} ".log"
  !insertmacro ${MACRO} ".ini"
  !insertmacro ${MACRO} ".sh"
!macroend

!macro AssociateExt EXT
  WriteRegStr HKCU "Software\Classes\${EXT}\OpenWithProgids" "${APPKEY}.File" ""
!macroend

!macro UnassociateExt EXT
  DeleteRegValue HKCU "Software\Classes\${EXT}\OpenWithProgids" "${APPKEY}.File"
!macroend

Function BroadcastEnvChange
  System::Call 'user32::SendMessageTimeoutW(i 0xffff, i 0x001A, i 0, w "Environment", i 0x2, i 5000, *i .r0)'
FunctionEnd

Section "${APPNAME} (required)" SecCore
  SectionIn RO
  SetOutPath "$INSTDIR"
  File "${SRCEXE}"
  File "..\README.md"
  !if /FileExists "..\LICENSE"
    File "..\LICENSE"
  !endif
  File "..\docs\USER-GUIDE.md"

  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\${APPKEY}" "InstallDir" "$INSTDIR"

  WriteRegStr HKCU "${UNINSTKEY}" "DisplayName" "${APPNAME}"
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTKEY}" "Publisher" "Just IDE"
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayIcon" "$INSTDIR\jide.exe"
  WriteRegStr HKCU "${UNINSTKEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTKEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegDWORD HKCU "${UNINSTKEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTKEY}" "NoRepair" 1

  CreateDirectory "$SMPROGRAMS\${APPNAME}"
  CreateShortcut "$SMPROGRAMS\${APPNAME}\${APPNAME}.lnk" "$INSTDIR\jide.exe"
SectionEnd

Section "Add to PATH" SecPath
  ReadRegStr $0 HKCU "Environment" "Path"
  ${If} $0 == ""
    WriteRegExpandStr HKCU "Environment" "Path" "$INSTDIR"
  ${Else}
    ${WordFind} ";$0;" ";$INSTDIR;" "E+1{" $1
    ${If} ${Errors}
      ClearErrors
      WriteRegExpandStr HKCU "Environment" "Path" "$0;$INSTDIR"
    ${EndIf}
  ${EndIf}
  WriteRegDWORD HKCU "Software\${APPKEY}" "PathAdded" 1
  Call BroadcastEnvChange
SectionEnd

Section "Add 'Open with Just IDE' to Explorer context menu" SecContext
  WriteRegStr HKCU "Software\Classes\*\shell\${APPKEY}" "" "Open with ${APPNAME}"
  WriteRegStr HKCU "Software\Classes\*\shell\${APPKEY}" "Icon" "$INSTDIR\jide.exe"
  WriteRegStr HKCU "Software\Classes\*\shell\${APPKEY}\command" "" '"$INSTDIR\jide.exe" "%1"'
  WriteRegStr HKCU "Software\Classes\Directory\shell\${APPKEY}" "" "Open with ${APPNAME}"
  WriteRegStr HKCU "Software\Classes\Directory\shell\${APPKEY}" "Icon" "$INSTDIR\jide.exe"
  WriteRegStr HKCU "Software\Classes\Directory\shell\${APPKEY}\command" "" '"$INSTDIR\jide.exe" "%1"'
  WriteRegStr HKCU "Software\Classes\Directory\Background\shell\${APPKEY}" "" "Open with ${APPNAME}"
  WriteRegStr HKCU "Software\Classes\Directory\Background\shell\${APPKEY}" "Icon" "$INSTDIR\jide.exe"
  WriteRegStr HKCU "Software\Classes\Directory\Background\shell\${APPKEY}\command" "" '"$INSTDIR\jide.exe" "%V"'
SectionEnd

Section "Desktop shortcut" SecDesktop
  CreateShortcut "$DESKTOP\${APPNAME}.lnk" "$INSTDIR\jide.exe"
SectionEnd

Section /o "Associate common file extensions" SecAssoc
  WriteRegStr HKCU "Software\Classes\${APPKEY}.File" "" "${APPNAME} document"
  WriteRegStr HKCU "Software\Classes\${APPKEY}.File\DefaultIcon" "" "$INSTDIR\jide.exe,0"
  WriteRegStr HKCU "Software\Classes\${APPKEY}.File\shell\open\command" "" '"$INSTDIR\jide.exe" "%1"'
  !insertmacro EachExtension AssociateExt
  WriteRegDWORD HKCU "Software\${APPKEY}" "Associated" 1
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecCore} "Installs ${APPNAME} for the current user. No administrator rights needed."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecPath} "Lets you run 'jide' from any terminal (adds the install folder to your user PATH)."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecContext} "Adds 'Open with Just IDE' for files, folders and folder backgrounds."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} "Creates a shortcut on your desktop."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecAssoc} "Offers ${APPNAME} for common text and source file types. Off by default."
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Function un.SettingsPageCreate
  !insertmacro MUI_HEADER_TEXT "Uninstall options" "Choose what to do with your personal data."
  nsDialogs::Create 1018
  Pop $0
  ${If} $0 == error
    Abort
  ${EndIf}
  ${NSD_CreateLabel} 0 0 100% 24u "Your settings, session and backups are kept by default so a reinstall picks up where you left off."
  Pop $0
  ${NSD_CreateCheckbox} 0 34u 100% 12u "Remove user settings && session"
  Pop $RemoveCheckbox
  nsDialogs::Show
FunctionEnd

Function un.SettingsPageLeave
  ${NSD_GetState} $RemoveCheckbox $RemoveUserData
FunctionEnd

Section "Uninstall"
  Delete "$INSTDIR\jide.exe"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\USER-GUIDE.md"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\${APPNAME}\${APPNAME}.lnk"
  RMDir "$SMPROGRAMS\${APPNAME}"
  Delete "$DESKTOP\${APPNAME}.lnk"

  DeleteRegKey HKCU "Software\Classes\*\shell\${APPKEY}"
  DeleteRegKey HKCU "Software\Classes\Directory\shell\${APPKEY}"
  DeleteRegKey HKCU "Software\Classes\Directory\Background\shell\${APPKEY}"

  ReadRegDWORD $0 HKCU "Software\${APPKEY}" "Associated"
  ${If} $0 == 1
    !insertmacro EachExtension UnassociateExt
    DeleteRegKey HKCU "Software\Classes\${APPKEY}.File"
  ${EndIf}

  ReadRegDWORD $0 HKCU "Software\${APPKEY}" "PathAdded"
  ${If} $0 == 1
    ReadRegStr $1 HKCU "Environment" "Path"
    ${WordReplace} ";$1;" ";$INSTDIR;" ";" "+*" $2
    ${WordReplace} $2 ";;" ";" "+*" $2
    StrCpy $3 $2 1
    ${If} $3 == ";"
      StrCpy $2 $2 "" 1
    ${EndIf}
    StrCpy $3 $2 "" -1
    ${If} $3 == ";"
      StrCpy $2 $2 -1
    ${EndIf}
    WriteRegExpandStr HKCU "Environment" "Path" "$2"
    System::Call 'user32::SendMessageTimeoutW(i 0xffff, i 0x001A, i 0, w "Environment", i 0x2, i 5000, *i .r0)'
  ${EndIf}

  DeleteRegKey HKCU "${UNINSTKEY}"
  DeleteRegKey HKCU "Software\${APPKEY}"

  ${If} $RemoveUserData == ${BST_CHECKED}
    RMDir /r "$APPDATA\${APPKEY}"
    RMDir /r "$LOCALAPPDATA\${APPKEY}"
  ${EndIf}
SectionEnd
