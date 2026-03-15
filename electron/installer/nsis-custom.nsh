!macro customHeader
!macroend

!macro customInit
!macroend

!macro customInstallMode
!macroend

!macro customPageAfterChangeDir
  Page custom OpenSourcePage

  Function OpenSourcePage
    nsDialogs::Create 1018
    Pop $0

    ${If} $0 == error
      Abort
    ${EndIf}

    ${NSD_CreateLabel} 0 10u 100% 24u "GeminiStudio is free and open source software (MIT License)."
    Pop $1

    ${NSD_CreateLabel} 0 40u 100% 20u "You have the right to use, modify, and distribute this software."
    Pop $2

    ${NSD_CreateLabel} 0 70u 100% 20u "Source Code && Support:"
    Pop $3

    ${NSD_CreateLink} 0 95u 100% 15u "https://github.com/studio-toolkit/chat-toolkit-rust-mcp"
    Pop $4
    ${NSD_OnClick} $4 OpenGitHub

    ${NSD_CreateLabel} 0 125u 100% 20u "If you find this useful, please give us a star on GitHub!"
    Pop $5

    nsDialogs::Show
  FunctionEnd

  Function OpenGitHub
    Pop $0
    ExecShell "open" "https://github.com/studio-toolkit/chat-toolkit-rust-mcp"
  FunctionEnd
!macroend

!macro customUnInit
!macroend

!macro customUnInstall
  DeleteRegKey HKCU "Software\GeminiStudio"
!macroend
