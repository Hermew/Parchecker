<#
.SYNOPSIS
    Busca tu contrasena en los lugares donde no deberia estar, sin filtrarla.

.DESCRIPTION
    El unico que puede verificar que una contrasena no se filtro es el que la
    conoce. Y ahi esta la trampa: si la buscas escribiendola en un comando
    -Select-String "MiClave"- acabas de escribirla en el historial de PowerShell
    y en la lista de procesos. El test se filtra a si mismo.

    Este script pide la clave por la misma ventanita del proyecto, la mantiene en
    memoria, y busca. Nunca la imprime, nunca la escribe a disco, nunca la pasa
    como argumento a nada.

    Informa CUANTAS coincidencias hay y EN QUE ARCHIVO. Jamas muestra la linea
    que coincidio: mostrarla seria filtrar lo que vino a buscar.

    Donde busca por defecto:
      - Historial de PowerShell (PSReadLine)
      - Historial y sesiones de Claude Code
      - La carpeta temporal del usuario
      - Cualquier ruta extra que le pases

.PARAMETER Extra
    Rutas adicionales para revisar.

.PARAMETER MaxMB
    Ignora archivos mas grandes que esto. Por defecto 20 MB.

.PARAMETER Canario
    En vez de pedir la clave, busca este texto. Sirve para comprobar que el
    buscador funciona antes de confiar en un resultado negativo.

.EXAMPLE
    .\BuscarFuga.ps1

.EXAMPLE
    .\BuscarFuga.ps1 -Canario "Parchecker"

.NOTES
    Un resultado limpio no prueba que nunca se filtro: prueba que no esta en los
    lugares revisados, ahora. Es una red, no un teorema.
#>

[CmdletBinding()]
param(
    [string[]]$Extra = @(),
    [int]$MaxMB = 20,
    [string]$Canario
)

$ErrorActionPreference = "Stop"

# --------------------------------------------------------------------------
# Conseguir el texto a buscar
# --------------------------------------------------------------------------

if ($Canario) {
    $aguja = $Canario
    Write-Host "Modo canario: buscando un texto conocido para comprobar el buscador." -ForegroundColor Yellow
} else {
    $ventana = Join-Path $PSScriptRoot "..\askpass\Askpass.ps1"
    if (-not (Test-Path $ventana)) { throw "No encuentro $ventana" }

    # Se pide por la ventanita, igual que para cifrar. La clave no pasa por aca
    # como argumento ni queda en el historial.
    $psExe = Join-Path $env:SystemRoot "System32\WindowsPowerShell\v1.0\powershell.exe"
    $tmpErr = [System.IO.Path]::GetTempFileName()
    try {
        # Start-Process une el array con espacios y NO entrecomilla: cualquier
        # argumento con espacios se parte en varios y el hijo no puede enlazar
        # sus parametros. Hay que entrecomillar a mano lo que lleve espacios.
        $argumentos = @(
            "-NoProfile", "-STA", "-ExecutionPolicy", "Bypass",
            "-File", "`"$ventana`"",
            "-Titulo", "`"Buscar fuga`"",
            "-Mensaje", "`"Que contrasena busco (no se va a mostrar)`""
        ) -join " "

        $proc = Start-Process -FilePath $psExe -PassThru -Wait -NoNewWindow `
            -ArgumentList $argumentos `
            -RedirectStandardOutput "$tmpErr.out" -RedirectStandardError $tmpErr

        # 1 es cancelar; cualquier otro codigo es una falla y hay que decirlo,
        # no disfrazarla de cancelacion.
        if ($proc.ExitCode -eq 1) { Write-Host "Cancelado."; exit 1 }
        if ($proc.ExitCode -ne 0) {
            Write-Host "La ventana fallo (codigo $($proc.ExitCode)):" -ForegroundColor Red
            Get-Content $tmpErr -ErrorAction SilentlyContinue | Select-Object -First 5
            exit 2
        }
        $bytes = [System.IO.File]::ReadAllBytes("$tmpErr.out")
        $aguja = [System.Text.Encoding]::Unicode.GetString($bytes)
    } finally {
        foreach ($f in @($tmpErr, "$tmpErr.out")) {
            if (Test-Path $f) {
                # Sobreescribir antes de borrar: el archivo temporal tuvo el secreto.
                try { [System.IO.File]::WriteAllBytes($f, (New-Object byte[] 4096)) } catch {}
                Remove-Item $f -Force -ErrorAction SilentlyContinue
            }
        }
    }
}

if ([string]::IsNullOrEmpty($aguja)) { throw "No llego nada que buscar." }
if ($aguja.Length -lt 4) { throw "Muy corta para buscar sin ahogarse en falsos positivos." }

# --------------------------------------------------------------------------
# Donde buscar
# --------------------------------------------------------------------------

$lugares = @(
    (Join-Path $env:APPDATA "Microsoft\Windows\PowerShell\PSReadLine"),
    (Join-Path $env:USERPROFILE ".claude"),
    (Join-Path $env:USERPROFILE ".bash_history"),
    $env:TEMP
) + $Extra | Where-Object { $_ -and (Test-Path $_) }

Write-Host ""
Write-Host "Revisando $($lugares.Count) lugar(es). Solo se informa el archivo y cuantas veces, nunca la linea."
Write-Host ""

$limite = $MaxMB * 1MB
$revisados = 0
$hallazgos = @()

foreach ($lugar in $lugares) {
    $archivos = if ((Get-Item $lugar).PSIsContainer) {
        Get-ChildItem $lugar -Recurse -File -ErrorAction SilentlyContinue
    } else {
        Get-Item $lugar
    }

    foreach ($a in $archivos) {
        if ($a.Length -gt $limite -or $a.Length -eq 0) { continue }
        $revisados++
        try {
            $contenido = [System.IO.File]::ReadAllText($a.FullName)
        } catch { continue }

        # Comparacion ordinal: la contrasena distingue mayusculas.
        $veces = 0
        $i = $contenido.IndexOf($aguja, [StringComparison]::Ordinal)
        while ($i -ge 0) {
            $veces++
            $i = $contenido.IndexOf($aguja, $i + 1, [StringComparison]::Ordinal)
        }
        if ($veces -gt 0) {
            $hallazgos += [pscustomobject]@{ Archivo = $a.FullName; Veces = $veces }
        }
        $contenido = $null
    }
}

$aguja = $null
[System.GC]::Collect()

# --------------------------------------------------------------------------
# Informe
# --------------------------------------------------------------------------

Write-Host "Archivos revisados: $revisados"
Write-Host ""

if ($hallazgos.Count -eq 0) {
    Write-Host "LIMPIO: no aparece en ninguno de los lugares revisados." -ForegroundColor Green
    Write-Host ""
    Write-Host "Ojo con lo que esto significa: prueba que no esta ahi, ahora."
    Write-Host "No prueba que nunca se filtro. Es una red, no un teorema."
    exit 0
}

Write-Host "APARECE en $($hallazgos.Count) archivo(s):" -ForegroundColor Red
foreach ($h in $hallazgos) {
    Write-Host ("  {0,3} vez/veces  {1}" -f $h.Veces, $h.Archivo)
}
Write-Host ""
Write-Host "No muestro las lineas a proposito: mostrarlas seria filtrar lo que vine a buscar."
Write-Host "Si esa contrasena protege algo real, cambiala."
exit 1
