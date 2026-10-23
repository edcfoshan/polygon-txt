; 更新模式默认跳过重建快捷方式，但历史自愈可能把标准入口改到了便携版。
; 只纠正已经存在的标准入口，保留 /NS 和桌面快捷方式的创建选择。
!macro JisigRepairShortcut LINK
  ${If} ${FileExists} "${LINK}"
    CreateShortcut "${LINK}" "$INSTDIR\${MAINBINARYNAME}.exe" "" "$INSTDIR\${MAINBINARYNAME}.exe" 0
    !insertmacro SetLnkAppUserModelId "${LINK}"
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; /UPDATE 默认不创建入口；自管更新通过 /NS 区分是否勾选创建。
  ${If} $UpdateMode = 1
  ${AndIf} $NoShortcutMode != 1
    CreateShortcut "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe" "" "$INSTDIR\${MAINBINARYNAME}.exe" 0
    !insertmacro SetLnkAppUserModelId "$SMPROGRAMS\${PRODUCTNAME}.lnk"
    CreateShortcut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe" "" "$INSTDIR\${MAINBINARYNAME}.exe" 0
    !insertmacro SetLnkAppUserModelId "$DESKTOP\${PRODUCTNAME}.lnk"
  ${EndIf}
  !insertmacro JisigRepairShortcut "$SMPROGRAMS\${PRODUCTNAME}.lnk"
  ${If} $AppStartMenuFolder != ""
    !insertmacro JisigRepairShortcut "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk"
  ${EndIf}
  !insertmacro JisigRepairShortcut "$DESKTOP\${PRODUCTNAME}.lnk"
!macroend
