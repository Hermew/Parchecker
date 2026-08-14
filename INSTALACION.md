# Instalación

Cómo dejar Parchecker andando en una máquina Windows limpia.

Las tres piezas del proyecto se instalan por separado y ninguna depende de las
otras. Además de los pasos de cada una, el entorno tiene que cumplir ocho
condiciones: están en [§ Reglas del entorno](#reglas-del-entorno), con lo que
pasa si alguna no se cumple.

---

## Qué parte necesitás

No hace falta instalar todo. Las tres piezas son independientes:

| | Qué necesita | Qué te deja |
|---|---|---|
| **La ventana** (`askpass/`) | Nada — PowerShell 5.1 ya viene con Windows | Una ventana que pide un secreto, usable desde `git`, `ssh` o cualquier script |
| **Camino Python** (`cifrar/`) | WinRAR + Python 3.8+ | Archivos `.rar` cifrados, que abre cualquiera con WinRAR |
| **Camino Rust** (`sobre/`) | Toolchain de Rust, una sola vez | Un binario suelto de 0,7 MB, sin depender de WinRAR |

El trade-off entre los dos caminos está razonado en
[`sobre/README.md`](sobre/README.md#lo-que-se-pierde): un `.rar` lo abre
cualquiera, un `.age` necesita `age` o esta herramienta. Si el archivo es para
mandarle a alguien, el `.rar` gana; si es para vos, el `.age`.

> [!TIP]
> `.\tools\Comprobar.ps1` revisa todo esto de una y por cada cosa que falta
> imprime el comando que la resuelve.

---

## La ventana sola

No requiere instalación. PowerShell 5.1 viene con Windows desde el 7.

```powershell
.\askpass\Askpass.ps1 -Titulo "Prueba" -Mensaje "Escribi algo" -Texto
```

Si en vez de abrirse la ventana sale un error de política de ejecución, ver la
[regla 7](#regla-7--los-ps1-sueltos-se-invocan-con--executionpolicy-bypass).

| Windows | Por qué |
|---|---|
| 10 versión 2004 o superior | Para que la ventana se excluya de las capturas de pantalla. Con una versión anterior funciona igual, pero avisa por `stderr` que no puede ocultarse |

---

## Camino 1 — Python + WinRAR

### Instalar

```powershell
winget install --id Python.Python.3.12 -e
winget install --id RARLab.WinRAR -e
```

WinRAR y no 7-Zip por una razón que no es de gusto: 7-Zip **exige** la clave
como argumento (`-pMiClave`), visible para cualquier proceso de la máquina, que
es exactamente lo que este proyecto existe para evitar. Está explicado en el
[README](README.md#el-hallazgo).

### Comprobar

```powershell
$PSVersionTable.PSVersion                          # 5.1 o superior
python --version                                   # 3.8 o superior
Test-Path "$env:ProgramFiles\WinRAR\Rar.exe"       # True
```

Si acabás de instalar algo y alguno de esos comandos dice que no existe, ver la
[regla 1](#regla-1--después-de-instalar-algo-abrí-una-terminal-nueva).

Prueba de fuego, que no abre ninguna ventana:

```powershell
python pruebas\test_roundtrip.py
```

---

## Camino 2 — Rust (`sobre/`)

```powershell
winget install --id Rustlang.Rustup -e
```

Después de instalar, **abrí una terminal nueva** (regla 1) y dejá que
`rustup-init` termine de configurar el toolchain.

### Usá MSVC, que es el default

`rustup` en Windows usa `x86_64-pc-windows-msvc` por defecto. Necesita las Build
Tools de C++ para el enlazador; `rustup-init` detecta si faltan y ofrece
instalarlas, o se bajan sueltas eligiendo el workload **"Desarrollo de escritorio
con C++"**.

Pesa varios GB. A cambio no le aplican las reglas
[3](#regla-3--el-toolchain-gnu-necesita-un-mingw-w64-completo-aparte) ni
[4](#regla-4--mingw-w64-va-en-una-ruta-sin-espacios), que es el motivo de que sea
el default.

<details>
<summary><b>Si necesitás el toolchain GNU</b></summary>

<br>

El GNU baja bastante menos. Necesita dos cosas más, las dos obligatorias:

```powershell
rustup default stable-x86_64-pc-windows-gnu
winget install --id BrechtSanders.WinLibs.POSIX.UCRT -e --location C:\mingw64
```

Dos cosas de esa segunda línea:

1. **El MinGW aparte no es opcional.** Al toolchain de `rustup` le falta
   `dlltool` ([regla 3](#regla-3--el-toolchain-gnu-necesita-un-mingw-w64-completo-aparte)).
2. **La ruta del `--location` no puede tener espacios**
   ([regla 4](#regla-4--mingw-w64-va-en-una-ruta-sin-espacios)). Si el instalador
   ignora el `--location`, descomprimilo a mano en `C:\mingw64`.

El paquete se agrega solo al PATH, pero la terminal que ya estaba abierta no se
entera: abrí una nueva.

</details>

### Comprobar

```powershell
rustup show          # que diga cual toolchain esta activo
cargo --version
```

Y compilar, que es la prueba de verdad:

```powershell
cd sobre
cargo build
```

Tiene que terminar en `Finished` sin una sola advertencia. Con `cargo clippy`
tampoco debería salir ninguna.

---

## Reglas del entorno

Cómo tiene que quedar el entorno. Cada regla dice qué pasa si no se cumple.

### Las dos que valen para cualquier camino

#### Regla 1 — Después de instalar algo, abrí una terminal nueva

El PATH se hereda cuando **arranca** la sesión de la terminal. Un instalador que
modifica el PATH lo cambia para las terminales futuras, no para la que ya está
abierta.

Sin eso, `Get-Command` y `where.exe` informan que falta un programa que ya está
instalado. Para descartarlo, buscarlo en disco en vez de creerle al PATH:

```powershell
Get-ChildItem -Path $env:LOCALAPPDATA, $env:ProgramFiles -Filter loquesea.exe `
  -Recurse -Depth 4 -ErrorAction SilentlyContinue
```

#### Regla 2 — Si el repo está en una carpeta sincronizada, sacá `target/` de ahí

`target/` pesa cientos de megas y se reescribe entero en cada compilación, así
que un cliente de sincronización queda subiendo sin parar. Los artefactos se
redirigen con un `.cargo/config.toml` local:

```toml
[build]
target-dir = "C:/Users/TU_USUARIO/AppData/Local/cargo-target/sobre"
```

Ese archivo **no está versionado** a propósito (`sobre/.gitignore`): la ruta
lleva el nombre de usuario adentro y no le sirve a nadie más.

### Las del camino Rust

#### Regla 3 — El toolchain GNU necesita un MinGW-w64 completo aparte

El componente `rust-mingw` de `rustup` trae las librerías de runtime y
`rust-lld`, no las binutils de GNU. `dlltool` no está, y las dependencias que
generan librerías de importación lo necesitan.

Se resuelve instalando un MinGW-w64 completo con su `bin` en el PATH, o usando
MSVC, donde la regla no aplica.

Sin él, el build corta sobre una dependencia que no figura en el `Cargo.toml`:

```
error: error calling dlltool 'dlltool.exe': program not found
error: could not compile `windows-sys` (lib) due to 1 previous error
```

#### Regla 4 — MinGW-w64 va en una ruta sin espacios

`ld` arma sus rutas de búsqueda de librerías a partir de dónde está instalado el
toolchain, y esas rutas no van entrecomilladas: un espacio las parte en dos. Los
destinos por defecto (`Program Files`, las carpetas de perfil de usuario) casi
siempre tienen alguno, así que la ruta hay que elegirla — `C:\mingw64` sirve.

> [!IMPORTANT]
> **Es la ruta del toolchain, no la del proyecto.** Lo que `rustc` le pasa
> explícitamente al enlazador sí va entrecomillado, así que el repo puede vivir
> en una carpeta con espacios en el nombre. La que no puede tenerlos es la
> instalación de MinGW.

Sin eso, el build corta con la ruta cortada justo donde estaba el espacio:

```
ld.exe: cannot find C:/Program: No such file or directory
ld.exe: cannot find Files/.../default-manifest.o: No such file or directory
```

#### Regla 5 — Elegí MSVC salvo que tengas una razón concreta

Es el host por defecto de `rustup` en Windows, y con él las reglas 3 y 4 no
aplican.

| | GNU | MSVC |
|---|---|---|
| Descarga | chica | varios GB |
| Reglas 3 y 4 | las dos | ninguna |
| Soporte | segundo | host por defecto en Windows |

### Las del camino Python

#### Regla 6 — Python se instala de winget o python.org, no del stub de la Store

Windows trae un `python.exe` cuyo único trabajo es ofrecer la instalación desde
la Microsoft Store. No es un intérprete.

```powershell
winget install --id Python.Python.3.12 -e
```

Si el stub sigue ganando, se desactiva en *Configuración → Aplicaciones → Alias
de ejecución de aplicaciones*. Se lo reconoce porque `python --version` no
imprime ninguna versión: abre la tienda, o no hace nada.

#### Regla 7 — Los `.ps1` sueltos se invocan con `-ExecutionPolicy Bypass`

La política de ejecución por defecto de PowerShell bloquea los scripts sueltos.
Se saltea por corrida, sin tocar la configuración de la máquina:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\askpass\Askpass.ps1 -Texto
```

Solo aplica si corrés `Askpass.ps1` por tu cuenta: `cifrar.py` ya llama a
PowerShell con esa opción. Sin ella, el mensaje es `... no se puede cargar porque
la ejecución de scripts está deshabilitada en este sistema.`

#### Regla 8 — WinRAR va en su carpeta por defecto, o en el PATH

El instalador de WinRAR no toca el PATH, y no hace falta que lo toque:
`cifrar.py` busca primero en las dos carpetas de Program Files, que es donde
WinRAR se instala por defecto. Si está en otro lado, esa carpeta va al PATH.

Sin eso: `No encontre Rar.exe. Instala WinRAR o agrega su carpeta al PATH.`

---

## El hook de pre-commit

`tools/higiene.py` caza caracteres invisibles y está pensado para correr en cada
commit. **Git no clona los hooks**, así que en un clon recién bajado no corre
nada hasta que lo actives:

```powershell
git config core.hooksPath tools/hooks
```

Un solo comando, una sola vez por clon. El hook está versionado en
`tools/hooks/pre-commit`, así que se actualiza con el repo.

Necesita git 2.9 o superior. Para comprobar que quedó activo, metele un carácter
invisible a cualquier archivo y tratá de commitearlo: el commit tiene que
frenarse.

---

## Comprobación final

```powershell
.\tools\Comprobar.ps1                 # el entorno entero, con los arreglos
python pruebas\test_roundtrip.py      # 21 chequeos de cifrado, sin abrir ventana
python tools\higiene.py .             # tiene que salir limpio
```

Y si instalaste el camino Rust:

```powershell
cd sobre
cargo build
```

El roundtrip valida claves ASCII, con acentos y `ñ`, con símbolos que rompen
shells, y de 120 caracteres: que creen, que abran con la clave correcta, que
**no** abran con la equivocada, y que sin la clave no se pueda ni listar el
contenido.

Lo único que queda para probar a mano es la ventana, porque hay que tipear:

```powershell
python cifrar\cifrar.py prueba.rar --vacio --verificar
```
