!macro NSIS_HOOK_PREINSTALL
  ; Check for VC++ Redistributable (required by ORT, OpenVINO, DirectML DLLs)
  IfFileExists "$SYSDIR\vcruntime140.dll" vcredist_ok
    ; VC++ Runtime not found — install it
    SetDetailsPrint textonly
    DetailPrint "Installing Visual C++ Redistributable..."
    SetDetailsPrint listonly
    File "/oname=$TEMP\vc_redist.x64.exe" "${NSISDIR}\..\prereqs\vc_redist.x64.exe"
    nsExec::ExecToLog '"$TEMP\vc_redist.x64.exe" /install /quiet /norestart'
    Pop $0
    Delete "$TEMP\vc_redist.x64.exe"
  vcredist_ok:
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Write breadcrumb so the app knows where the installer was launched from.
  ; $EXEDIR = directory containing the installer .exe (e.g., E:\SmolPC-Lite\)
  CreateDirectory "$LOCALAPPDATA\SmolPC"
  FileOpen $0 "$LOCALAPPDATA\SmolPC\installer-source.txt" w
  FileWrite $0 "$EXEDIR"
  FileClose $0
!macroend
