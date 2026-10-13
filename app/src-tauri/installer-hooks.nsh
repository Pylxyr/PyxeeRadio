; Extra installer behaviour, included by Tauri's NSIS bundler (bundle.windows.nsis.installerHooks).
;
; Uninstalling removes the program and nothing else by default: the settings, the queue and the
; Twitch sign-in tokens live in %APPDATA%\TwitchRadio and survive, so a reinstall picks up where
; it left off. This asks once, defaulting to "No", whether to remove them as well. It is not asked
; during an in-place update or a silent uninstall, which never touch that folder.
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    ${IfNot} ${Silent}
      MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 "Also delete your PyxeeRadio settings, song queue and Twitch sign-in tokens?$\r$\n$\r$\nChoose No to keep them in case you install PyxeeRadio again." /SD IDNO IDNO twitchRadioKeepData
        SetShellVarContext current
        RMDir /r "$APPDATA\TwitchRadio"
      twitchRadioKeepData:
    ${EndIf}
  ${EndIf}
!macroend
