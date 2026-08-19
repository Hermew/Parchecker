# sobre

**Encrypted files with a secret that never touches the command line.**

A 0.8 MB binary. The passphrase is read from stdin and from nowhere else.

```
sobre cifrar notas.txt notas.sobre
sobre abrir  notas.sobre notas.txt
```

Output is standard [age](https://age-encryption.org/v1) — any age implementation opens it, not just this one.

---

## Orders

| | |
|---|---|
| `sobre cifrar <entrada> <salida>` | close the envelope |
| `sobre abrir <entrada> <salida>` | open it |
| `sobre generar <salida>` | create an x25519 identity |
| `sobre rellenar <entrada> <salida>` | pad the size to a block boundary |
| `sobre desrellenar <entrada> <salida>` | strip that padding |

---

## Why stdin only

Process arguments are readable by every other process on the machine, and they land in shell history. Most archivers take the passphrase that way and offer no alternative.

```
$ sobre cifrar -pMyPassword input output
sobre: la clave no se pasa por argumento, nunca. Mandala por stdin:
       los argumentos de un proceso los puede leer cualquier otro proceso.
```

Rejected before anything is read. Making the wrong thing impossible beats documenting that it should not be done.

The ban is on **secrets**, not on arguments. An x25519 recipient is a public key, so `--para age1...` goes on the command line without a second thought — that is what public means.

---

## Two ways to close it

```
                       cost per run       good for
passphrase (stdin)     ~2.5 s             a few large envelopes
--para age1...         ~0 s               many small ones, unattended
```

A passphrase runs through scrypt, and that is a second of deliberate work at each end. An x25519 recipient replaces the derivation with an elliptic-curve exchange, which is free at this scale.

```bash
sobre generar mi.identidad          # the public half is printed to stderr
sobre cifrar --para age1... notas.txt notas.sobre
sobre abrir --identidad mi.identidad notas.sobre notas.txt
```

`--para` may be repeated, and the envelope then opens for any one of those recipients.

The identity file holds `AGE-SECRET-KEY-1...` — as secret as a passphrase, and now sitting on disk. That is the trade: a passphrase lives in a head and costs a second; a key file lives on a disk and costs nothing. Neither is the safer one in general, only for a given threat.

With `--para` or `--identidad` no passphrase is read at all, which leaves stdin entirely to the content — so `--stdin` needs no frame in that mode.

---

## With the Parchecker prompt

```powershell
powershell -File ..\askpass\Askpass.ps1 -Confirmar |
    sobre cifrar --utf16le notas.txt notas.sobre
```

`--utf16le` exists because `Askpass.ps1` writes raw UTF-16LE, which has been its contract from the start. The askpass required no modification — that is the point of keeping it separate.

---

## Nothing has to land in the clear

A secret that only exists in memory should not need a detour through the filesystem to get encrypted. `--stdin` and `--stdout` let the content travel through the same pipe as the passphrase.

```
sobre cifrar --stdin  <salida>     content arrives on stdin, behind the passphrase
sobre abrir  --stdout <entrada>    plaintext leaves on stdout
sobre cifrar --stdin --stdout      pure filter: nothing touches disk on either side
```

Each stream flag drops its positional argument. Both work on every order.

Guarding the passphrase across three buffers and then requiring the content to pass through disk would be locking the door and leaving the window open.

### The frame

With `--stdin` and a passphrase, both share one stream, so the boundary has to be stated:

```
<passphrase length in bytes, ASCII decimal> \n <passphrase> <content>
```

```
printf '13\nclave-secretawhatever follows is the content' | sobre cifrar --stdin notas.sobre
```

An explicit length rather than a separator, because in UTF-16LE any character can carry `0x0A` as its low byte. Cutting at the first newline would split the passphrase in the wrong place, and nothing downstream could tell: the truncated passphrase is still a valid passphrase, just a different one. The failure would surface three steps later, at decryption, pointing nowhere.

The length is exact. A framed passphrase keeps its trailing newlines, because the byte count already said where it ends. Without a frame, stdin in its entirety is the passphrase and trailing newlines are stripped — those belong to the pipe, not to the secret.

Content is streamed straight into the encryptor. Nothing is buffered whole, at any size.

---

## Padding

An envelope is the size of its content plus a header and 16 bytes per 64 KiB chunk, so its size describes what is inside. `--bloque` rounds the content up to a multiple before sealing.

```bash
sobre cifrar --para age1... --bloque 4K v.txt r.sobre
sobre abrir  --identidad mi.identidad --desrellenar r.sobre v.txt
```

The same thing as separate pipeline stages, when the padding should be visible in the chain rather than hidden in a flag:

```bash
cat v.txt | sobre rellenar --bloque 4K --stdin --stdout \
          | sobre cifrar --para age1... --stdin r.sobre
```

The scheme is ISO/IEC 7816-4: one `0x80`, then zeros, up to the next multiple. Something is always appended, even when the content already lands on a boundary — padding that could measure zero would be indistinguishable from no padding at all.

> [!NOTE]
> The padding lives inside the plaintext, which is the only place it can do anything: `age` verifies the length and rejects trailing bytes, so padding the sealed envelope makes it unopenable.
>
> A padded envelope opens in any age implementation. What that implementation returns is the content with the tail still attached — only `sobre desrellenar` (or `abrir --desrellenar`) removes it.

Measured, one machine, x25519 recipient, `--bloque 4K`:

| content | envelope, padded | envelope, unpadded |
|--:|--:|--:|
| 10 B | 4 369 | 323 |
| 1 500 B | 4 419 | 1 874 |
| 3 000 B | 4 432 | 3 360 |
| 4 000 B | 4 391 | 4 285 |

Unpadded, the envelope tracks the content almost byte for byte. Padded, it does not.

The padded column is not constant, and how constant it gets depends on which way the envelope was closed:

| sealed with | one 100 B file, sealed 8 times, `--bloque 4K` |
|---|---|
| passphrase | 4 278 bytes, all eight |
| `--para` | seven distinct sizes between 4 332 and 4 402 |

`age` writes a **grease** stanza of random length into the header when recipients are used — a decoy that forces implementations to correctly ignore stanzas they do not recognise, the same anti-ossification trick TLS uses. An scrypt envelope must have exactly one recipient, so no decoy is added there and padding quantizes the size exactly.

Where the decoy is present its spread is tens of bytes and does not correlate with the content: the block is what carries the meaning, the grease is noise on top of it.

---

## Exit codes

The exit code is the only part of this program another program can read without parsing text.

| | |
|--:|---|
| `0` | done |
| `1` | usage, disk, permissions |
| `2` | that secret does not open that envelope |
| `3` | the input is not an age envelope that opens this way |
| `4` | the envelope demands more work than this machine will spend |

A wrapper can decide between re-prompting, raising the limit, and giving up, without matching on error strings.

> [!NOTE]
> `2` is the AEAD failing to unwrap the file key. `3` also covers a valid age file sealed the other way — a passphrase against an x25519 envelope, or the reverse. `4` is described below.

---

## Work factor

scrypt costs `2^n`, and `age` picks `n` by measuring the machine so that deriving takes about a second. The number is written into the envelope, so **the envelope decides what opening it costs**, not the machine opening it.

That machine will refuse a factor more than 4 above its own — sixteen times its second. So an envelope sealed on hardware more than ~16× faster than the one opening it does not open:

```
sobre: notas.sobre fue cerrado con un factor de trabajo de 22 y esta maquina
       acepta hasta 18. Usá --max-trabajo 22 si estás dispuesto a esperar 16x
       lo que tarda normalmente.
```

Exit `4`, with both numbers, because those are what choosing a `--max-trabajo` requires.

| | |
|---|---|
| `--trabajo <n>` | fix the factor when sealing, instead of measuring |
| `--max-trabajo <n>` | raise what this machine will accept when opening |

`--trabajo` is a speed dial and a security dial at once — the same lever, read from either end. Dropping it is not free work saved; it is brute force made cheaper by the same factor.

---

## Memory

The passphrase is wiped from every buffer it passes through, not only from the final one.

| Stage | Covered by |
|---|---|
| Raw bytes from stdin | `Zeroizing<Vec<u8>>` |
| 16-bit units, `--utf16le` only | `Zeroizing<Vec<u16>>` |
| Decoded text | `Zeroizing<String>` |
| Final value | `SecretString` |

A plain `Vec` or `String` is freed without being overwritten, so its contents stay in the heap until something else reuses that memory. The `--utf16le` path matters most: it is the one the askpass uses, and it makes two more copies than the other, converting bytes to `u16` and `u16` to text.

Each wrapper goes on **before** the buffer is filled, not after. A read that fails halfway still leaves whatever arrived covered.

### Growth is the copy nobody wipes

**Every buffer that holds the passphrase is allocated at its final size.** A growing `String` reallocates, and reallocating copies the bytes into a fresh block and releases the old one exactly as it stood. That abandoned block holds the passphrase and no wrapper reaches it — `Zeroizing` clears the buffer it knows about, not the one the allocator already took back. `zeroize` states the limit itself, for `Vec`: *"cannot ensure that previous reallocations did not leave values on the heap."*

Two places in this path would reallocate on their own:

- `String::from_utf16` reserves one UTF-16 unit per UTF-8 byte, an arithmetic that only holds for ASCII. An `ñ` is one unit and two bytes, an accent likewise, so a Spanish passphrase overruns the reservation and grows. Decoding here reserves the worst case — three UTF-8 bytes per unit — and pushes into a buffer that never has to grow.
- `SecretString::from(String)` goes through `into_boxed_str()`, which calls `shrink_to_fit()`. Spare capacity makes that reallocate. The value handed over is built with capacity equal to its length, so there is nothing to shrink.

On the UTF-8 path validation uses `str::from_utf8`, which inspects without consuming. `String::from_utf8` would save a copy but returns an error **holding the bytes**, so the passphrase would travel inside the error message. `char::decode_utf16` has the same property on the UTF-16LE path — its error keeps the unpaired surrogate — so it is replaced with a fixed string before it can go anywhere.

No extra dependency: `age` already vendors `zeroize` under `secrecy`, used as `age::secrecy::zeroize::Zeroizing`.

> [!NOTE]
> This shortens the window; it does not close it. A memory dump taken **while** the passphrase is in use still finds it, and the kernel pipe buffer held it too. An identity file, being a file, is outside this entirely. See the threat model in the [project README](../README.md).

---

## Cryptography

The format is [age](https://age-encryption.org/v1): ChaCha20-Poly1305, with scrypt or X25519 to reach the file key. Specified, audited, and implemented by the `age` crate. This program moves bytes and keeps the secret off the command line. Nothing more.

Third-party licenses: [TERCEROS.md](TERCEROS.md), generated from `cargo metadata`.

---

## Performance

Measured on one machine, minimum of 5 runs, 64 MB of random data so compression cannot cheat.

**Startup** — launch the process and exit:

| | |
|---|--:|
| `sobre.exe` (Rust) | **6.9 ms** |
| `python -c pass` | 67.4 ms |
| `python` + `import cryptography` | 91.1 ms |
| `powershell -NoProfile` | 136.3 ms |

**Encryption throughput** — MB/s, fixed cost excluded:

| | |
|---|--:|
| `sobre` (ChaCha20-Poly1305) | 527 MB/s |
| Python equivalent (`cryptography`) | 556 MB/s |
| `rar -m0` (AES, no compression) | 584 MB/s |

Those are the same number. Nobody writes the hot loop for cryptography in their own language: Python calls OpenSSL, `age` uses SIMD crates, RAR uses the CPU's AES-NI. All three end in hand-optimized native code. Language choice affects startup, distribution and memory handling — not encryption speed.

### Where the fixed cost lives

Sealing a 1 KB file with a passphrase takes about two seconds, and all of it is scrypt deriving the key. Timings below include process spawn, so the last row is at the floor of what can be measured this way:

| | sealing 1 KB |
|---|--:|
| passphrase, factor measured by `age` | ~2 500 ms |
| passphrase, `--trabajo 16` | ~208 ms |
| passphrase, `--trabajo 10` | ~78 ms |
| `--para age1...` (X25519) | indistinguishable from startup |

Each step of the work factor doubles the cost, and the measurements track that: factor 20 against factor 16 came out 16.2× apart, against a predicted 16.

The calibration itself is cheap. `age` runs scrypt once at a low factor and then extrapolates by doubling rather than re-measuring, so the second that gets spent is the real derivation, not the measurement of it.

> [!WARNING]
> `rar` takes 99 ms for the same file with its own key derivation. Being that much faster is worse, not better: it means its derivation is correspondingly cheaper to attack.

For one file, two seconds is not felt. For 500 files in a loop it is 20 minutes of pure derivation — put everything in one envelope, or use `--para`, which is what the age documentation recommends for programmatic use.

---

## Trade-off

A `.rar` opens for anyone with WinRAR. A `.age` opens for anyone with `age` or this tool.

For your own files, this is better. For sending to a client, the `.rar` path in the [main project](../README.md) still wins.

---

## Build

```
cargo build --release
cargo test
```

> [!WARNING]
> Use the **MSVC** toolchain, which `rustup` selects by default on Windows. It requires the C++ Build Tools — `rustup-init` offers to install them — and produces no surprises.
>
> The GNU toolchain is **not** an equivalent substitute despite the smaller download. It additionally requires a full MinGW-w64 installed separately, in a path without spaces, because `rustup`'s copy is missing `dlltool`. Procedure: [INSTALACION.md](../INSTALACION.md#camino-2--rust-sobre).

If the repository sits inside a cloud-synced folder, move build artifacts out: `target/` is hundreds of megabytes and changes on every build. Use a local `.cargo/config.toml`, **unversioned** because the path contains your username:

```toml
[build]
target-dir = "C:/Users/YOUR_USER/AppData/Local/cargo-target/sobre"
```

---

## Verified

Unit tests cover the frame parser, the padding scheme and the error classifier; the rest is exercised against the release binary.

| | |
|---|---|
| Round-trip with accents and `ñ` | returns identical |
| Wrong passphrase | rejected, exit `2` |
| Input that is not an envelope | rejected, exit `3` |
| Truncated envelope | rejected, exit `3` |
| Passphrase against an x25519 envelope, and the reverse | rejected, exit `3` |
| An identity that is not a recipient of the envelope | rejected, exit `3` |
| Work factor above what the machine accepts | rejected, exit `4`, both numbers named |
| Work factor outside 1–63 | rejected, exit `1`, no panic |
| Passphrase as an argument | rejected before anything is read |
| Mistyped order | rejected before stdin is read |
| Flags an order would ignore | rejected, not ignored |
| Existing output file | not overwritten without `--forzar` |
| UTF-16LE from `Askpass.ps1` | full round-trip |
| UTF-8 read as UTF-16LE | fails, as it must |
| Empty passphrase on stdin | rejected |
| Output format | every path begins with the `age` header |
| Envelope closed via UTF-8, opened via UTF-16LE, and the reverse | opens both ways |
| Same, with a 480-character passphrase | opens both ways |
| `--stdin` envelope | opens through the ordinary path |
| `--stdin --stdout` round-trip | returns identical |
| Framed passphrase containing `0x0A` | not split |
| Framed passphrase ending in a newline | preserved exactly |
| Malformed frame header | rejected, exit `1` |
| Generated identity | parses back as identity and recipient |
| Two recipients | envelope opens for each |
| Padding round-trip, 0 to 4 097 bytes | returns identical, size on the block |
| Content ending in `0x80` and zeros | not mistaken for padding |
| Content of all zeros | returns whole |
| Padding spanning a read boundary, 200 KB | returns identical |
| Padded size against content size | uncorrelated |
| Four-process pipeline, pad → seal → open → unpad | returns identical |

The UTF-8/UTF-16LE pairs confirm that both decoding paths derive exactly the same string. If they did not, the envelope would not open.

---

## License

**Apache 2.0** — see [LICENSE](../LICENSE).

Earlier Spanish documentation: [`.old/README-ESP.md`](.old/README-ESP.md).
