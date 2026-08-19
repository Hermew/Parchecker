# sobre

La idea de [Parchecker](..) sin depender de WinRAR.

Un binario de 0,7 MB. Mete un archivo en un sobre cerrado. La clave entra por
stdin y por ningún otro lado.

```
sobre cifrar notas.txt notas.sobre
sobre abrir  notas.sobre notas.txt
```

## Por qué existe

La versión original tuvo que descubrir que `rar.exe -p` acepta la clave por
stdin, y después medir cinco codificaciones para dar con la que espera. Todo ese
trabajo fue **negociar con un programa ajeno** que no fue pensado para esto.

Acá no hay con quién negociar: la interfaz la definimos nosotros. La clave entra
por stdin, en UTF-8, y listo.

El hack no se mejora — **deja de ser necesario**.

## Se enchufa con la ventanita sin tocar nada

```powershell
powershell -File ..\Parchecker\askpass\Askpass.ps1 -Confirmar |
    sobre cifrar --utf16le notas.txt notas.sobre
```

`--utf16le` es porque `Askpass.ps1` escribe UTF-16LE crudo, que es su contrato
desde el primer día. **No hubo que modificar el askpass para esto.** Ese era el
punto de separarlo.

## La criptografía no es mía

El formato es [age](https://age-encryption.org/v1): ChaCha20-Poly1305 con scrypt
para derivar la clave. Está especificado, auditado, y lo implementa el crate
`age`. Este programa mueve bytes y se ocupa de que la clave no toque la línea de
comandos. Nada más.

Los archivos son `.age` estándar: los abre cualquier implementación de age, no
solo esta.

**Ventaja real sobre la versión en Python:** acá la clave vive en un
`SecretString`, que se sobreescribe con ceros cuando sale de alcance. En Python un
`str` es inmutable y queda en el heap hasta que el recolector se digne — por eso
en el proyecto original la parte sensible tenía que vivir en PowerShell.

## El borrado va por todo el camino, no solo por el final

`SecretString` cubre el lugar donde la clave descansa. Pero antes de llegar ahí
pasa por los buffers en los que se la lee y se la convierte, y un `Vec` o un
`String` común se liberan **sin borrarse**: el contenido queda en el heap hasta
que otra cosa lo pise.

Por eso cada paso intermedio va envuelto en `Zeroizing`, que hace lo mismo que
`SecretString` para un buffer cualquiera:

| Paso | Qué lo cubre |
|---|---|
| Los bytes crudos que llegan de stdin | `Zeroizing<Vec<u8>>` |
| Las unidades de 16 bits, solo en `--utf16le` | `Zeroizing<Vec<u16>>` |
| La cadena final | `SecretString` |

El camino `--utf16le` es el que más importa, porque es el que usa `Askpass.ps1` y
el que hace dos copias más que el otro: una para pasar de bytes a `u16` y otra
para pasar de `u16` a texto.

En el camino UTF-8 la validación es `str::from_utf8`, que mira sin consumir. La
alternativa —`String::from_utf8`— ahorra una copia, pero cuando falla devuelve un
error **que se queda con los bytes adentro**: la clave terminaría viajando dentro
del mensaje de error, que es a donde va a parar todo lo que se imprime.

No hace falta declarar `zeroize` aparte: `age` ya lo trae por debajo de `secrecy`
y se usa como `age::secrecy::zeroize::Zeroizing`.

> [!NOTE]
> Esto no mueve el modelo de amenaza. Un volcado de memoria tomado **mientras** la
> clave está en uso la encuentra igual, y el buffer del pipe del kernel también la
> tuvo. Lo que se acorta es la ventana; no se cierra.
>
> La lección sirve para cualquier programa que toque un secreto: **el crate da la
> herramienta, no la garantía.** Hay que seguir el dato por todos los lugares por
> donde pasa.

## No hay `-p`

```
$ sobre cifrar -pMiClave entrada salida
sobre: la clave no se pasa por argumento, nunca. Mandala por stdin:
       los argumentos de un proceso los puede leer cualquier otro proceso.
```

Está rechazado explícitamente. Que sea imposible hacer lo incorrecto es mejor que
documentar que no se debe.

## Lo que se pierde

Un `.rar` lo abre cualquiera que tenga WinRAR, que en Argentina es media
población. Un `.age` no lo abre nadie que no tenga `age` o esta herramienta.

Si el archivo es para vos, esto es mejor. Si es para mandarle a un cliente, el
`.rar` del proyecto original sigue ganando.

## Velocidad: Rust no gana donde uno cree

Medido en esta máquina, mínimo de 5 corridas, archivo de 64 MB de datos
aleatorios para que la compresión no pueda hacer trampa.

**Arranque pelado** — lanzar el proceso y salir:

| | |
|---|--:|
| `sobre.exe` (Rust) | **6,9 ms** |
| `python -c pass` | 67,4 ms |
| `python` + `import cryptography` | 91,1 ms |
| `powershell -NoProfile` | 136,3 ms |

Diez a veinte veces más rápido. **Este es el único lugar donde Rust gana de
verdad**, y para una herramienta que se invoca una vez por archivo, importa.

**Rendimiento de cifrado** — MB por segundo, descontando el costo fijo:

| | |
|---|--:|
| `sobre` (Rust, ChaCha20-Poly1305) | 527 MB/s |
| Análogo en Python (`cryptography`) | 556 MB/s |
| `rar -m0` (AES, sin comprimir) | 584 MB/s |

**Son el mismo número.** Python incluso sale un pelo adelante, dentro del ruido.

El motivo es que en criptografía **nadie escribe el bucle caliente en su
lenguaje**: Python llama a OpenSSL, `age` usa crates con SIMD e intrínsecos, RAR
usa AES-NI del procesador. Los tres terminan en código nativo optimizado a mano.
Elegir el lenguaje no cambia la velocidad del cifrado; cambia el arranque, la
distribución y el manejo de memoria.

> [!WARNING]
> **El hallazgo incómodo: cifrar 1 KB tarda 1,9 segundos.**
>
> No es lento el cifrado — es el KDF, **a propósito**. `age` no usa un costo
> fijo: **mide la máquina en cada ejecución** y calibra scrypt para tardar cerca
> de un segundo. En una máquina más rápida sube el trabajo, así que atacar por
> fuerza bruta cuesta lo mismo en cualquier hardware.
>
> El costo real es peor que ese segundo, porque la calibración **también corre
> scrypt varias veces para medir**. Total: 1,88 s de los 1,89 s que tarda cifrar
> 1 KB. El archivo no pesa nada; el candado sí.
>
> Comparación: `rar` tarda 99 ms para lo mismo. `rar` es **19 veces más rápido y
> eso es peor**, no mejor: significa que su derivación de clave es mucho más
> barata de atacar.

**Cuándo esto importa:** para cifrar un archivo a mano, dos segundos no se
sienten. Para cifrar 500 archivos en un bucle son **16 minutos de puro KDF**, y
ahí conviene meter todo en un solo sobre, o usar una identidad `x25519` en vez de
passphrase — que es exactamente lo que recomienda la documentación de `age` para
uso programático.

## Compilar

```
cargo build --release
```

Necesita un enlazador. **Usá el toolchain MSVC**, que es el que `rustup` elige
por defecto en Windows: pide las Build Tools de C++ —`rustup-init` ofrece
instalarlas solo— y a cambio no aparece ninguna sorpresa.

> [!WARNING]
> El toolchain GNU **no es un reemplazo equivalente**, aunque baje mucho menos.
> Pide dos cosas más, las dos obligatorias: un MinGW-w64 completo aparte —al de
> `rustup` le falta `dlltool`— instalado en una ruta sin espacios. El
> procedimiento está en
> [INSTALACION.md](../INSTALACION.md#camino-2--rust-sobre).

Si el repo te queda dentro de una carpeta que sincroniza a la nube, conviene
sacar los artefactos de ahí: `target/` pesa cientos de megas y cambia en cada
compilación. Se hace con un `.cargo/config.toml` local, que **no está
versionado** justamente porque la ruta lleva tu nombre de usuario adentro:

```toml
[build]
target-dir = "C:/Users/TU_USUARIO/AppData/Local/cargo-target/sobre"
```

## Probado

| | |
|---|---|
| Roundtrip con acentos y `ñ` | vuelve idéntico |
| Clave equivocada | rechazada |
| Clave por argumento | rechazada antes de leer nada |
| Salida existente | no se pisa sin `--forzar` |
| UTF-16LE de `Askpass.ps1` | roundtrip completo |
| UTF-8 leído como UTF-16LE | falla, como debe |
| Clave vacía por stdin | rechazada |
| Formato | el archivo empieza con la cabecera de `age` |
| Un sobre cerrado por UTF-8 abierto por UTF-16LE, y al revés | abre en los dos sentidos |
| Lo mismo con una clave de 480 caracteres | abre en los dos sentidos |

Los dos últimos son la prueba de que envolver los buffers intermedios en
`Zeroizing` no cambió el valor que llega al cifrado: si los dos caminos de
decodificación no derivaran exactamente la misma cadena, el sobre no abriría.
