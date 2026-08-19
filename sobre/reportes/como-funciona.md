# Cómo funciona, pieza por pieza

Cada sección arranca con una analogía y sigue con el mecanismo. La analogía sirve
para agarrar la forma; el mecanismo es lo que hay que saber cuando algo falla.

---

## 1. Por qué la clave entra por stdin

> **El pizarrón del hall.** Escribir el PIN en el pizarrón de la sala de espera
> del banco no es lo mismo que decírselo al cajero en la ventanilla. En los dos
> casos "se lo diste al banco". En uno lo leyó todo el que pasó.

Los argumentos de un proceso son públicos. En Windows cualquier proceso los ve
con `Get-Process`; en Linux están en `/proc/<pid>/cmdline`. Además el shell los
guarda en su historial, en texto plano, en el disco.

`sobre` no acepta la clave por argumento y además **rechaza el intento** en vez
de ignorarlo:

```
$ sobre cifrar -pMiClave entrada salida
sobre: la clave no se pasa por argumento, nunca.
```

La diferencia entre no aceptarlo y rechazarlo es que en el segundo caso el que se
equivocó se entera. Un flag que se ignora en silencio deja a alguien creyendo que
cifró con una clave que nunca llegó.

**Dónde miente la analogía:** el cajero puede acordarse de tu PIN. Los buffers
por donde pasa la clave también, y de eso trata la sección 3.

---

## 2. Los dos modos de cerrar: candado de combinación y buzón

> **El candado de combinación** no requiere que lleves nada encima: la
> combinación la tenés en la cabeza. Pero abrirlo pide girar el dial, y girar
> lleva tiempo.
>
> **El buzón con ranura** es al revés. Cualquiera puede meter una carta por la
> ranura —para eso no hace falta permiso— pero sacarla requiere la llave del
> buzón, que es una cosa física que hay que guardar en algún lado. Meter es
> instantáneo.

| | passphrase | `--para` (x25519) |
|---|---|---|
| Qué tenés que guardar | nada | un archivo de identidad |
| Qué cuesta cerrar | ~2,5 s | nada medible |
| Qué cuesta abrir | ~2,5 s | nada medible |
| Quién puede cerrar | el que sabe la clave | cualquiera con la clave pública |

Esa última fila es la parte que más se pasa por alto. Con `--para`, **cerrar no
requiere ningún secreto**. Un servidor de producción puede cifrar registros a tu
nombre sin tener con qué leerlos. Si alguien entra a ese servidor, se lleva la
capacidad de escribirte, no la de leerte.

```bash
sobre generar mi.identidad        # la mitad pública sale por stderr
sobre cifrar --para age1... notas.txt notas.sobre
sobre abrir --identidad mi.identidad notas.sobre notas.txt
```

Un destinatario `age1...` es público, así que va como argumento sin problema. La
prohibición de la sección 1 es sobre **secretos**, no sobre argumentos.

**Dónde miente la analogía:** el buzón real no le da al cartero forma de saber si
la carta llegó entera. Acá sí — cada bloque va con su sello de autenticación, y
si alguien lo toca, no abre.

---

## 3. Por qué la clave se borra de tres buffers y no de uno

> **El cuaderno y el tacho.** Anotás la clave en un cuaderno, la copiás a otro
> para pasarla en limpio, y de ahí a la planilla final. Tachar sólo la planilla
> no sirve: los dos cuadernos siguen en el cajón.
>
> Y hay un cuaderno que ni siquiera sabés que existe. Cuando el cuaderno se
> queda sin hojas, la secretaria copia todo a uno más grande y **tira el viejo
> tal cual, sin tachar nada**. Vos podés tachar el que tenés en la mano. El que
> ya se fue al tacho, no.

En Rust un `Vec` o un `String` se liberan sin sobreescribirse: el contenido queda
en el heap hasta que otra cosa lo pise. `Zeroizing` lo tapa con ceros al salir de
alcance. La clave pasa por tres formas antes de llegar a destino, y las tres van
envueltas:

| Etapa | Envoltorio |
|---|---|
| Bytes crudos de stdin | `Zeroizing<Vec<u8>>` |
| Unidades de 16 bits (sólo `--utf16le`) | `Zeroizing<Vec<u16>>` |
| Texto decodificado | `Zeroizing<String>` |
| Valor final | `SecretString` |

El cuaderno que se va al tacho es la **realocación**, y es lo que hace la
diferencia entre "usamos la biblioteca" y "seguimos el dato". Cuando un `String`
crece, el allocator pide un bloque nuevo, copia y libera el viejo intacto. Ese
bloque tiene la clave y ningún `Zeroizing` lo alcanza, porque `Zeroizing` borra
el buffer que conoce, no el que el allocator ya se llevó. El crate `zeroize` lo
dice de su propio `Vec`:

> *"cannot ensure that previous reallocations did not leave values on the heap"*

Por eso todos los buffers se reservan del tamaño final antes de llenarlos. El
detalle completo, con los dos lugares donde pasaba, está en
[hallazgos.md](hallazgos.md#1-la-fuga-por-realocación).

**Dónde miente la analogía:** los cuadernos del tacho son recuperables si alguien
revuelve. En el heap sólo son recuperables si alguien puede leer la memoria del
proceso, que ya es un ataque bastante más caro. Esto achica la ventana; no la
cierra.

---

## 4. El marco: cómo se pasan dos cosas por un caño

> **El dictado por teléfono.** Le tenés que pasar a alguien un nombre y una
> dirección, seguidos.
>
> — *"Te digo el nombre y cuando diga PUNTO empieza la dirección."*
>
> Funciona hasta que el nombre es **Punto Rodríguez**. Ahí el otro corta en el
> lugar equivocado, anota "Punto" como nombre y "Rodríguez" como principio de la
> dirección, y **nadie se entera**, porque "Punto" también es un nombre válido.
>
> — *"El nombre son quince letras."*
>
> Contra eso no hay forma de equivocarse.

Cuando la clave y el contenido viajan por el mismo stdin hay que decir dónde
termina una y empieza el otro. `sobre` usa el largo, no un separador:

```
13 \n clave-secreta <contenido>
▲     ▲             ▲
│     └─ exactamente 13 bytes, sean los que sean
│
└─ ASCII decimal: nunca choca con la clave, sea el idioma que sea
```

El "Punto Rodríguez" de este caso es UTF-16LE. Ahí cada carácter son dos bytes, y
un carácter cualquiera puede llevar `0x0A` como byte bajo — que es exactamente el
byte del salto de línea. Un separador cortaría la clave al medio, y la clave
cortada **también es una clave válida, sólo que otra**. El error aparecería tres
pasos después, al no poder abrir el sobre, con un mensaje que no apunta a nada.

Consecuencia del largo exacto: con marco, la clave conserva los saltos de línea
del final. Sin marco se le sacan, porque ahí son del pipe. El marco hace
expresable una clave que sin él era imposible de mandar.

Con `--para` o `--identidad` no viaja ninguna clave por stdin, así que **el marco
no hace falta**: stdin queda entero para el contenido.

---

## 5. Los códigos de salida: el semáforo

> **El portero.** Si te dice "no entrás" y nada más, no sabés si volver con otra
> llave o irte a tu casa. Si te dice *"esa llave no es"* volvés con otra. Si te
> dice *"esto no es una puerta, es una pared pintada"* te vas. Es el mismo
> rechazo, pero uno te dice qué hacer después.

| | Significa | Qué hace quien llama |
|--:|---|---|
| `0` | salió bien | seguir |
| `1` | uso, disco, permisos | arreglar y reintentar |
| `2` | ese secreto no abre ese sobre | volver a pedir la clave |
| `3` | no es un sobre que abra así | rendirse |
| `4` | pide más trabajo del que esta máquina gasta | subir `--max-trabajo` o rendirse |

El código de salida es **la única parte de este programa que otro programa puede
leer sin parsear texto**. El texto de los mensajes puede cambiar entre versiones;
el número no.

El `3` cubre también el cruce entre modos: una passphrase contra un sobre x25519,
o al revés. Es un sobre age perfectamente válido; simplemente no se abre así.

---

## 6. El factor de trabajo: la caja fuerte de manivela

> **La caja fuerte que se abre a manivela.** No tiene teclado: para abrirla hay
> que darle vueltas. Vos le das las vueltas una vez y tardás dos segundos. El que
> quiere probar un millón de combinaciones tiene que dar las vueltas **un millón
> de veces**.
>
> Bajar el número de vueltas te ahorra tiempo a vos y le ahorra exactamente el
> mismo tiempo a él.

Eso es scrypt. El "número de vueltas" es el factor de trabajo, y el costo es
`2^n`: cada punto que sube duplica el tiempo.

```
--trabajo 10   ~78 ms
--trabajo 16   ~208 ms
--trabajo 20   ~2 500 ms
```

Medido: factor 20 contra factor 16 dio 16,2×, contra un 16 predicho por la
fórmula. La escala se cumple.

**El número está grabado en la caja, no lo elige quien la abre.** `age` lo mide
al cerrar, apuntando a un segundo en *esa* máquina, y lo escribe en el sobre.
Quien abre paga lo que diga el sobre.

De ahí sale la trampa más práctica de todas: una máquina acepta hasta 4 puntos
por encima de su propio segundo, o sea 16 veces su trabajo. **Un sobre cerrado en
una máquina más de ~16× más rápida que la que lo abre, no abre:**

```
sobre: notas.sobre fue cerrado con un factor de trabajo de 22 y esta maquina
       acepta hasta 18. Usá --max-trabajo 22 si estás dispuesto a esperar 16x
       lo que tarda normalmente.
```

Sale con `4` y con los dos números, porque son los que hacen falta para elegir un
`--max-trabajo` sin adivinar.

**Dónde miente la analogía:** la caja fuerte real la puede forzar un soplete.
Acá el único camino es probar claves, y por eso el número de vueltas es toda la
defensa.

---

## 7. El acolchado: la caja de zapatos

> **La caja de zapatos.** Si mandás un anillo en una cajita de anillo, el
> cartero sabe que es un anillo sin abrir nada. Si mandás todo —el anillo, la
> carta, el reloj— en cajas de zapatos idénticas, lo único que sabe es "algo que
> entra en una caja de zapatos".

Un sobre pesa lo que pesa el contenido más una cabecera y 16 bytes por cada
bloque de 64 KiB. O sea que **el tamaño del sobre describe el contenido**:

| contenido | sobre sin acolchar | sobre con `--bloque 4K` |
|--:|--:|--:|
| 10 B | 323 | 4 369 |
| 1 500 B | 1 874 | 4 419 |
| 3 000 B | 3 360 | 4 432 |
| 4 000 B | 4 285 | 4 391 |

Sin acolchar, la columna sigue al contenido casi byte a byte. Con acolchado, no.

El esquema es ISO/IEC 7816-4: un byte `0x80` y después ceros, hasta el próximo
múltiplo. **Siempre se agrega algo**, aun cuando el contenido ya caiga justo,
porque un acolchado que pudiera medir cero sería indistinguible de no tener
acolchado.

El acolchado va **adentro del texto en claro**, que es el único lugar donde
sirve: `age` verifica el largo y rechaza cualquier cosa que venga después del
sobre cerrado. La prueba directa está en
[hallazgos.md](hallazgos.md#5-age-rechaza-los-bytes-de-más).

Viene en dos formas, y hacen lo mismo:

```bash
# como flag
sobre cifrar --para age1... --bloque 4K v.txt r.sobre
sobre abrir  --identidad mi.identidad --desrellenar r.sobre v.txt

# como piezas del caño, cuando conviene que el acolchado se vea
cat v.txt | sobre rellenar --bloque 4K --stdin --stdout \
          | sobre cifrar --para age1... --stdin r.sobre
```

---

## 8. El grease: el renglón basura del formulario

> **El campo que no significa nada.** Si un formulario tiene siempre exactamente
> tres campos, tarde o temprano alguien escribe un lector que asume tres campos.
> El día que agregues un cuarto, se rompe medio mundo.
>
> La solución es meter desde el primer día un cuarto campo con basura aleatoria,
> que no significa nada y cambia en cada formulario. Así ningún lector puede
> asumir nada, y todos aprenden a ignorar lo que no conocen.

`age` mete una stanza **grease** de largo aleatorio en la cabecera cuando hay
destinatarios. Es la misma jugada anti-osificación que usa TLS. Se ve así:

```
-> R$1J/g(~-grease VU0, EY% @"q#`HY
yBhGP+vCtzQ6of8HkvLHYWOapuHbRluR2oqtQnLIaq6mfnbc2NcVbPawLFkdU6ga
```

Efecto medible: dos sobres del mismo contenido **no pesan igual**.

| cerrado con | un archivo de 100 B, ocho veces, `--bloque 4K` |
|---|---|
| passphrase | 4 278 bytes las ocho |
| `--para` | siete tamaños distintos, entre 4 332 y 4 402 |

Un sobre de scrypt tiene que tener exactamente un destinatario, así que ahí no se
agrega señuelo y el acolchado cuantiza el tamaño de forma exacta. Donde el
señuelo está, su dispersión es de decenas de bytes y **no correlaciona con el
contenido**: el bloque es lo que lleva significado, el grease es ruido encima.

---

## 9. Los flujos: la cinta transportadora

> **El depósito y la cinta.** Para cifrar algo que sólo existe en memoria, antes
> había que bajarlo al depósito (escribirlo en claro al disco), procesarlo, y
> borrar el depósito. La cinta transportadora no tiene depósito: entra por un
> lado, sale por el otro, nunca se apoya en ningún lado.

```
sobre cifrar --stdin  <salida>     el contenido entra por stdin
sobre abrir  --stdout <entrada>    el contenido sale por stdout
sobre cifrar --stdin --stdout      filtro puro: no toca disco de ningún lado
```

Cada flag de flujo se lleva su argumento posicional. Las cinco órdenes los
aceptan.

Lo que hace esto encadenable no son los flags, sino una separación que ya estaba:
**por stdout salen datos, por stderr salen palabras.** Un programa que mezcla las
dos cosas en stdout no se puede poner en el medio de un caño.

```bash
tar cz /datos | sobre cifrar --para age1... --stdin --stdout > backup.sobre
```

---

## 10. Todo junto: los movimientos posibles

Las cinco órdenes, con dónde entra y dónde sale cada una:

| Orden | Entra | Sale | Secreto que usa |
|---|---|---|---|
| `cifrar` | claro | cerrado | clave, o `--para` |
| `abrir` | cerrado | claro | clave, o `--identidad` |
| `generar` | — | identidad | ninguno |
| `rellenar` | claro | claro acolchado | ninguno |
| `desrellenar` | claro acolchado | claro | ninguno |

Y para `cifrar` y `abrir`, con dos entradas y dos salidas posibles, ocho
movimientos:

| Entra por | Sale por | ¿El claro toca disco? |
|---|---|---|
| archivo | archivo | sí |
| archivo | stdout | sólo de un lado |
| stdin | archivo | sólo de un lado |
| stdin | stdout | **no** |

La última fila es la que motivó todo: un secreto que sólo existe en memoria ya no
necesita aterrizar en claro para poder ser cifrado.
