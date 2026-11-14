; Standalone native packaging. No Tauri bundler, WebView runtime, elevation,
; process-name termination, data deletion or recursive cleanup.
Unicode true
!include MUI2.nsh
!include LogicLib.nsh
!include FileFunc.nsh
!include x64.nsh
!include Sections.nsh
!include WordFunc.nsh
!include Win\COM.nsh
!include nsDialogs.nsh
!ifndef VERSION
!error "VERSION is required"
!endif
!ifndef BINARY
!error "BINARY is required"
!endif
!ifndef OUTPUT
!error "OUTPUT is required"
!endif
!define PRODUCT "Aurora Launcher"
; Keep the established installation location and package/data identity. This is
; an internal compatibility path, not the application or download name.
!define INSTALL_FOLDER "Aurora Launcher Native Preview"
!define EXECUTABLE "aurora-launcher.exe"
!define LEGACY_EXECUTABLE "aurora-native.exe"
!ifdef ACCEPTANCE_ROOT
!define PACKAGE_ID "com.aurora.launcher.native-preview.acceptance.${ACCEPTANCE_ID}"
!else
!define PACKAGE_ID "com.aurora.launcher.native-preview"
!endif
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PACKAGE_ID}"
Name "${PRODUCT}"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\${INSTALL_FOLDER}"
RequestExecutionLevel user
SetCompressor /SOLID lzma
Icon "..\assets\icon.ico"
UninstallIcon "..\assets\icon.ico"
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "${PRODUCT}"
VIAddVersionKey "FileDescription" "Aurora Launcher installer"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "Aurora Launcher contributors"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\..\LICENSE"
Page custom UpgradePage UpgradeLeave
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
Var Parameters
Var StartSlot
Var DesktopSlot
Var Previous
Var Activated
Var InstalledExe
Var LegacyStartSlot
Var LegacyDesktopSlot
Var RemovePrevious
Var UpgradeCheckbox
Var PriorVersion

Function UpgradePage
  ${If} $PriorVersion == ""
    Abort
  ${EndIf}
  !insertmacro MUI_HEADER_TEXT "Upgrade Aurora Launcher" "Install ${VERSION} and keep your files and data."
  nsDialogs::Create 1018
  Pop $0
  ${NSD_CreateLabel} 0 0 100% 36u "Aurora Launcher $PriorVersion is installed. This upgrade replaces the launcher program. Your instances, mods, worlds, accounts, settings and other app files are preserved."
  Pop $0
  ${NSD_CreateCheckbox} 0 48u 100% 24u "Remove the previous launcher executable (recommended)"
  Pop $UpgradeCheckbox
  ${NSD_SetState} $UpgradeCheckbox $RemovePrevious
  ${NSD_CreateLabel} 0 82u 100% 32u "If unchecked, the previous program is kept as aurora-launcher.previous.exe. It does not create another installed-app entry or change your data."
  Pop $0
  nsDialogs::Show
FunctionEnd

Function UpgradeLeave
  ${NSD_GetState} $UpgradeCheckbox $RemovePrevious
  ${If} $RemovePrevious == 0
  ${AndIf} ${FileExists} "$INSTDIR\aurora-launcher.previous.exe"
    MessageBox MB_OK|MB_ICONEXCLAMATION "A previous launcher backup already exists. Preserve it elsewhere or select the recommended replacement option."
    Abort
  ${EndIf}
FunctionEnd

!macro SlotOwned SLOT
  StrCpy $9 0
  System::Call 'kernel32::GetFileAttributesW(w "${SLOT}") i.r3'
  IntOp $3 $3 & 0x400
  ${If} $3 == 0
    !insertmacro ComHlpr_CreateInProcInstance ${CLSID_ShellLink} ${IID_IShellLink} r0 ""
    ${If} $0 P<> 0
      ${IUnknown::QueryInterface} $0 '(g "${IID_IPersistFile}", *p.r1)'
      ${If} $1 P<> 0
        ${IPersistFile::Load} $1 '(w "${SLOT}", i 0).r3'
        ${If} $3 == 0
          ${IShellLink::GetPath} $0 '(t .r2, i ${NSIS_MAX_STRLEN}, p 0, i 0).r3'
          ${If} $3 == 0
          ${AndIf} $2 == "$INSTDIR\${EXECUTABLE}"
            StrCpy $9 1
          ${ElseIf} $3 == 0
          ${AndIf} $2 == "$INSTDIR\${LEGACY_EXECUTABLE}"
            StrCpy $9 1
          ${EndIf}
        ${EndIf}
        ${IUnknown::Release} $1 ""
      ${EndIf}
      ${IUnknown::Release} $0 ""
    ${EndIf}
  ${EndIf}
!macroend

!macro CheckAncestors PATH
  StrCpy $4 "${PATH}"
  ${Do}
    System::Call 'kernel32::GetFileAttributesW(w r4) i.r5'
    ${If} $5 != -1
      IntOp $5 $5 & 0x400
      ${If} $5 != 0
        Abort "The installation path contains a redirected directory. No files were changed."
      ${EndIf}
    ${EndIf}
    ${GetParent} "$4" $5
    ${If} $5 == $4
      ${ExitDo}
    ${EndIf}
    StrCpy $4 $5
  ${LoopUntil} $4 == ""
!macroend

!macro InitPaths
  SetShellVarContext current
  SetRegView 64
  StrCpy $InstalledExe "${EXECUTABLE}"
  ; Ignore /D: the shipped preview has one distinct, fixed per-user identity.
!ifdef ACCEPTANCE_ROOT
  StrCpy $INSTDIR "${ACCEPTANCE_ROOT}\installation"
  StrCpy $StartSlot "${ACCEPTANCE_ROOT}\start.lnk"
  StrCpy $DesktopSlot "${ACCEPTANCE_ROOT}\desktop.lnk"
  StrCpy $LegacyStartSlot "${ACCEPTANCE_ROOT}\legacy-start.lnk"
  StrCpy $LegacyDesktopSlot "${ACCEPTANCE_ROOT}\legacy-desktop.lnk"
!else
  StrCpy $INSTDIR "$LOCALAPPDATA\${INSTALL_FOLDER}"
  StrCpy $StartSlot "$SMPROGRAMS\${PRODUCT}\${PRODUCT}.lnk"
  StrCpy $DesktopSlot "$DESKTOP\${PRODUCT}.lnk"
  StrCpy $LegacyStartSlot "$SMPROGRAMS\${INSTALL_FOLDER}.lnk"
  StrCpy $LegacyDesktopSlot "$DESKTOP\${INSTALL_FOLDER}.lnk"
!endif
  !insertmacro CheckAncestors "$INSTDIR"
  !insertmacro CheckAncestors "$StartSlot"
  !insertmacro CheckAncestors "$DesktopSlot"
  !insertmacro CheckAncestors "$LegacyStartSlot"
  !insertmacro CheckAncestors "$LegacyDesktopSlot"
  System::Call 'ole32::CoInitializeEx(p 0, i 2)'
!macroend

Function .onInit
  ; The same safe default applies to direct downloads and silent updater installs.
  StrCpy $RemovePrevious 1
  StrCpy $PriorVersion ""
  ${IfNot} ${RunningX64}
    Abort "Aurora Launcher requires Windows x64."
  ${EndIf}
  !insertmacro InitPaths
  System::Call 'kernel32::CreateMutexW(p 0, i 1, w "Local\AuroraNativeInstaller.${PACKAGE_ID}") p.r0 ?e'
  Pop $1
  ${If} $0 P= 0
  ${OrIf} $1 == 183
    Abort "Another Aurora Launcher installer is running."
  ${EndIf}
  !insertmacro CheckAncestors "$INSTDIR\${EXECUTABLE}"
  !insertmacro CheckAncestors "$INSTDIR\${LEGACY_EXECUTABLE}"
  !insertmacro CheckAncestors "$INSTDIR\LICENSE.txt"
  !insertmacro CheckAncestors "$INSTDIR\THIRD_PARTY_NOTICES.txt"
  !insertmacro CheckAncestors "$INSTDIR\MaterialSymbols-LICENSE.txt"
  !insertmacro CheckAncestors "$INSTDIR\native-package.ini"
  !insertmacro CheckAncestors "$INSTDIR\uninstall.exe"
  !insertmacro CheckAncestors "$INSTDIR\aurora-launcher.previous.exe"
  ${GetParameters} $Parameters
  ClearErrors
  ${GetOptions} $Parameters "/KEEPPREVIOUS" $0
  ${IfNot} ${Errors}
    StrCpy $RemovePrevious 0
  ${EndIf}
  ${GetOptions} $Parameters "/UPDATEPID=" $6
  ${If} $6 != ""
    IntOp $7 $6 + 0
    ${If} $7 != $6
    ${OrIf} $7 < 1
      Abort "The updating launcher could not be identified. Install manually."
    ${EndIf}
    ; Open the launcher once by its exact PID. Wait on that handle, never
    ; discover or terminate by name. The caller exits right after spawning us.
    System::Call 'kernel32::OpenProcess(i 0x100000, i 0, i r6) p.r7 ?e'
    Pop $9
    ${If} $7 P= 0
      ; 87: no process has this PID any more, so the launcher already exited
      ; before we could open it. Any other failure is unexplained: stop. The
      ; in-use probe below still refuses while the installed executable runs.
      ${If} $9 != 87
        Abort "The updating launcher could not be identified. Install manually."
      ${EndIf}
    ${Else}
      System::Call 'kernel32::WaitForSingleObject(p r7, i 30000) i.r8'
      System::Call 'kernel32::CloseHandle(p r7)'
      ${If} $8 != 0
        Abort "The launcher did not exit. Its installation is unchanged."
      ${EndIf}
    ${EndIf}
  ${EndIf}
  IfFileExists "$INSTDIR\*" 0 slots
  ReadINIStr $0 "$INSTDIR\native-package.ini" "Package" "Identity"
  ${If} $0 != "${PACKAGE_ID}"
    ; Uninstall deliberately leaves unrelated files. A fresh install may add
    ; its fixed files only when every owned slot is absent; unknown/malformed
    ; package evidence or any collision remains a hard conflict.
    IfFileExists "$INSTDIR\native-package.ini" unknown
    IfFileExists "$INSTDIR\${EXECUTABLE}" unknown
    IfFileExists "$INSTDIR\${LEGACY_EXECUTABLE}" unknown
    IfFileExists "$INSTDIR\uninstall.exe" unknown
    IfFileExists "$INSTDIR\LICENSE.txt" unknown
    IfFileExists "$INSTDIR\THIRD_PARTY_NOTICES.txt" unknown
    IfFileExists "$INSTDIR\MaterialSymbols-LICENSE.txt" unknown
    IfFileExists "$INSTDIR\native-new.exe" unknown
    IfFileExists "$INSTDIR\native-previous.exe" unknown
    Goto slots
    unknown:
    Abort "This directory contains unrecognized launcher package files. No files were changed."
  ${EndIf}
  ReadINIStr $0 "$INSTDIR\native-package.ini" "Package" "Version"
  StrCpy $PriorVersion $0
  ${VersionCompare} "$0" "${VERSION}" $1
  ${If} $1 == 1
    Abort "A newer Aurora Launcher version is installed. Downgrades require a separately reviewed rollback."
  ${EndIf}
  ; Only a recognized prior package may supply the old executable slot.
  ; Both names at once are ambiguous; preserve them for explicit recovery.
  IfFileExists "$INSTDIR\${LEGACY_EXECUTABLE}" 0 probe
  IfFileExists "$INSTDIR\${EXECUTABLE}" ambiguous
  ${If} $0 != "1.4.2"
  ${AndIf} $0 != "1.4.3"
  ${AndIf} $0 != "2.0.0"
  ${AndIf} $0 != "2.0.1"
  ${AndIf} $0 != "2.1.0"
    Abort "The earlier installation version is unrecognized. Existing files were preserved."
  ${EndIf}
  StrCpy $InstalledExe "${LEGACY_EXECUTABLE}"
  Goto probe
  ambiguous:
  Abort "Both launcher executable names exist. Preserve them and review the installation before retrying."
  probe:
  IfFileExists "$INSTDIR\$InstalledExe" 0 slots
  System::Call 'kernel32::CreateFileW(w "$INSTDIR\$InstalledExe", i 0x40010000, i 1, p 0, i 3, i 0, p 0) p.r0'
  ${If} $0 P= -1
    Abort "Close Aurora Launcher before installing."
  ${EndIf}
  System::Call 'kernel32::CloseHandle(p r0)'
  slots:
  IfFileExists "$StartSlot" 0 desktop
  !insertmacro SlotOwned "$StartSlot"
  ${If} $9 != 1
    Abort "The Start menu shortcut is owned by another application."
  ${EndIf}
  desktop:
  IfFileExists "$DesktopSlot" 0 done
  !insertmacro SlotOwned "$DesktopSlot"
  ${If} $9 != 1
    ; The archived launcher may own the unqualified desktop name. Keep it.
    StrCpy $DesktopSlot ""
  ${EndIf}
  done:
  ${If} $RemovePrevious == 0
  ${AndIf} $PriorVersion != ""
  ${AndIf} ${FileExists} "$INSTDIR\aurora-launcher.previous.exe"
    Abort "A previous launcher backup already exists. Existing files were preserved."
  ${EndIf}
FunctionEnd

Section "Aurora Launcher" Main
  SectionIn RO
  ; Refuse unknown interrupted transactions. Preserve all recovery evidence.
  IfFileExists "$INSTDIR\native-new.exe" conflict
  IfFileExists "$INSTDIR\native-previous.exe" conflict
  CreateDirectory "$INSTDIR"
  SetOutPath "$INSTDIR"
  ClearErrors
  File /oname=native-new.exe "${BINARY}"
  IfErrors failed
  StrCpy $Previous 0
  StrCpy $Activated 0
  IfFileExists "$INSTDIR\$InstalledExe" 0 activate
  ClearErrors
  Rename "$INSTDIR\$InstalledExe" "$INSTDIR\native-previous.exe"
  IfErrors failed
  StrCpy $Previous 1
  activate:
  ClearErrors
  Rename "$INSTDIR\native-new.exe" "$INSTDIR\${EXECUTABLE}"
  IfErrors restore
  StrCpy $Activated 1
  File /oname=LICENSE.txt "..\..\LICENSE"
  File /oname=THIRD_PARTY_NOTICES.txt "..\THIRD_PARTY_NOTICES.md"
  File /oname=MaterialSymbols-LICENSE.txt "..\assets\MaterialSymbols-LICENSE.txt"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteINIStr "$INSTDIR\native-package.ini" "Package" "Identity" "${PACKAGE_ID}"
  WriteINIStr "$INSTDIR\native-package.ini" "Package" "Version" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "${PRODUCT}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "Aurora Launcher contributors"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\${EXECUTABLE}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '$\"$INSTDIR\uninstall.exe$\"'
  WriteRegStr HKCU "${UNINSTALL_KEY}" "QuietUninstallString" '$\"$INSTDIR\uninstall.exe$\" /S'
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
!ifndef ACCEPTANCE_ROOT
  CreateDirectory "$SMPROGRAMS\${PRODUCT}"
!endif
  CreateShortcut "$StartSlot" "$INSTDIR\${EXECUTABLE}"
  IfErrors restore
  ; Retire only shortcuts proven to target this package's exact directory.
  !insertmacro SlotOwned "$LegacyStartSlot"
  ${If} $9 == 1
    Delete "$LegacyStartSlot"
  ${EndIf}
  !insertmacro SlotOwned "$LegacyDesktopSlot"
  ${If} $9 == 1
    ${If} $DesktopSlot != ""
      CreateShortcut "$DesktopSlot" "$INSTDIR\${EXECUTABLE}"
      IfErrors restore
    ${EndIf}
    Delete "$LegacyDesktopSlot"
  ${EndIf}
  ; Retire only this transaction's old program after successful activation.
  ; The optional backup never changes data or installs a second app identity.
  ClearErrors
  ${If} $RemovePrevious == 1
    Delete "$INSTDIR\native-previous.exe"
  ${ElseIf} $Previous == 1
    Rename "$INSTDIR\native-previous.exe" "$INSTDIR\aurora-launcher.previous.exe"
  ${EndIf}
  IfErrors retirement_failed
  Goto done
  retirement_failed:
  SetErrorLevel 2
  Abort "The new launcher is installed, but the previous program could not be retired. Existing files and recovery evidence were preserved."
  restore:
  ${If} $Activated == 1
    Delete "$INSTDIR\${EXECUTABLE}"
  ${EndIf}
  ${If} $Previous == 1
    Rename "$INSTDIR\native-previous.exe" "$INSTDIR\$InstalledExe"
  ${EndIf}
  failed:
  Delete "$INSTDIR\native-new.exe"
  SetErrorLevel 2
  Abort "Installation failed. Close Aurora Launcher and retry. Recovery files, if present, have been preserved."
  conflict:
  SetErrorLevel 3
  Abort "An interrupted Aurora Launcher installation needs review. Existing files have been preserved."
  done:
SectionEnd

Section /o "Desktop shortcut" Desktop
  ${If} $DesktopSlot != ""
    CreateShortcut "$DesktopSlot" "$INSTDIR\${EXECUTABLE}"
  ${Else}
    DetailPrint "The existing desktop shortcut belongs to another application and was preserved."
  ${EndIf}
SectionEnd

Function .onInstSuccess
  ${GetOptions} $Parameters "/RESTART" $0
  ${IfNot} ${Errors}
    Exec '$\"$INSTDIR\${EXECUTABLE}$\"'
  ${EndIf}
FunctionEnd

Function un.onInit
  !insertmacro InitPaths
  System::Call 'kernel32::CreateMutexW(p 0, i 1, w "Local\AuroraNativeInstaller.${PACKAGE_ID}") p.r0 ?e'
  Pop $1
  ${If} $0 P= 0
  ${OrIf} $1 == 183
    SetErrorLevel 2
    Abort "Another Aurora Launcher installer is running. Close it before uninstalling."
  ${EndIf}
  !insertmacro CheckAncestors "$INSTDIR\${EXECUTABLE}"
  !insertmacro CheckAncestors "$INSTDIR\${LEGACY_EXECUTABLE}"
  !insertmacro CheckAncestors "$INSTDIR\native-package.ini"
  !insertmacro CheckAncestors "$INSTDIR\LICENSE.txt"
  !insertmacro CheckAncestors "$INSTDIR\THIRD_PARTY_NOTICES.txt"
  !insertmacro CheckAncestors "$INSTDIR\MaterialSymbols-LICENSE.txt"
  ReadINIStr $0 "$INSTDIR\native-package.ini" "Package" "Identity"
  ${If} $0 != "${PACKAGE_ID}"
    Abort "Aurora Launcher installation ownership could not be verified."
  ${EndIf}
FunctionEnd

Section "Uninstall"
  ; No delete-data option. No wildcard Delete or recursive RMDir. Sentinel or
  ; unknown files keep the install directory present; all game data is outside.
  Delete "$INSTDIR\${EXECUTABLE}"
  IfFileExists "$INSTDIR\${EXECUTABLE}" 0 shortcuts
  SetErrorLevel 2
  Abort "Close Aurora Launcher before uninstalling. Data is preserved."
  shortcuts:
  !insertmacro SlotOwned "$StartSlot"
  ${If} $9 == 1
    Delete "$StartSlot"
  ${EndIf}
  !insertmacro SlotOwned "$DesktopSlot"
  ${If} $9 == 1
    Delete "$DesktopSlot"
  ${EndIf}
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\THIRD_PARTY_NOTICES.txt"
  Delete "$INSTDIR\MaterialSymbols-LICENSE.txt"
  Delete "$INSTDIR\native-package.ini"
  Delete "$INSTDIR\uninstall.exe"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
  RMDir "$INSTDIR"
!ifndef ACCEPTANCE_ROOT
  RMDir "$SMPROGRAMS\${PRODUCT}"
!endif
SectionEnd
