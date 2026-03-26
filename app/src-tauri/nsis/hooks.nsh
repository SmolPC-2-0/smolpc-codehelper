!macro NSIS_HOOK_PREINSTALL
  ; Check for VC++ Redistributable (required by ORT, OpenVINO, DirectML DLLs)
  IfFileExists "$SYSDIR\vcruntime140.dll" vcredist_ok
    ; VC++ Runtime not found — try to install it
    ; /nonfatal: build succeeds even if vc_redist.x64.exe wasn't staged
    File /nonfatal "/oname=$TEMP\vc_redist.x64.exe" "${NSISDIR}\..\prereqs\vc_redist.x64.exe"
    IfFileExists "$TEMP\vc_redist.x64.exe" 0 vcredist_ok
      SetDetailsPrint textonly
      DetailPrint "Installing Visual C++ Redistributable..."
      SetDetailsPrint listonly
      nsExec::ExecToLog '"$TEMP\vc_redist.x64.exe" /install /quiet /norestart'
      Pop $0
      Delete "$TEMP\vc_redist.x64.exe"
  vcredist_ok:
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Write breadcrumb so the app knows where the installer was launched from.
  ; $EXEDIR = directory containing the installer .exe (e.g., E:\SmolPC-Lite\)
  CreateDirectory "$LOCALAPPDATA\SmolPC 2.0"
  FileOpen $0 "$LOCALAPPDATA\SmolPC 2.0\installer-source.txt" w
  FileWrite $0 "$EXEDIR"
  FileClose $0
!macroend
