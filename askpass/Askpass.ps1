<#
.SYNOPSIS
    Pide un secreto en una ventana de Windows y lo escribe por stdout. Nada mas.

.DESCRIPTION
    Sigue el patron SSH_ASKPASS: este script no sabe para que se usa el secreto.
    Solo lo pide, lo entrega y se va. Quien lo llama decide que hacer con el.

    El secreto sale por stdout como bytes UTF-16LE crudos, sin salto de linea al
    final. Todo lo demas (avisos, errores) va por stderr, asi stdout queda limpio
    para el que lo pipea.

    Codigos de salida:
        0  el usuario acepto; el secreto esta en stdout
        1  el usuario cancelo; stdout vacio
        2  error (por ejemplo, las dos veces no coincidieron)

.PARAMETER Titulo
    Titulo de la ventana.

.PARAMETER Mensaje
    La linea que se lee arriba del campo.

.PARAMETER Confirmar
    Agrega un segundo campo y exige que coincidan. Para cuando se esta creando
    algo: si te equivocas al tipear, despues no hay forma de abrir el archivo.

.PARAMETER PermitirVacio
    Acepta un secreto vacio. Por defecto el boton queda deshabilitado.

.PARAMETER Texto
    Devuelve el secreto como una linea de texto comun en vez de bytes UTF-16LE.
    Es lo que esperan GIT_ASKPASS y SSH_ASKPASS. Ojo: en este modo el secreto
    pasa por un String de .NET, que es inmutable y no se puede sobreescribir, asi
    que se pierde la garantia de borrado de memoria. Se usa solo cuando la
    herramienta que llama exige este formato.

.EXAMPLE
    .\Askpass.ps1 -Titulo "Backup" -Mensaje "Contrasena del .rar" -Confirmar

.NOTES
    Este archivo tiene BOM a proposito: sin el, PowerShell 5.1 lee el script como
    ANSI y rompe los acentos.
#>

[CmdletBinding()]
param(
    [string]$Titulo = "Contrasena",
    [string]$Mensaje = "Escribi la contrasena",
    [switch]$Confirmar,
    [switch]$PermitirVacio,
    [switch]$Texto
)

$ErrorActionPreference = "Stop"

Add-Type -AssemblyName PresentationFramework, PresentationCore, WindowsBase

# --------------------------------------------------------------------------
# Que la ventana no salga en capturas de pantalla
# --------------------------------------------------------------------------
# WDA_EXCLUDEFROMCAPTURE (0x11) le dice al compositor de Windows que excluya la
# ventana de cualquier captura: PrintScreen, grabadores, pantalla compartida en
# Meet o Zoom, y tambien lo que ve una IA que mira el escritorio. Para el usuario
# la ventana se ve normal; en la captura queda un rectangulo negro.
# Necesita Windows 10 2004 o mas nuevo; si falla, se sigue igual pero avisando.

if (-not ("PC3R.Nativo" -as [type])) {
    Add-Type -Namespace PC3R -Name Nativo -MemberDefinition @"
[DllImport("user32.dll", SetLastError = true)]
public static extern bool SetWindowDisplayAffinity(IntPtr hWnd, uint dwAffinity);
"@
}
$WDA_EXCLUDEFROMCAPTURE = 0x00000011

# --------------------------------------------------------------------------
# Tema: seguir el del sistema
# --------------------------------------------------------------------------

function Get-TemaOscuro {
    try {
        $clave = "HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Themes\Personalize"
        return (Get-ItemProperty -Path $clave -Name AppsUseLightTheme -ErrorAction Stop).AppsUseLightTheme -eq 0
    } catch {
        return $false
    }
}

if (Get-TemaOscuro) {
    $cFondo = "#FF202020"; $cTexto = "#FFF0F0F0"; $cCampo = "#FF2D2D2D"
    $cBorde = "#FF3F3F46"; $cSuave = "#FF9A9A9A"
} else {
    $cFondo = "#FFF3F3F3"; $cTexto = "#FF1B1B1B"; $cCampo = "#FFFFFFFF"
    $cBorde = "#FFD0D0D0"; $cSuave = "#FF6B6B6B"
}

# --------------------------------------------------------------------------
# La ventana
# --------------------------------------------------------------------------

$xaml = @'
<Window xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
        xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
        Title="TITULO" Width="430" SizeToContent="Height"
        WindowStartupLocation="CenterScreen" ResizeMode="NoResize"
        ShowInTaskbar="False" Topmost="True" Background="CFONDO"
        FontFamily="Segoe UI" FontSize="13">
  <StackPanel Margin="22,18,22,18">
    <TextBlock x:Name="lblMensaje" Text="MENSAJE" Foreground="CTEXTO"
               FontSize="15" TextWrapping="Wrap" Margin="0,0,0,12"/>

    <Grid>
      <PasswordBox x:Name="txtClave" Height="34" Padding="8,0,34,0"
                   VerticalContentAlignment="Center" FontSize="14"
                   Background="CCAMPO" Foreground="CTEXTO" BorderBrush="CBORDE"/>
      <TextBox x:Name="txtVisible" Height="34" Padding="8,0,34,0" Visibility="Collapsed"
               VerticalContentAlignment="Center" FontSize="14"
               Background="CCAMPO" Foreground="CTEXTO" BorderBrush="CBORDE"/>
      <ToggleButton x:Name="btnOjo" Content="&#128065;" Width="28" Height="28"
                    HorizontalAlignment="Right" Margin="0,0,3,0" BorderThickness="0"
                    Background="Transparent" Foreground="CSUAVE" Cursor="Hand"
                    ToolTip="Mostrar la contrasena"/>
    </Grid>

    <Grid x:Name="filaConfirmar" Visibility="Collapsed" Margin="0,8,0,0">
      <PasswordBox x:Name="txtClave2" Height="34" Padding="8,0,8,0"
                   VerticalContentAlignment="Center" FontSize="14"
                   Background="CCAMPO" Foreground="CTEXTO" BorderBrush="CBORDE"/>
    </Grid>

    <ProgressBar x:Name="barFuerza" Height="4" Margin="0,10,0,0" Minimum="0" Maximum="100"
                 Background="CBORDE" BorderThickness="0"/>
    <TextBlock x:Name="lblEstado" Text=" " Foreground="CSUAVE" FontSize="11" Margin="0,6,0,0"/>

    <StackPanel Orientation="Horizontal" HorizontalAlignment="Right" Margin="0,14,0,0">
      <Button x:Name="btnCancelar" Content="Cancelar" Width="96" Height="32" Margin="0,0,8,0"
              IsCancel="True"/>
      <Button x:Name="btnAceptar" Content="Aceptar" Width="96" Height="32"
              IsDefault="True" IsEnabled="False"/>
    </StackPanel>
  </StackPanel>
</Window>
'@

# Los colores y textos se inyectan aca y no con binding: es una ventana de 30
# lineas, no hace falta un ViewModel.
$xaml = $xaml.Replace("CFONDO", $cFondo).Replace("CTEXTO", $cTexto).Replace("CCAMPO", $cCampo)
$xaml = $xaml.Replace("CBORDE", $cBorde).Replace("CSUAVE", $cSuave)
$xaml = $xaml.Replace("TITULO", [System.Security.SecurityElement]::Escape($Titulo))
$xaml = $xaml.Replace("MENSAJE", [System.Security.SecurityElement]::Escape($Mensaje))

$ventana = [Windows.Markup.XamlReader]::Parse($xaml)

$txtClave      = $ventana.FindName("txtClave")
$txtClave2     = $ventana.FindName("txtClave2")
$txtVisible    = $ventana.FindName("txtVisible")
$btnOjo        = $ventana.FindName("btnOjo")
$btnAceptar    = $ventana.FindName("btnAceptar")
$btnCancelar   = $ventana.FindName("btnCancelar")
$filaConfirmar = $ventana.FindName("filaConfirmar")
$barFuerza     = $ventana.FindName("barFuerza")
$lblEstado     = $ventana.FindName("lblEstado")

if ($Confirmar) { $filaConfirmar.Visibility = "Visible" }

# --------------------------------------------------------------------------
# Fuerza y estado
# --------------------------------------------------------------------------

function Get-Entropia([string]$s) {
    # Estimacion clasica: largo por bits de cada caracter segun el alfabeto usado.
    # No detecta que "perro123" es mala, pero distingue 4 caracteres de 20.
    if ([string]::IsNullOrEmpty($s)) { return 0 }
    $alfabeto = 0
    if ($s -cmatch "[a-z]")        { $alfabeto += 26 }
    if ($s -cmatch "[A-Z]")        { $alfabeto += 26 }
    if ($s -match  "[0-9]")        { $alfabeto += 10 }
    if ($s -match  "[^a-zA-Z0-9]") { $alfabeto += 33 }
    if ($alfabeto -eq 0) { return 0 }
    return [math]::Round($s.Length * [math]::Log($alfabeto, 2))
}

function Get-TextoInseguro($sec) {
    # Solo para medir fuerza y comparar. Vive milisegundos y se libera enseguida.
    if ($null -eq $sec -or $sec.Length -eq 0) { return "" }
    $ptr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($sec)
    try   { return [Runtime.InteropServices.Marshal]::PtrToStringBSTR($ptr) }
    finally { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($ptr) }
}

function Update-Estado {
    if ($txtVisible.Visibility -eq "Visible") {
        $texto = $txtVisible.Text
    } else {
        $texto = Get-TextoInseguro $txtClave.SecurePassword
    }
    $largo = $texto.Length

    $bits = Get-Entropia $texto
    $barFuerza.Value = [math]::Min(100, $bits * 100 / 90)

    $avisos = @()
    if ([System.Windows.Input.Keyboard]::IsKeyToggled([System.Windows.Input.Key]::CapsLock)) {
        $avisos += "Bloq Mayus activado"
    }
    if ($largo -gt 0)   { $avisos += "$bits bits" }
    if ($largo -gt 127) { $avisos += "RAR corta a 127 caracteres" }

    $coinciden = $true
    if ($Confirmar) {
        $texto2 = Get-TextoInseguro $txtClave2.SecurePassword
        $coinciden = ($texto -ceq $texto2)
        if ($txtClave2.SecurePassword.Length -gt 0 -and -not $coinciden) {
            $avisos += "las dos no coinciden"
        }
        $texto2 = $null
    }

    if ($avisos.Count -gt 0) { $lblEstado.Text = ($avisos -join "   -   ") }
    else                     { $lblEstado.Text = " " }

    $hayAlgo = ($largo -gt 0) -or $PermitirVacio
    $btnAceptar.IsEnabled = $hayAlgo -and $coinciden
    $texto = $null
}

$txtClave.Add_PasswordChanged({ Update-Estado })
$txtClave2.Add_PasswordChanged({ Update-Estado })
$txtVisible.Add_TextChanged({ Update-Estado })

# --------------------------------------------------------------------------
# El ojito
# --------------------------------------------------------------------------
# Revelar obliga a tener el texto plano en un TextBox: es el unico momento en que
# la clave existe como string comun. Es consciente, y lo pide el usuario al tocar
# el boton.

$btnOjo.Add_Checked({
    $txtVisible.Text = Get-TextoInseguro $txtClave.SecurePassword
    $txtClave.Visibility = "Collapsed"
    $txtVisible.Visibility = "Visible"
    $txtVisible.Focus() | Out-Null
    $txtVisible.CaretIndex = $txtVisible.Text.Length
})

$btnOjo.Add_Unchecked({
    $txtClave.Password = $txtVisible.Text
    $txtVisible.Clear()
    $txtVisible.Visibility = "Collapsed"
    $txtClave.Visibility = "Visible"
    $txtClave.Focus() | Out-Null
})

$btnCancelar.Add_Click({ $ventana.DialogResult = $false })
$btnAceptar.Add_Click({ $ventana.DialogResult = $true })

$ventana.Add_SourceInitialized({
    $hwnd = (New-Object System.Windows.Interop.WindowInteropHelper($ventana)).Handle
    if (-not [PC3R.Nativo]::SetWindowDisplayAffinity($hwnd, $WDA_EXCLUDEFROMCAPTURE)) {
        [Console]::Error.WriteLine("aviso: este Windows no permite ocultar la ventana de las capturas")
    }
})

$ventana.Add_ContentRendered({
    $ventana.Activate() | Out-Null
    $txtClave.Focus() | Out-Null
    Update-Estado
})

$aceptado = $ventana.ShowDialog()

if ($aceptado -ne $true) {
    [Console]::Error.WriteLine("cancelado")
    exit 1
}

# --------------------------------------------------------------------------
# Entregar el secreto
# --------------------------------------------------------------------------
# De SecureString a bytes sin pasar por un String de .NET, que seria inmutable y
# quedaria en el heap hasta que el recolector tenga ganas. El BSTR se libera con
# ZeroFreeBSTR (sobreescribe con ceros) y el array de bytes se limpia a mano.

if ($btnOjo.IsChecked) { $txtClave.Password = $txtVisible.Text; $txtVisible.Clear() }

$segura = $txtClave.SecurePassword

if ($Confirmar) {
    $a = Get-TextoInseguro $segura
    $b = Get-TextoInseguro $txtClave2.SecurePassword
    $iguales = ($a -ceq $b)
    $a = $null; $b = $null
    if (-not $iguales) {
        [Console]::Error.WriteLine("las dos contrasenas no coinciden")
        exit 2
    }
}

if ($Texto) {
    # git y ssh leen la primera linea de stdout como texto plano. Aca el secreto
    # existe como String de .NET y no hay forma de borrarlo a mano; el precio de
    # hablar el protocolo que esas herramientas entienden.
    $plano = Get-TextoInseguro $segura
    [Console]::Out.WriteLine($plano)
    [Console]::Out.Flush()
    $plano = $null
    $segura.Dispose()
    exit 0
}

$bstr = [IntPtr]::Zero
$bytes = $null
try {
    $bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($segura)
    # Los 4 bytes previos al puntero guardan el largo en bytes del BSTR.
    $largoBytes = [Runtime.InteropServices.Marshal]::ReadInt32($bstr, -4)
    $bytes = New-Object byte[] $largoBytes
    [Runtime.InteropServices.Marshal]::Copy($bstr, $bytes, 0, $largoBytes)

    $stdout = [Console]::OpenStandardOutput()
    $stdout.Write($bytes, 0, $largoBytes)
    $stdout.Flush()
} finally {
    if ($null -ne $bytes) { [Array]::Clear($bytes, 0, $bytes.Length) }
    if ($bstr -ne [IntPtr]::Zero) { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr) }
    $segura.Dispose()
}

exit 0
