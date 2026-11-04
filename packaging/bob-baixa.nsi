Unicode true
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"

!ifndef EXE
  !define EXE "..\src-tauri\target\x86_64-pc-windows-msvc\release\bob-baixa.exe"
!endif
!ifndef EXTDIR
  !define EXTDIR "..\extension"
!endif
!ifndef ICON
  !define ICON "..\src-tauri\icons\icon.ico"
!endif

Name "bob baixa"
OutFile "bob-baixa-setup.exe"
InstallDir "$LOCALAPPDATA\bob baixa"
InstallDirRegKey HKCU "Software\bob baixa" "InstallDir"
RequestExecutionLevel user
ShowInstDetails show
ShowUninstDetails show

!define MUI_ICON "${ICON}"
!define MUI_UNICON "${ICON}"
!define MUI_ABORTWARNING

!define MUI_WELCOMEPAGE_TITLE "bob baixa"
!define MUI_WELCOMEPAGE_TEXT "este assistente instala o bob baixa.$\r$\n$\r$\nna primeira abertura o programa baixa o yt-dlp e o ffmpeg sozinho."
!define MUI_COMPONENTSPAGE_TITLE "componentes"
!define MUI_COMPONENTSPAGE_SUBTITLE "escolha o que instalar"
!define MUI_DIRECTORYPAGE_TEXT_TOP "escolha a pasta do bob baixa."
!define MUI_DIRECTORYPAGE_TEXT_DESTINATION "pasta"
!define MUI_FINISHPAGE_TITLE "o bob baixa foi instalado"
!define MUI_FINISHPAGE_TEXT "pode abrir o programa agora."
!define MUI_FINISHPAGE_RUN "$INSTDIR\bob-baixa.exe"
!define MUI_FINISHPAGE_RUN_TEXT "abrir o bob baixa"
!define MUI_UNCONFIRMPAGE_TEXT_TOP "o bob baixa será removido. os vídeos já baixados continuam na pasta escolhida."

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "PortugueseBR"
!insertmacro MUI_LANGUAGE "English"

Section "bob baixa" SecApp
  SectionIn RO
  SetOutPath "$INSTDIR"
  File "${EXE}"
  SetOutPath "$INSTDIR\extension"
  File "${EXTDIR}\manifest.json"
  File "${EXTDIR}\background.js"
  File "${EXTDIR}\content.js"
  File "${EXTDIR}\popup.html"
  File "${EXTDIR}\popup.js"
  File "${EXTDIR}\LEIA-ME.txt"
  File "${EXTDIR}\icon32.png"
  File "${EXTDIR}\icon128.png"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateDirectory "$SMPROGRAMS\bob baixa"
  CreateShortcut "$SMPROGRAMS\bob baixa\bob baixa.lnk" "$INSTDIR\bob-baixa.exe" "" "$INSTDIR\bob-baixa.exe" 0
  CreateShortcut "$SMPROGRAMS\bob baixa\desinstalar.lnk" "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\bob baixa" "DisplayName" "bob baixa"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\bob baixa" "UninstallString" "$\"$INSTDIR\Uninstall.exe$\""
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\bob baixa" "DisplayIcon" "$INSTDIR\bob-baixa.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\bob baixa" "Publisher" "bob baixa"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\bob baixa" "DisplayVersion" "1.0.0"
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\bob baixa" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\bob baixa" "NoRepair" 1
  WriteRegStr HKCU "Software\bob baixa" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Classes\bobbaixa" "" "URL:bob baixa"
  WriteRegStr HKCU "Software\Classes\bobbaixa" "URL Protocol" ""
  WriteRegStr HKCU "Software\Classes\bobbaixa\DefaultIcon" "" "$INSTDIR\bob-baixa.exe,0"
  WriteRegStr HKCU "Software\Classes\bobbaixa\shell\open\command" "" "$\"$INSTDIR\bob-baixa.exe$\" $\"%1$\""
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\bob baixa" "EstimatedSize" "$0"
SectionEnd

Section "atalho na área de trabalho" SecDesktop
  CreateShortcut "$DESKTOP\bob baixa.lnk" "$INSTDIR\bob-baixa.exe" "" "$INSTDIR\bob-baixa.exe" 0
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\bob-baixa.exe"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir /r "$INSTDIR\extension"
  Delete "$DESKTOP\bob baixa.lnk"
  Delete "$SMPROGRAMS\bob baixa\bob baixa.lnk"
  Delete "$SMPROGRAMS\bob baixa\desinstalar.lnk"
  RMDir "$SMPROGRAMS\bob baixa"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\bob baixa"
  DeleteRegKey HKCU "Software\Classes\bobbaixa"
  DeleteRegKey HKCU "Software\bob baixa"
  RMDir "$INSTDIR"
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecApp} "o programa, o menu iniciar e o protocolo bobbaixa://"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} "cria um atalho na área de trabalho"
!insertmacro MUI_FUNCTION_DESCRIPTION_END
