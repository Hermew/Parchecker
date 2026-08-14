@echo off
REM ---------------------------------------------------------------------------
REM Shim para GIT_ASKPASS y SSH_ASKPASS.
REM
REM Esas dos variables tienen que apuntar a algo ejecutable, y un .ps1 no lo es.
REM Este .cmd es el puente: recibe el mensaje que git o ssh quieren mostrar
REM (llega como primer argumento) y abre la ventanita con ese texto.
REM
REM     set GIT_ASKPASS=C:\ruta\Parchecker\askpass\askpass.cmd
REM     set SSH_ASKPASS=C:\ruta\Parchecker\askpass\askpass.cmd
REM     set SSH_ASKPASS_REQUIRE=force
REM
REM Desde ahi, cualquier cosa que pida una contrasena la pide por la ventana.
REM ---------------------------------------------------------------------------

setlocal

set "MENSAJE=%~1"
if "%MENSAJE%"=="" set "MENSAJE=Escribi el secreto"

powershell.exe -NoProfile -NonInteractive -STA -ExecutionPolicy Bypass ^
  -File "%~dp0Askpass.ps1" -Texto -Titulo "Autenticacion" -Mensaje "%MENSAJE%"

exit /b %ERRORLEVEL%
