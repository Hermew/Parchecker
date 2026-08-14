<#
.SYNOPSIS
    Revisa el entorno para correr Parchecker y dice como arreglar lo que falte.

.DESCRIPTION
    Las tres piezas del proyecto son independientes y no hace falta tenerlas
    todas. Por eso este script no exige que este todo instalado: exige que lo
    que este empezado este completo.

        [ok]     la pieza esta y sirve
        [falta]  algo quedo a medio instalar  -> sale con 1
        [aviso]  funciona, pero con una limitacion que conviene saber
        [info]   ese camino no esta instalado, y esta bien

    Cada [falta] viene con el comando que lo arregla. El razonamiento detras de
    cada uno esta en INSTALACION.md.

.EXAMPLE
    .\tools\Comprobar.ps1

.NOTES
    Sin acentos a proposito: es lo que se hace en todos los scripts del repo,
    para que la salida se lea igual en cualquier codepage de consola.
#>

[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

$script:fallas = 0

# ---------------------------------------------------------------------------
# Salida
# ---------------------------------------------------------------------------

function Write-Estado {
    param(
        [Parameter(Mandatory)] [ValidateSet("ok", "falta", "aviso", "info")] [string]$Estado,
        [Parameter(Mandatory)] [string]$Texto,
        [string[]]$Arreglo = @()
    )

    $color = switch ($Estado) {
        "ok"    { "Green" }
        "falta" { "Red" }
        "aviso" { "Yellow" }
        "info"  { "DarkGray" }
    }

    Write-Host ("  [{0}] " -f $Estado.PadRight(5)) -ForegroundColor $color -NoNewline
    Write-Host $Texto

    foreach ($linea in $Arreglo) {
        Write-Host "          $linea" -ForegroundColor DarkGray
    }

    if ($Estado -eq "falta") { $script:fallas++ }
}

function Write-Seccion {
    param([string]$Titulo)
    Write-Host ""
    Write-Host $Titulo -ForegroundColor Cyan
}

# El nombre de usuario no aporta nada al diagnostico y viaja en cualquier captura
# o pegado de la salida. Se reemplaza por el mismo placeholder que usan los .md.
#
# Se ancla en la raiz real de los perfiles y se come el segmento siguiente, sea
# cual sea: asi tapa tambien la forma corta 8.3 (NOMBRE~1), que no coincide con
# $env:USERPROFILE. Como exige la raiz, no toca ninguna ruta que no sea de perfil.
function Get-RutaSegura {
    param([string]$Ruta)
    if ([string]::IsNullOrEmpty($Ruta)) { return $Ruta }

    $raiz = Split-Path $env:USERPROFILE -Parent
    return $Ruta -replace ("(?i)^" + [regex]::Escape($raiz) + "\\[^\\]+"), "$raiz\TU_USUARIO"
}

function Get-Programa {
    param([string]$Nombre)
    $cmd = Get-Command $Nombre -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    return $null
}

Write-Host ""
Write-Host "Parchecker - comprobacion del entorno"
Write-Host "====================================="

# ---------------------------------------------------------------------------
# Base: lo que necesita la ventana, que es el corazon del proyecto
# ---------------------------------------------------------------------------

Write-Seccion "Base (la ventana de askpass/)"

$ps = $PSVersionTable.PSVersion
if ($ps.Major -gt 5 -or ($ps.Major -eq 5 -and $ps.Minor -ge 1)) {
    Write-Estado ok "PowerShell $ps"
} else {
    Write-Estado falta "PowerShell $ps - hace falta 5.1 o superior" @(
        "5.1 viene con Windows desde el 7. Si estas abajo de eso, actualiza Windows."
    )
}

# 19041 es la build de Windows 10 2004, desde donde existe
# WDA_EXCLUDEFROMCAPTURE. Sin eso la ventana sale en las capturas.
$build = [System.Environment]::OSVersion.Version.Build
if ($build -ge 19041) {
    Write-Estado ok "Windows build $build - la ventana se excluye de las capturas"
} else {
    Write-Estado aviso "Windows build $build - anterior a la 2004" @(
        "Funciona igual, pero la ventana NO se oculta de las capturas de pantalla.",
        "Detalle: INSTALACION.md, seccion 'La ventana sola'."
    )
}

# ---------------------------------------------------------------------------
# Camino Python
# ---------------------------------------------------------------------------

Write-Seccion "Camino Python (cifrar/)"

$pythonExe = Get-Programa "python"
$pythonVersion = $null
if ($pythonExe) {
    try {
        $salida = & $pythonExe --version
        if ("$salida" -match "Python\s+(\d+)\.(\d+)") {
            $pythonVersion = [version]("{0}.{1}" -f $matches[1], $matches[2])
        }
    } catch {
        $pythonVersion = $null
    }
}

# Mismo orden de busqueda que buscar_rar() en cifrar/cifrar.py, para que el
# script no diga que esta lo que la herramienta no va a encontrar.
$rar = $null
foreach ($candidato in @("$env:ProgramFiles\WinRAR\Rar.exe", "${env:ProgramFiles(x86)}\WinRAR\Rar.exe")) {
    if (Test-Path $candidato) { $rar = $candidato; break }
}
if (-not $rar) { $rar = Get-Programa "rar" }

if (-not $pythonExe -and -not $rar) {
    Write-Estado info "No instalado. Si lo queres: INSTALACION.md, camino 1."
} else {
    if (-not $pythonExe) {
        Write-Estado falta "Python" @(
            "Instalalo:  winget install --id Python.Python.3.12 -e",
            "Si lo acabas de instalar, abri una terminal nueva (INSTALACION.md, regla 1)."
        )
    } elseif (-not $pythonVersion) {
        Write-Estado falta "python.exe no informa version - es el stub de la Microsoft Store" @(
            "Instala Python de verdad:  winget install --id Python.Python.3.12 -e",
            "Detalle:                   INSTALACION.md, regla 6."
        )
    } elseif ($pythonVersion -lt [version]"3.8") {
        Write-Estado falta "Python $pythonVersion - hace falta 3.8 o superior" @(
            "Actualizalo:  winget install --id Python.Python.3.12 -e"
        )
    } else {
        Write-Estado ok "Python $pythonVersion"
    }

    if ($rar) {
        Write-Estado ok ("Rar.exe en " + (Get-RutaSegura (Split-Path $rar)))
    } else {
        Write-Estado falta "Rar.exe" @(
            "Instalalo:  winget install --id RARLab.WinRAR -e",
            "Detalle:    INSTALACION.md, regla 8."
        )
    }
}

# ---------------------------------------------------------------------------
# Camino Rust
# ---------------------------------------------------------------------------

Write-Seccion "Camino Rust (sobre/)"

$cargo = Get-Programa "cargo"
if (-not $cargo) {
    Write-Estado info "No instalado. Si lo queres: INSTALACION.md, camino 2."
} else {
    $cargoVersion = ""
    try {
        $salida = & $cargo --version
        if ("$salida" -match "cargo\s+(\S+)") { $cargoVersion = $matches[1] }
    } catch { }
    Write-Estado ok ("cargo $cargoVersion").Trim()

    # El host triple de rustc dice cual toolchain esta activo. Es mas confiable
    # de parsear que la salida de `rustup show`.
    $triple = $null
    $rustc = Get-Programa "rustc"
    if ($rustc) {
        try {
            foreach ($linea in (& $rustc -vV)) {
                if ($linea -match "^host:\s*(\S+)") { $triple = $matches[1] }
            }
        } catch { }
    }

    if (-not $triple) {
        Write-Estado aviso "No pude determinar el toolchain activo" @(
            "Proba a mano:  rustup show"
        )
    } elseif ($triple -like "*-gnu") {
        Write-Estado ok "toolchain $triple"

        $dlltool = Get-Programa "dlltool"
        if (-not $dlltool) {
            Write-Estado falta "dlltool - el toolchain GNU de rustup no lo trae" @(
                "Instalalo:  winget install --id BrechtSanders.WinLibs.POSIX.UCRT -e --location C:\mingw64",
                "Detalle:    INSTALACION.md, regla 3."
            )
        } elseif ($dlltool -match '\s') {
            Write-Estado falta ("dlltool esta en una ruta con espacios: " + (Get-RutaSegura (Split-Path $dlltool))) @(
                "El enlazador de MinGW parte las rutas en los espacios y el build falla",
                "con un 'cannot find' que muestra la ruta cortada al medio.",
                "Reinstala MinGW en una ruta sin espacios, tipo C:\mingw64.",
                "Detalle:    INSTALACION.md, regla 4."
            )
        } else {
            Write-Estado ok ("dlltool en " + (Get-RutaSegura (Split-Path $dlltool)))
        }
    } else {
        Write-Estado ok "toolchain $triple - no le aplican las reglas 3 ni 4"
    }
}

# ---------------------------------------------------------------------------
# Repo
# ---------------------------------------------------------------------------

if (Get-Programa "git") {
    Write-Seccion "Repo"

    $hooks = ""
    try { $hooks = & git config core.hooksPath } catch { }

    if ("$hooks".Trim() -eq "tools/hooks") {
        Write-Estado ok "hook de pre-commit activo"
    } else {
        Write-Estado aviso "hook de pre-commit sin activar" @(
            "Activalo:  git config core.hooksPath tools/hooks",
            "Sin eso, higiene.py no corre al commitear. Git no clona los hooks.",
            "Detalle:   INSTALACION.md, seccion 'El hook de pre-commit'."
        )
    }
}

# ---------------------------------------------------------------------------

Write-Host ""
if ($script:fallas -eq 0) {
    Write-Host "Todo en orden." -ForegroundColor Green
    exit 0
}

Write-Host ("Hay {0} cosa(s) a medio instalar. El detalle esta en INSTALACION.md." -f $script:fallas) -ForegroundColor Red
exit 1
