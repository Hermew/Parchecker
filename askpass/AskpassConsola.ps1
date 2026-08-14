<#
.SYNOPSIS
    La misma ventana, dibujada con caracteres en la consola.

.DESCRIPTION
    Implementa el MISMO contrato que Askpass.ps1: pide un secreto, lo escribe por
    stdout como bytes UTF-16LE crudos, y sale con 0 (aceptado), 1 (cancelado) o
    2 (error). Es intercambiable con la version grafica sin tocar a quien la llama.

    La diferencia es que no abre ninguna ventana: dibuja un calco de la ventana
    con caracteres de dibujo de cajas, directo en la terminal.

    El dibujo va por STDERR. Es la unica forma de que stdout quede limpio para el
    secreto: si la interfaz se escribiera por stdout, el que pipea el resultado
    recibiria la ventana entera mezclada con la contrasena.

    Nunca se hace eco de lo que se teclea. Se lee tecla por tecla con ReadKey y se
    dibujan puntos. Igual que la version grafica, el secreto se acumula en un
    SecureString y jamas existe como String de .NET.

.PARAMETER Titulo
    Titulo de la ventana dibujada.

.PARAMETER Mensaje
    La linea que se lee arriba del campo.

.PARAMETER Confirmar
    Agrega un segundo campo y exige que coincidan.

.PARAMETER PermitirVacio
    Acepta un secreto vacio.

.PARAMETER Texto
    Devuelve el secreto como linea de texto en vez de bytes UTF-16LE, para
    GIT_ASKPASS y SSH_ASKPASS. Pierde la garantia de borrado de memoria.

.PARAMETER Ascii
    Fuerza el juego de caracteres mas pobre (+-|), por si la terminal es rara.

.PARAMETER Demo
    Dibuja la ventana con datos de ejemplo por stdout y sale. No lee el teclado
    ni pide nada. Sirve para ver como queda sin tener que tipear.

.EXAMPLE
    .\AskpassConsola.ps1 -Titulo "Cifrar backup.rar" -Confirmar

.EXAMPLE
    .\AskpassConsola.ps1 -Demo

.NOTES
    Este archivo tiene BOM a proposito: sin el, PowerShell 5.1 lo lee como ANSI y
    rompe los acentos.
#>

[CmdletBinding()]
param(
    [string]$Titulo = "Contrasena",
    [string]$Mensaje = "Escribi la contrasena",
    [switch]$Confirmar,
    [switch]$PermitirVacio,
    [switch]$Texto,
    [switch]$Ascii,
    [switch]$Demo
)

$ErrorActionPreference = "Stop"

# --------------------------------------------------------------------------
# Que caracteres aguanta esta terminal
# --------------------------------------------------------------------------
# La consola de Windows en espanol arranca en cp850. Ahi los marcos (╔═╗║ ┌─┐│) y
# los bloques (█▓▒░) SI existen, pero las esquinas redondeadas, ● y ⚠ no: salen
# como signos de pregunta. Se intenta pasar a UTF-8; si no se puede, se usa el
# juego que cp850 garantiza.

$rico = $false
if (-not $Ascii) {
    try {
        [Console]::OutputEncoding = [Text.Encoding]::UTF8
        $rico = ([Console]::OutputEncoding.CodePage -eq 65001)
    } catch {
        $rico = $false
    }
}

function Glifos {
    if ($Ascii) {
        return @{
            eSI='+'; eSD='+'; eII='+'; eID='+'; hor='='; ver='|'
            tIzD='+'; tDeD='+'; tIzS='+'; tDeS='+'
            iSI='+'; iSD='+'; iII='+'; iID='+'; iHor='-'; iVer='|'
            punto='*'; lleno='#'; vacio='.'; aviso='!'; sep='-'; medio='-'
        }
    }
    # cp850 tiene marcos y bloques; ● ⚠ y las redondeadas, no.
    # Los tees vienen en dos sabores y no se pueden mezclar: si la linea que cruza
    # es doble (═) va ╠╣, y si es simple (─) va ╟╢. Cruzarlos deja un diente.
    $g = @{
        eSI=[char]0x2554; eSD=[char]0x2557; eII=[char]0x255A; eID=[char]0x255D
        hor=[char]0x2550; ver=[char]0x2551
        tIzD=[char]0x2560; tDeD=[char]0x2563     # ╠ ╣  para la linea doble
        tIzS=[char]0x255F; tDeS=[char]0x2562     # ╟ ╢  para la linea simple
        iSI=[char]0x250C; iSD=[char]0x2510; iII=[char]0x2514; iID=[char]0x2518
        iHor=[char]0x2500; iVer=[char]0x2502
        lleno=[char]0x2588; vacio=[char]0x2591; sep=[char]0x2500
    }
    $g.medio = [char]0x00B7       # · esta en cp850 y en cp1252, sobrevive siempre
    if ($rico) {
        $g.punto = [char]0x25CF   # ●
        $g.aviso = [char]0x26A0   # ⚠
    } else {
        $g.punto = '*'
        $g.aviso = '!'
    }
    return $g
}

$G = Glifos
$ANCHO = 54           # ancho interno, entre los dos bordes verticales
$ANCHO_BARRA = 40     # la barra arranca a plomo con la cajita del campo

# --------------------------------------------------------------------------
# Armado de las lineas
# --------------------------------------------------------------------------

function Recortar([string]$s, [int]$max) {
    if ($null -eq $s) { return "" }
    if ($s.Length -le $max) { return $s }
    return $s.Substring(0, [Math]::Max(0, $max - 1)) + "."
}

# Toda fila mide exactamente ANCHO+2. Cada llamador trae su propia sangria de dos
# espacios, asi el texto suelto queda a plomo con el borde de la cajita.
function Fila([string]$contenido) {
    $c = Recortar $contenido $ANCHO
    return "{0}{1}{2}" -f $G.ver, $c.PadRight($ANCHO), $G.ver
}

function FilaCampo([string]$contenido) {
    # El campo de texto: una cajita simple adentro de la ventana.
    $interno = $ANCHO - 8
    $c = Recortar $contenido $interno
    return Fila ("  {0} {1} {2}" -f $G.iVer, $c.PadRight($interno), $G.iVer)
}

function BordeCampo([string]$izq, [string]$der) {
    return Fila ("  {0}{1}{2}" -f $izq, ([string]$G.iHor * ($ANCHO - 6)), $der)
}

function Separador([string]$izq, [string]$car, [string]$der) {
    return "{0}{1}{2}" -f $izq, ([string]$car * $ANCHO), $der
}

function Barra([int]$bits) {
    $llenas = [Math]::Min($ANCHO_BARRA, [Math]::Round($bits * $ANCHO_BARRA / 90))
    return ([string]$G.lleno * $llenas) + ([string]$G.vacio * ($ANCHO_BARRA - $llenas))
}

function FilaBarra([int]$bits) {
    $etiqueta = if ($bits) { "$bits bits" } else { "" }
    return Fila ("  " + (Barra $bits) + "  " + $etiqueta)
}

function Entropia([string]$s) {
    if ([string]::IsNullOrEmpty($s)) { return 0 }
    $alfabeto = 0
    if ($s -cmatch "[a-z]")        { $alfabeto += 26 }
    if ($s -cmatch "[A-Z]")        { $alfabeto += 26 }
    if ($s -match  "[0-9]")        { $alfabeto += 10 }
    if ($s -match  "[^a-zA-Z0-9]") { $alfabeto += 33 }
    if ($alfabeto -eq 0) { return 0 }
    return [int][Math]::Round($s.Length * [Math]::Log($alfabeto, 2))
}

function TextoInseguro($sec) {
    # Solo para medir fuerza y comparar. Vive milisegundos y se libera enseguida.
    if ($null -eq $sec -or $sec.Length -eq 0) { return "" }
    $ptr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($sec)
    try   { return [Runtime.InteropServices.Marshal]::PtrToStringBSTR($ptr) }
    finally { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($ptr) }
}

# --------------------------------------------------------------------------
# El plano de la ventana
# --------------------------------------------------------------------------
# Se arma una vez, se dibuja una vez, y despues solo se reescriben las filas que
# cambian. Redibujar todo en cada tecla haria parpadear la terminal entera.

$FILA = @{}
function Plano([string]$campo1, [string]$campo2, [int]$bits, [string]$aviso, [int]$activo) {
    $lineas = New-Object System.Collections.Generic.List[string]
    $lineas.Add((Separador $G.eSI $G.hor $G.eSD))
    $lineas.Add((Fila ("  Parchecker  " + $G.medio + "  " + $Titulo)))
    $lineas.Add((Separador $G.tIzD $G.hor $G.tDeD))
    $lineas.Add((Fila ""))
    $lineas.Add((Fila ("  " + $Mensaje)))
    $lineas.Add((Fila ""))

    $lineas.Add((BordeCampo $G.iSI $G.iSD))
    $FILA.campo1 = $lineas.Count
    $lineas.Add((FilaCampo $campo1))
    $lineas.Add((BordeCampo $G.iII $G.iID))

    if ($Confirmar) {
        $lineas.Add((Fila ""))
        $lineas.Add((Fila "  y de nuevo, para estar seguros:"))
        $lineas.Add((BordeCampo $G.iSI $G.iSD))
        $FILA.campo2 = $lineas.Count
        $lineas.Add((FilaCampo $campo2))
        $lineas.Add((BordeCampo $G.iII $G.iID))
    }

    $lineas.Add((Fila ""))
    $FILA.barra = $lineas.Count
    $lineas.Add((FilaBarra $bits))
    $FILA.aviso = $lineas.Count
    $lineas.Add((Fila ("  " + $aviso)))
    $lineas.Add((Fila ""))
    $lineas.Add((Separador $G.tIzS $G.sep $G.tDeS))
    $lineas.Add((Fila "  Enter aceptar   Esc cancelar   F2 ver   ^U limpiar"))
    $lineas.Add((Separador $G.eII $G.hor $G.eID))
    return $lineas
}

# --------------------------------------------------------------------------
# Modo demo: dibujar y salir
# --------------------------------------------------------------------------

if ($Demo) {
    $ejemplo = [string]$G.punto * 14
    foreach ($l in (Plano $ejemplo ($ejemplo) 72 ("" + $G.aviso + " Bloq Mayus activado") 0)) {
        [Console]::Out.WriteLine($l)
    }
    exit 0
}

# --------------------------------------------------------------------------
# Sin consola de verdad no hay dibujo posible
# --------------------------------------------------------------------------

try { $null = [Console]::CursorTop } catch {
    [Console]::Error.WriteLine("Esta version necesita una terminal de verdad. Si estas redirigiendo la entrada, usa Askpass.ps1 (la ventana grafica).")
    exit 2
}
if ([Console]::IsInputRedirected) {
    [Console]::Error.WriteLine("La entrada esta redirigida: no puedo leer el teclado. Usa Askpass.ps1 (la ventana grafica).")
    exit 2
}

# --------------------------------------------------------------------------
# Que la consola tampoco salga en las capturas
# --------------------------------------------------------------------------
# Mismo truco que la version grafica, aplicado a la ventana de la consola. En
# conhost funciona; en Windows Terminal, GetConsoleWindow devuelve una ventana
# oculta que no es la que se ve, asi que puede no tener efecto. Se restaura si o
# si al salir: dejarle la terminal invisible al usuario seria imperdonable.

if (-not ("PC3R.NativoConsola" -as [type])) {
    Add-Type -Namespace PC3R -Name NativoConsola -MemberDefinition @"
[DllImport("user32.dll", SetLastError = true)]
public static extern bool SetWindowDisplayAffinity(IntPtr hWnd, uint dwAffinity);
[DllImport("kernel32.dll")]
public static extern IntPtr GetConsoleWindow();
"@
}
$WDA_NONE = 0x0
$WDA_EXCLUDEFROMCAPTURE = 0x11
$hwndConsola = [PC3R.NativoConsola]::GetConsoleWindow()
$afinidadPuesta = $false

# --------------------------------------------------------------------------
# Bucle
# --------------------------------------------------------------------------

$sec1 = New-Object System.Security.SecureString
$sec2 = New-Object System.Security.SecureString
$activo = 0
$revelar = $false
$cursorAntes = $true
$salida = 1

try {
    if ($hwndConsola -ne [IntPtr]::Zero) {
        $afinidadPuesta = [PC3R.NativoConsola]::SetWindowDisplayAffinity($hwndConsola, $WDA_EXCLUDEFROMCAPTURE)
    }
    try { $cursorAntes = [Console]::CursorVisible; [Console]::CursorVisible = $false } catch {}

    # Dibujo inicial completo. La base se calcula DESPUES de escribir: si la
    # terminal scrolleo para hacer lugar, las coordenadas ya quedan corregidas.
    $lineas = Plano "" "" 0 "" 0
    foreach ($l in $lineas) { [Console]::Error.WriteLine($l) }
    $base = [Console]::CursorTop - $lineas.Count

    function Repintar([int]$indice, [string]$contenido) {
        if ($base -lt 0) { return }
        [Console]::SetCursorPosition(0, $base + $indice)
        [Console]::Error.Write($contenido)
    }

    while ($true) {
        # --- estado actual ---
        $t1 = TextoInseguro $sec1
        $t2 = TextoInseguro $sec2
        $actual = if ($activo -eq 0) { $t1 } else { $t2 }

        $marca1 = if ($revelar -and $activo -eq 0) { $t1 } else { [string]$G.punto * $sec1.Length }
        $marca2 = if ($revelar -and $activo -eq 1) { $t2 } else { [string]$G.punto * $sec2.Length }
        if ($activo -eq 0) { $marca1 += "_" } else { $marca2 += "_" }

        $bits = Entropia $t1
        $avisos = @()
        if ([Console]::CapsLock) { $avisos += ("" + $G.aviso + " Bloq Mayus activado") }
        if ($t1.Length -gt 127) { $avisos += "RAR corta a 127 caracteres" }
        if ($Confirmar -and $sec2.Length -gt 0 -and $t1 -cne $t2) { $avisos += "las dos no coinciden" }

        Repintar $FILA.campo1 (FilaCampo $marca1)
        if ($Confirmar) { Repintar $FILA.campo2 (FilaCampo $marca2) }
        Repintar $FILA.barra (FilaBarra $bits)
        Repintar $FILA.aviso (Fila ("  " + ($avisos -join "   ")))

        $coinciden = (-not $Confirmar) -or ($t1 -ceq $t2)
        $hayAlgo = ($sec1.Length -gt 0) -or $PermitirVacio
        $t1 = $null; $t2 = $null; $actual = $null

        # --- teclado ---
        $tecla = [Console]::ReadKey($true)
        $ctrl = ($tecla.Modifiers -band [ConsoleModifiers]::Control) -ne 0

        if ($tecla.Key -eq [ConsoleKey]::Escape) {
            $salida = 1
            break
        }
        elseif ($tecla.Key -eq [ConsoleKey]::Enter) {
            if ($Confirmar -and $activo -eq 0) { $activo = 1; continue }
            if (-not $hayAlgo) { continue }
            if (-not $coinciden) { continue }
            $salida = 0
            break
        }
        elseif ($tecla.Key -eq [ConsoleKey]::Tab) {
            if ($Confirmar) { $activo = 1 - $activo }
        }
        elseif ($tecla.Key -eq [ConsoleKey]::F2) {
            $revelar = -not $revelar
        }
        elseif ($tecla.Key -eq [ConsoleKey]::Backspace) {
            $s = if ($activo -eq 0) { $sec1 } else { $sec2 }
            if ($s.Length -gt 0) { $s.RemoveAt($s.Length - 1) }
        }
        elseif ($ctrl -and $tecla.Key -eq [ConsoleKey]::U) {
            if ($activo -eq 0) { $sec1.Clear() } else { $sec2.Clear() }
        }
        elseif ($tecla.KeyChar -and -not [char]::IsControl($tecla.KeyChar)) {
            $s = if ($activo -eq 0) { $sec1 } else { $sec2 }
            if ($s.Length -lt 512) { $s.AppendChar($tecla.KeyChar) }
        }
    }

    # Dejar el cursor debajo del dibujo, no en el medio.
    if ($base -ge 0) { [Console]::SetCursorPosition(0, $base + $lineas.Count) }
}
finally {
    try { [Console]::CursorVisible = $cursorAntes } catch {}
    if ($afinidadPuesta -and $hwndConsola -ne [IntPtr]::Zero) {
        [void][PC3R.NativoConsola]::SetWindowDisplayAffinity($hwndConsola, $WDA_NONE)
    }
}

if ($salida -ne 0) {
    [Console]::Error.WriteLine("cancelado")
    $sec1.Dispose(); $sec2.Dispose()
    exit 1
}

if ($Confirmar) {
    $a = TextoInseguro $sec1
    $b = TextoInseguro $sec2
    $iguales = ($a -ceq $b)
    $a = $null; $b = $null
    if (-not $iguales) {
        [Console]::Error.WriteLine("las dos contrasenas no coinciden")
        $sec1.Dispose(); $sec2.Dispose()
        exit 2
    }
}

# --------------------------------------------------------------------------
# Entregar el secreto: identico a la version grafica
# --------------------------------------------------------------------------

if ($Texto) {
    $plano = TextoInseguro $sec1
    [Console]::Out.WriteLine($plano)
    [Console]::Out.Flush()
    $plano = $null
    $sec1.Dispose(); $sec2.Dispose()
    exit 0
}

$bstr = [IntPtr]::Zero
$bytes = $null
try {
    $bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($sec1)
    $largoBytes = [Runtime.InteropServices.Marshal]::ReadInt32($bstr, -4)
    $bytes = New-Object byte[] $largoBytes
    [Runtime.InteropServices.Marshal]::Copy($bstr, $bytes, 0, $largoBytes)

    $stdout = [Console]::OpenStandardOutput()
    $stdout.Write($bytes, 0, $largoBytes)
    $stdout.Flush()
} finally {
    if ($null -ne $bytes) { [Array]::Clear($bytes, 0, $bytes.Length) }
    if ($bstr -ne [IntPtr]::Zero) { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr) }
    $sec1.Dispose(); $sec2.Dispose()
}

exit 0
