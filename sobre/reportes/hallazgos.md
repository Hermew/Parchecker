# Hallazgos

Cinco cosas que no se ven leyendo el código propio: cuatro salieron de leer el
crate `age` o el crate `zeroize`, y una de medir. Cada una está anotada con qué
la comprueba, porque un hallazgo sin forma de verificarlo es una opinión.

---

## 1. La fuga por realocación

**Dónde:** el camino `--utf16le`, que es el que usa `Askpass.ps1`.
**Qué comprueba que está cerrado:** que todos los buffers se reserven del tamaño
final antes de llenarse.

`Zeroizing` no protege un dato: protege **una dirección de memoria**. Cuando el
valor sale de alcance, escribe ceros sobre el bloque que tiene apuntado. El
bloque que tiene apuntado *en ese momento*.

Un `String` que crece no crece en el lugar. El allocator pide un bloque más
grande, **copia** los bytes, y libera el viejo tal cual estaba. Ese bloque
liberado sigue teniendo la clave escrita, y `Zeroizing` ya no lo alcanza porque
ahora apunta al nuevo. El propio `zeroize` lo documenta como su límite, en el
impl de `Vec`:

> *"cannot ensure that previous reallocations did not leave values on the heap"*

Había dos lugares que realocaban:

**`String::from_utf16`.** Mirando su implementación:

```rust
let mut ret = String::with_capacity(v.len());   // v.len() son unidades UTF-16
for c in char::decode_utf16(v) { ret.push(c); } // push escribe bytes UTF-8
```

Reserva **una unidad UTF-16 por byte UTF-8**. Esa cuenta cierra únicamente en
ASCII. Una `ñ` es una unidad y dos bytes; un acento, igual. Con cualquier clave
en castellano la reserva queda corta, el `String` crece, y cada crecimiento
abandona una copia.

Lo que más llama la atención es cuál era el camino afectado: `--utf16le` es el de
`Askpass.ps1`, el que el README declara como el que más importa. Con
`password123` no pasaba nada; con `contraseña123`, sí.

**`SecretString::from(String)`.** Se ve inofensivo, pero `SecretString` es
`SecretBox<str>`, así que ese `From` hace `into_boxed_str()`, que llama a
`shrink_to_fit()`. Si sobra capacidad, encoger también realoca. Y sobraba
siempre, porque los `pop()` que sacan el `\n` del pipe bajan el largo y dejan la
capacidad donde estaba.

**Cómo quedó:** el decodificado reserva el peor caso —tres bytes UTF-8 por unidad
UTF-16— y empuja carácter por carácter, así que no crece nunca. El valor final se
arma con capacidad igual al largo, así que `shrink_to_fit` no tiene nada que
encoger.

De paso, `char::decode_utf16` tenía el mismo problema que el código ya evitaba en
el camino UTF-8: su error se queda con la unidad suelta adentro, o sea con un
pedazo de la clave. Se cambia por un texto fijo antes de que pueda propagarse,
igual que se usaba `str::from_utf8` en vez de `String::from_utf8`.

---

## 2. `NoMatchingKeys` no es "clave incorrecta"

**Dónde:** la clasificación de errores que decide los códigos de salida.
**Qué lo comprueba:** `clave_incorrecta_se_distingue_de_sobre_invalido`.

El nombre de la variante induce al error, y clasificar por intuición habría hecho
que el código `2` no se disparara nunca.

Una clave incorrecta produce **`DecryptError::DecryptionFailed`**. El camino es
este: la identidad scrypt deriva una clave, intenta desenvolver la clave del
archivo con ChaCha20-Poly1305, el AEAD no cierra, y `age` mapea ese fallo con

```rust
impl From<chacha20poly1305::aead::Error> for DecryptError {
    fn from(_: chacha20poly1305::aead::Error) -> Self {
        DecryptError::DecryptionFailed
    }
}
```

**`NoMatchingKeys` significa otra cosa**: que ninguna identidad reconoció el
stanza. Se produce en `protocol.rs`, cuando `find_map` sobre las identidades no
devuelve nada. Con una sola identidad en juego, eso quiere decir que el sobre no
está cerrado de la forma con la que se lo está tratando de abrir — una passphrase
contra un sobre x25519, o al revés. Es un sobre age perfectamente válido.

Por eso `DecryptionFailed` es código `2` y `NoMatchingKeys` es código `3`.

---

## 3. El grease hace variar el tamaño, pero sólo con destinatarios

**Dónde:** apareció como un test de acolchado que fallaba.
**Qué lo comprueba:** el chequeo de punta a punta que compara dos contenidos
distintos acolchados al mismo bloque.

El test asertaba que dos contenidos acolchados a 4 KB dieran sobres del mismo
tamaño. Daban 46 bytes de diferencia. La causa estaba en la cabecera:

```
-> R$1J/g(~-grease VU0, EY% @"q#`HY
```

`age` inyecta una stanza **grease** de largo aleatorio: un señuelo que obliga a
toda implementación a ignorar correctamente las stanzas que no conoce. Es la
misma técnica anti-osificación de TLS.

Medido, un archivo de 100 bytes cerrado ocho veces con `--bloque 4K`:

| cerrado con | resultado |
|---|---|
| passphrase | 4 278 bytes las ocho veces |
| `--para` | siete tamaños distintos, entre 4 332 y 4 402 |

El señuelo aparece cuando hay destinatarios. Un sobre de scrypt tiene que tener
exactamente un destinatario, así que ahí no se agrega y el acolchado cuantiza de
forma exacta.

Y donde el señuelo está, no filtra nada: cerrando **el mismo archivo** doce veces
salieron doce tamaños distintos entre 4 328 y 4 415, y contenidos de 10 a 4 000
bytes acolchados cayeron todos dentro de esa misma banda. La dispersión es ruido
que no correlaciona con el contenido.

**Lo que cambió por esto:** el test dejó de comparar bytes exactos y pasó a
verificar la propiedad que importa — que el tamaño del sobre deje de seguir al
tamaño del contenido.

---

## 4. La calibración de scrypt es barata; el costo es la derivación

**Dónde:** corrige un dato que el README de la herramienta afirmaba.
**Qué lo comprueba:** las mediciones de `--trabajo` a distintos factores.

El README decía que la calibración corría scrypt varias veces y se llevaba 1,88 s
de los 1,89 s totales. Es falso, y se cae con una sola medición: `--trabajo 10`
termina en 78 ms, cosa imposible si la calibración costara 1,88 s.

El código de `age` extrapola en vez de volver a medir:

```rust
duration.map(|mut d| {
    // Use duration as a proxy for CPU usage, which scales linearly with N.
    while d < ONE_SECOND && log_n < 63 {
        log_n += 1;
        d *= 2;          // multiplica, no vuelve a correr scrypt
    }
    log_n
})
```

Corre scrypt **una sola vez** en un factor bajo y de ahí en adelante duplica en
aritmética. La calibración vale milisegundos. El par de segundos es la derivación
real, en el momento de envolver la clave del archivo.

Medido, sellar 1 KB, incluyendo el arranque del proceso (~41 ms):

| | |
|---|--:|
| factor medido por `age` | ~2 500 ms |
| `--trabajo 20` | ~2 523 ms |
| `--trabajo 16` | ~208 ms |
| `--trabajo 10` | ~78 ms |
| `--para` (x25519) | indistinguible del arranque |

Descontando el arranque, factor 20 contra factor 16 da 16,2×, contra un 16
predicho por `2^(20-16)`. La escala se cumple.

**Consecuencia práctica:** `--trabajo` es un dial de velocidad y de seguridad al
mismo tiempo. Bajarlo no es trabajo ahorrado; es fuerza bruta abaratada en el
mismo factor.

---

## 5. `age` rechaza los bytes de más

**Dónde:** decidió dónde va el acolchado.
**Qué lo comprueba:** una prueba directa, pegándole basura al final de un sobre.

La opción más cómoda para acolchar habría sido pegar relleno **después** del
sobre ya cerrado. Eso dejaría el formato intacto para cualquier implementación de
age y no requeriría inventar nada.

No funciona:

```
$ cat r.sobre > rpad.sobre
$ head -c 5000 /dev/urandom >> rpad.sobre
$ printf 'k' | sobre abrir rpad.sobre out.txt
sobre: rpad.sobre no es un sobre que se pueda abrir asi
  causa: decryption error
```

`age` no ignora lo que sobra: lo rechaza. Es buena propiedad —así detecta
truncamientos y agregados— y obliga a que el acolchado vaya adentro del texto en
claro, antes de cifrar.

Eso trae un costo que conviene tener anotado: un sobre acolchado lo abre
cualquier implementación de age, pero lo que devuelve es el contenido con la cola
pegada. Sólo `sobre desrellenar` (o `abrir --desrellenar`) la saca.

---

## Lo que queda sin cerrar

**El tamaño aproximado sigue visible.** `--bloque` mete todo lo que entra en un
bloque en la misma bolsa, pero no confunde un sobre de 4 KB con uno de 40 MB.

**Una copia de memoria en vivo encuentra la clave.** Todo lo del punto 1 achica
la ventana; no la cierra. Mientras la clave se usa está en memoria, y el buffer
del pipe del kernel también la tuvo.

**Un archivo de identidad es un archivo.** Queda entero afuera del trabajo de
barrido de buffers: está en el disco, en claro, y quien lo copia abre todo.

**`--trabajo` fija el costo de derivación, no el tiempo total.** El tiempo sigue
dependiendo de la máquina; lo que queda declarado es cuánto trabajo se hace, no
cuánto tarda.
