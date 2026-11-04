!macro NSIS_HOOK_POSTINSTALL
  CreateShortcut "$DESKTOP\bob baixa.lnk" "$INSTDIR\bob-baixa.exe"
  WriteRegStr HKCU "Software\Classes\bobbaixa" "" "URL:bob baixa"
  WriteRegStr HKCU "Software\Classes\bobbaixa" "URL Protocol" ""
  WriteRegStr HKCU "Software\Classes\bobbaixa\DefaultIcon" "" "$INSTDIR\bob-baixa.exe,0"
  WriteRegStr HKCU "Software\Classes\bobbaixa\shell\open\command" "" '"$INSTDIR\bob-baixa.exe" "%1"'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  Delete "$DESKTOP\bob baixa.lnk"
  DeleteRegKey HKCU "Software\Classes\bobbaixa"
  DeleteRegKey HKCU "Software\Classes\Directory\Background\shell\BobBaixa"
  DeleteRegKey HKCU "Software\Classes\DesktopBackground\Shell\BobBaixa"
  DeleteRegKey HKCU "Software\Classes\LibraryFolder\background\shell\BobBaixa"
!macroend
