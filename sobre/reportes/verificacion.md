# Verificación

Qué está probado, con qué, y con qué números. Todo lo de acá se reproduce con
`cargo test` y con el binario de release.

---

## Pruebas automáticas

```
cargo test          29 pruebas
cargo clippy        sin avisos
```

Las 29 corren en menos de un milisegundo porque ninguna cifra nada: prueban el
marco, el acolchado y la clasificación de errores, que es donde vive la lógica
propia. Lo criptográfico lo prueba el crate `age`, que ya viene con las suyas.

| Grupo | Pruebas | Qué fija |
|---|--:|---|
| El marco de `--stdin` | 12 | separación clave/contenido, largo exacto, rechazos |
| Códigos de salida | 5 | que 1, 2, 3 y 4 no se pisen ni se confundan |
| Tamaños y factores | 2 | `4K`/`1M`, y el rango 1–63 antes de que `age` panickee |
| Acolchado | 8 | ida y vuelta, y los casos que rompen una implementación ingenua |
| Identidades | 2 | que una identidad generada vuelva de texto |

Las que vale la pena mirar de cerca:

**`marco_con_clave_que_contiene_0x0a_no_se_parte`** — usa `U+0A0A`, cuyos dos
bytes en UTF-16LE son `0x0A 0x0A`. Es el caso que hunde a cualquier separador por
salto de línea.

**`los_dos_caminos_derivan_la_misma_clave`** — la misma passphrase con acentos y
`ñ`, entrada por UTF-8 y por UTF-16LE, tiene que dar el mismo `SecretString`. Si
esto fallara, un sobre cerrado con `--utf16le` no abriría sin el flag.

**`clave_incorrecta_se_distingue_de_sobre_invalido`** — fija que
`DecryptionFailed` sea código `2` y `NoMatchingKeys` código `3`. El nombre de la
segunda induce a lo contrario; ver [hallazgos](hallazgos.md#2-nomatchingkeys-no-es-clave-incorrecta).

**`contenido_que_termina_en_marca_y_ceros_no_confunde`** — contenido real que
termina en `0x80 0x00 0x00 0x00`, o sea igual que un acolchado, seguido de un
acolchado de verdad. El sufijo retenido tiene que ceder cuando llega el real.

**`el_acolchado_cruza_los_bordes_de_lectura`** — 200 KB con buffer de lectura de
64 KiB, para que el sufijo candidato sobreviva de una lectura a la siguiente.

**`el_factor_de_trabajo_se_valida_antes_de_que_age_panickee`** —
`set_work_factor` hace `assert!(0 < n && n < 64)`. Validar antes convierte un
panic en un mensaje.

---

## Pruebas de punta a punta

57 chequeos contra el binario de release, mirando lo que ve el shell: el código
de salida y los bytes producidos.

| Grupo | Chequeos |
|---|--:|
| Regresión — lo que ya andaba sigue andando | 8 |
| Identidades x25519 | 16 |
| Factor de trabajo | 8 |
| Acolchado | 15 |
| Validación de flags | 10 |

**Regresión.** Cifrar y abrir clásico, ida y vuelta con acentos, `--stdin`
enmarcado, clave incorrecta a `2`, no-es-sobre a `3`, y `--version`.

**Identidades.** `generar` escribe la privada y deja la pública de comentario;
ida y vuelta con `--para`/`--identidad`; el sobre arranca con la cabecera age
estándar; caño puro sin marco; dos destinatarios, y cada uno abre con la suya. Y
los tres cruces que tienen que fallar con `3`: clave contra sobre x25519,
identidad contra sobre de clave, e identidad ajena.

**Factor de trabajo.** `--trabajo 12` cierra y abre; `--max-trabajo 5` contra un
sobre normal da `4` y el mensaje nombra el flag que lo arregla; `--max-trabajo
40` lo abre; `--trabajo 99` y `--trabajo 0` dan `1` sin panic.

**Acolchado.** Ida y vuelta por bloque en 10, 1000 y 5000 bytes; sin
`--desrellenar` el contenido vuelve con la cola —4096 bytes exactos—; las órdenes
sueltas `rellenar` y `desrellenar` encadenadas; el caño de cuatro procesos
`rellenar | cifrar` y `abrir | desrellenar`; y desacolchar algo sin cola da `1`.

El chequeo de tamaño no compara bytes exactos, porque el grease lo hace
imposible. Verifica la propiedad: dos contenidos de 10 y 3000 bytes acolchados al
mismo bloque tienen que diferir en menos de 200 bytes, y **sin** acolchar en más
de 2000.

**Validación.** Diez combinaciones sin sentido que tienen que salir con `1`:
`--identidad` en cifrar, `--para` en abrir, `--trabajo` junto a `--para`,
`--max-trabajo` junto a `--identidad`, `--bloque` en abrir, `rellenar` sin
`--bloque`, `--stdin` en generar, la clave por argumento, y un flag con valor
faltante. Más `--flag=valor`, que tiene que andar igual que `--flag valor`.

---

## Mediciones

Una máquina, mínimo de varias corridas. Los tiempos incluyen el arranque del
proceso desde el shell, que en este arnés tiene un piso de ~41 ms; por eso las
cifras chicas no se pueden separar entre sí.

### Binario

```
833 024 bytes  (0,79 MB)
```

### Costo fijo de cerrar 1 KB

| | |
|---|--:|
| passphrase, factor medido por `age` | ~2 500 ms |
| passphrase, `--trabajo 20` | ~2 523 ms |
| passphrase, `--trabajo 16` | ~208 ms |
| passphrase, `--trabajo 10` | ~78 ms |
| `--para` (x25519) | indistinguible del arranque |

Abrir cuesta lo mismo que cerrar, porque el factor está escrito en el sobre:

| | |
|---|--:|
| el del factor calibrado | ~2 501 ms |
| el de `--trabajo 20` | ~2 479 ms |
| el de `--trabajo 10` | ~88 ms |

Descontado el arranque, factor 20 contra factor 16 da **16,2×**, contra un 16
predicho por `2^(20-16)`.

### Volumen

64 MB de datos aleatorios, con `--para` para que no haya derivación en el medio:

| | |
|---|--:|
| cifrar | 173 ms |
| abrir | 180 ms |
| ida y vuelta | idéntico |

Descontando el arranque quedan ~485 MB/s cifrando y ~460 MB/s abriendo. El
[README](../README.md#performance) reporta 527 MB/s con una metodología más
cuidada; estas cifras la corroboran, no la corrigen.

### Sobrecosto de tamaño

Medido en el camino de passphrase, donde no hay grease:

| contenido | sobre | de más |
|--:|--:|--:|
| 0 | 182 | +182 |
| 1 | 183 | +182 |
| 65 536 | 65 718 | +182 |
| 65 537 | 65 735 | +198 |
| 1 048 576 | 1 048 998 | +422 |

De donde sale la fórmula:

```
sobre = contenido + 166 + 16 × techo(contenido / 65536)
                    ▲           ▲
                    │           └─ un sello Poly1305 por bloque de 64 KiB
                    └─ cabecera age + nonce
```

Verifica en las cinco filas.

### Dispersión por el grease

Un archivo de 100 bytes, `--bloque 4K`, ocho corridas por camino:

| cerrado con | tamaños obtenidos |
|---|---|
| passphrase | 4 278 las ocho veces |
| `--para` | 4 332, 4 353, 4 354, 4 355 ×2, 4 358, 4 361, 4 402 |

Y sobre el mismo archivo con `--para`, doce corridas dieron doce tamaños entre
4 328 y 4 415. Contenidos de 10 a 4 000 bytes acolchados al mismo bloque cayeron
todos dentro de esa banda: la dispersión no correlaciona con el contenido.

---

## Lo que no está probado automáticamente

- **Compatibilidad con la CLI oficial de `age`.** El formato es el mismo y la
  cabecera se verifica, pero no hay una prueba que abra un sobre con la
  herramienta oficial. Requiere tenerla instalada.
- **El caso de `--max-trabajo` real.** Se prueba forzando un tope bajo, no con
  dos máquinas de velocidades distintas.
- **Volúmenes arriba de 64 MB**, y archivos que no entren en memoria del lado del
  acolchado —que retiene hasta un bloque, con tope de 16 MiB.
- **Concurrencia.** Nada prueba dos procesos escribiendo al mismo archivo de
  salida.
