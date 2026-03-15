!macro customHeader
  !system "echo NSIS Custom Header loaded"
!macroend

; ── Welcome page customization ─────────────────────────────────────────────
!macro customInit
  ; Show open source notice on the welcome page
!macroend

!macro customInstallMode
  ; Per-user install by default (no admin required)
!macroend

; ── Custom pages ───────────────────────────────────────────────────────────

!macro customPageAfterChangeDir
  ; Open Source & Support page
  !define MUI_PAGE_HEADER_TEXT "Open Source Software"
  !define MUI_PAGE_HEADER_SUBTEXT "GeminiStudio is free and open source"

  Page custom OpenSourcePage

  Function OpenSourcePage
    nsDialogs::Create 1018
    Pop $0

    ${NSD_CreateLabel} 0 0 100% 30u "GeminiStudio is free and open source software (MIT License)."
    Pop $1
    SetCtlColors $1 "" transparent
    ${NSD_AddStyle} $1 ${SS_CENTER}

    ${NSD_CreateLabel} 0 35u 100% 20u "You have the right to use, modify, and distribute this software."
    Pop $2
    SetCtlColors $2 "" transparent
    ${NSD_AddStyle} $2 ${SS_CENTER}

    ${NSD_CreateLabel} 0 60u 100% 20u "Source Code & Support:"
    Pop $3
    SetCtlColors $3 "" transparent
    ${NSD_AddStyle} $3 ${SS_CENTER}

    ${NSD_CreateLink} 0 82u 100% 15u "https://github.com/studio-toolkit/chat-toolkit-rust-mcp"
    Pop $4
    ${NSD_AddStyle} $4 ${SS_CENTER}
    ${NSD_OnClick} $4 OpenGitHub

    ${NSD_CreateLabel} 0 110u 100% 30u "If you find this project useful, please consider giving us a star on GitHub!"
    Pop $5
    SetCtlColors $5 "" transparent
    ${NSD_AddStyle} $5 ${SS_CENTER}

    ${NSD_CreateLabel} 0 145u 100% 15u "Please support us — every contribution helps keep the project alive."
    Pop $6
    SetCtlColors $6 "" transparent
    ${NSD_AddStyle} $6 ${SS_CENTER}

    nsDialogs::Show
  FunctionEnd

  Function OpenGitHub
    Pop $0
    ExecShell "open" "https://github.com/studio-toolkit/chat-toolkit-rust-mcp"
  FunctionEnd
!macroend

; ── Desktop shortcut question ──────────────────────────────────────────────
!macro customPageBeforeInstall
  ; The "createDesktopShortcut" option in electron-builder handles this,
  ; but we ensure it with NSIS too
!macroend

; ── Post-install ───────────────────────────────────────────────────────────
!macro customInstall
  ; Register the Roblox Studio plugin on first run
  ; The app itself handles this via its install flow
!macroend

; ── Uninstall ──────────────────────────────────────────────────────────────
!macro customUnInit
  ; Show confirmation
!macroend

!macro customUnInstall
  ; Clean up app-specific registry entries if any
  DeleteRegKey HKCU "Software\GeminiStudio"
!macroend
