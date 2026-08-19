# Preguntas frecuentes, por caso de uso

---

## Guardar un secreto propio

**¿Alcanza con la passphrase o conviene una identidad?**
Para archivos propios que abrís vos a mano, la passphrase. No tenés que cuidar
ningún archivo y los dos segundos no se sienten. La identidad conviene cuando
algo automático tiene que abrir sobres sin que haya alguien tipeando.

**Si alguien me roba el `.sobre`, ¿puede probar contraseñas?**
Sí, todas las que quiera, en su máquina, sin límite y sin que te enteres. Eso se
llama ataque offline y no hay forma de evitarlo: el archivo lo tiene él. Lo único
que lo frena es que cada intento le cueste. Por eso scrypt tarda un segundo a
propósito y por eso bajar `--trabajo` no es gratis.

**¿Puedo cambiar la contraseña de un sobre sin abrirlo?**
No. Hay que abrirlo con la vieja y cerrarlo con la nueva. La clave del archivo
está envuelta con la que derivó la passphrase, y para volver a envolverla hay que
desenvolverla primero.

**¿Qué pasa si el proceso se corta a la mitad de cifrar?**
El sobre queda truncado y no abre nunca más. No es recuperable ni parcialmente:
falta el cierre del último bloque. Al abrirlo da código `3`. Por eso conviene
cifrar a un archivo temporal y renombrar recién cuando `sobre` salió con `0`.

**¿Y si me olvido la contraseña?**
Se perdió el contenido. No hay recuperación, ni puerta de atrás, ni forma de
pedirle nada a nadie. Es la contrapartida de que tampoco la tenga nadie más.

---

## Backups y pipelines automáticos

**¿Puedo poner `sobre` en un cron o en un servicio?**
Sí, y ahí conviene `--para`. Una passphrase en un script automático tiene que
estar guardada en algún lado igual, así que no ganás nada, y encima pagás dos
segundos por corrida. Con `--para` no hay ningún secreto del lado que cifra.

```bash
tar cz /datos | sobre cifrar --para age1... --stdin --stdout > backup.sobre
```

**¿Por qué tarda dos segundos si el archivo es chico?**
Porque el costo fijo no es del archivo, es de la derivación de la clave. Un
archivo de 1 KB y uno de 1 GB pagan el mismo par de segundos. Para muchos
archivos chicos eso se vuelve el 99% del tiempo — mil archivos son más de media
hora de pura derivación. La solución es meter todo en un sobre, o usar `--para`.

**¿`--para` es menos seguro por ser tan rápido?**
No: es otra cosa. Lo que tarda en la passphrase es adivinar la passphrase, y eso
sólo hace falta porque las personas eligen claves cortas. Una clave x25519 es
aleatoria de 256 bits: no se adivina, así que no hay nada que encarecer. El costo
se mudó de "resistir intentos" a "cuidar un archivo".

**¿Y si me roban el archivo de identidad?**
El que lo tiene abre todos los sobres cerrados a esa clave pública, sin más. No
tiene contraseña encima. Si eso te preocupa, la identidad se puede guardar dentro
de un sobre con passphrase — pero entonces volvés a pagar los dos segundos para
llegar a ella, que es exactamente el intercambio que estabas evitando.

**¿Cómo sé desde un script si falló por la clave o por otra cosa?**
Por el código de salida, sin leer el mensaje:

```bash
sobre abrir --identidad id.txt x.sobre salida.txt
case $? in
  0) echo "abrió" ;;
  2) echo "la clave no es esa: vuelvo a pedirla" ;;
  3) echo "no es un sobre que abra así: me rindo" ;;
  4) echo "pide más trabajo del que gasto: subo --max-trabajo" ;;
  *) echo "otra cosa" ;;
esac
```

**¿Qué pasa si el que consume el pipe corta antes de tiempo?**
`sobre` sale con `1` y un error de escritura. No sale con `0`: un contenido
truncado no se reporta como éxito.

---

## Mandarle algo a otra persona

**¿El otro necesita tener `sobre`?**
No. El formato es age estándar, así que le abre con la herramienta oficial de
`age` o con cualquier implementación. Lo único que `sobre` agrega y otros no
sacan es el acolchado de `--bloque`, que quedaría pegado al final del contenido.

**¿Puedo cerrar un sobre para varias personas?**
Sí, repitiendo `--para`. El sobre abre para cualquiera de esas identidades, cada
una con la suya.

```bash
sobre cifrar --para age1aaa... --para age1bbb... informe.pdf informe.sobre
```

**¿Puedo mezclar passphrase y destinatarios en el mismo sobre?**
No, y no es una limitación de `sobre` sino del formato: un sobre de scrypt tiene
que tener exactamente un destinatario y no se puede combinar con otros tipos.

**¿Cómo le paso la contraseña al otro?**
Por un canal distinto del que usaste para el archivo. Si mandás las dos cosas por
el mismo mail, el que lea el mail tiene las dos. Esto no lo resuelve ninguna
herramienta: es el problema que las identidades x25519 existen para evitar.

---

## Ocultar el tamaño

**¿`--bloque` esconde qué archivo es?**
Esconde el tamaño exacto, que muchas veces alcanza para identificarlo. No esconde
el tamaño aproximado: un sobre de 4 KB y uno de 40 MB siguen siendo distintos. Lo
que hace es meter todo lo que entra en un bloque en la misma bolsa.

**¿Qué bloque conviene?**
El más chico que meta a todos tus archivos en pocas bolsas distintas. Si todo lo
que cifrás son notas de menos de 4 KB, `--bloque 4K` los vuelve indistinguibles
entre sí. Si tenés archivos de 3 KB y de 300 MB, ningún bloque los va a
confundir, y el precio de intentarlo es multiplicar el chico por cien mil.

**¿Por qué dos sobres del mismo tamaño no pesan igual?**
Si los cerraste con `--para`, por el grease: `age` mete a propósito una entrada
señuelo de largo aleatorio en la cabecera. Es ruido y no dice nada del contenido.
Con passphrase no pasa: ahí el tamaño queda exactamente constante.

**¿Puedo desacolchar un sobre que no está acolchado?**
Falla con código `1` y el mensaje dice que no encontró la marca `0x80`. No borra
nada al azar: si no hay acolchado, no hay nada que sacar y lo dice.

**¿El acolchado se puede aplicar dos veces?**
Sí, y desacolchar dos veces lo saca. Pero no tiene sentido: el segundo acolchado
opera sobre el primero, así que sólo agrega bloques.

---

## Cuando algo no abre

**"fue cerrado con un factor de trabajo de X" — ¿qué hago?**
El sobre se cerró en una máquina bastante más rápida que la que lo abre. Corré lo
mismo con `--max-trabajo X`, usando el número que el mensaje te dio. Va a tardar,
y el mensaje te dice cuánto más.

**¿Se puede evitar de antemano?**
Sí: cerrando con `--trabajo <n>` fijo en vez de dejar que `age` mida la máquina.
Así el costo queda declarado y no depende de dónde se cerró. Para algo que tenés
que poder abrir en diez años en hardware desconocido, conviene.

**Da código `3` y estoy seguro de que es un sobre. ¿Qué pasa?**
El `3` también aparece cuando el sobre está bien pero cerrado del otro modo:
passphrase contra un sobre x25519, o identidad contra un sobre de passphrase.
Fijate con qué se cerró.

**Da código `2` y la contraseña es la correcta.**
Revisá la codificación. Si el sobre se cerró con `--utf16le` y lo estás abriendo
sin el flag —o al revés— la clave derivada es otra. Los dos caminos producen el
mismo texto **si la clave llega bien**; lo que cambia es cómo se interpretan los
bytes que entran.

**Con `--stdin` no anda y sin `--stdin` sí.**
Con `--stdin` y passphrase, stdin lleva las dos cosas y necesita el marco:
`<largo>\n<clave><contenido>`. Sin el marco, `sobre` lee el largo de donde
empiece la clave y falla. Con `--para` o `--identidad` no hay marco porque no hay
clave en el caño.

---

## Cosas que la herramienta no hace

**¿Firma? ¿Puedo saber quién cerró el sobre?**
No. Un sobre garantiza que el contenido no fue modificado, no quién lo escribió.
Cualquiera con tu clave pública puede cerrarte un sobre a tu nombre. Si necesitás
saber el autor, hace falta una firma, que es otra herramienta.

**¿Comprime?**
No. Comprimí antes si querés: `tar cz` y después `sobre`. En ese orden — al revés
no comprime nada, porque el texto cifrado es indistinguible de ruido.

**¿Sirve para bases de datos o archivos que cambian?**
No como está. Cifra de punta a punta: para cambiar un byte hay que abrir todo y
volver a cerrar todo. Para datos vivos hace falta cifrado por bloques con acceso
aleatorio, que es un problema distinto.

**¿Puedo usarlo para contraseñas de un sistema bancario o similar?**
La criptografía alcanza; lo que falta es todo lo demás. Custodia en HSM,
rotación, revocación, doble control, traza de auditoría. Y ChaCha20-Poly1305 y
scrypt no están en la lista FIPS, que es un bloqueo regulatorio y no una crítica
técnica. "Es seguro" y "es aprobable" son dos ejes distintos.

**¿Y para algo con requisitos de seguridad funcional?**
No. Ese tipo de software pide, entre otras cosas, no reservar memoria en tiempo
de ejecución y tener tiempo de ejecución acotado. `sobre` reserva en el heap,
arrastra decenas de dependencias, y `panic = "abort"` significa morir ante lo
inesperado en vez de degradar de forma definida. `--trabajo` hace determinista el
costo de derivación, que es un paso, pero un paso de muchos.
