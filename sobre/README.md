# sobre

**Encrypted files with a passphrase that never touches the command line.**

A 0.7 MB binary. The passphrase is read from stdin and from nowhere else.

```
sobre cifrar notas.txt notas.sobre
sobre abrir  notas.sobre notas.txt
```

Output is standard [age](https://age-encryption.org/v1) — any age implementation opens it, not just this one.

---

## Why stdin only

Process arguments are readable by every other process on the machine, and they land in shell history. Most archivers take the passphrase that way and offer no alternative.

```
$ sobre cifrar -pMyPassword input output
sobre: la clave no se pasa por argumento, nunca. Mandala por stdin:
       los argumentos de un proceso los puede leer cualquier otro proceso.
```

Rejected before anything is read. Making the wrong thing impossible beats documenting that it should not be done.

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

Each stream flag drops its positional argument. Both work on both commands.

Guarding the passphrase across three buffers and then requiring the content to pass through disk would be locking the door and leaving the window open.

### The frame

With `--stdin` the passphrase and the content share one stream, so the boundary has to be stated:

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

## Exit codes

The exit code is the only part of this program another program can read without parsing text.

| | |
|--:|---|
| `0` | done |
| `1` | usage, disk, permissions |
| `2` | that passphrase does not open that envelope |
| `3` | the input is not an age envelope that a passphrase can open |

A wrapper can decide between re-prompting and giving up without matching on error strings.

> [!NOTE]
> `2` is the AEAD failing to unwrap the file key. `3` also covers a valid age file encrypted to an `x25519` or ssh recipient rather than to a passphrase: it is a real envelope, but no passphrase opens it.

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
> This shortens the window; it does not close it. A memory dump taken **while** the passphrase is in use still finds it, and the kernel pipe buffer held it too. See the threat model in the [project README](../README.md).

---

## Cryptography

The format is [age](https://age-encryption.org/v1): ChaCha20-Poly1305 with scrypt for key derivation. Specified, audited, and implemented by the `age` crate. This program moves bytes and keeps the passphrase off the command line. Nothing more.

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

> [!WARNING]
> **Encrypting 1 KB takes 1.9 seconds.**
>
> That is the KDF, deliberately. `age` does not use a fixed cost: it measures the machine on every run and calibrates scrypt to take about a second, so brute force costs the same on any hardware. Calibration itself runs scrypt several times to take that measurement — 1.88 s of the 1.89 s total.
>
> `rar` takes 99 ms for the same file. Being 19× faster is worse, not better: it means its key derivation is far cheaper to attack.

For a single file, two seconds is not felt. For 500 files in a loop it is 16 minutes of pure KDF — put everything in one envelope, or use an `x25519` identity instead of a passphrase, which is what the age documentation recommends for programmatic use.

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

Unit tests cover the frame parser and the error classifier; the rest is exercised against the release binary.

| | |
|---|---|
| Round-trip with accents and `ñ` | returns identical |
| Wrong passphrase | rejected, exit `2` |
| Input that is not an envelope | rejected, exit `3` |
| Truncated envelope | rejected, exit `3` |
| Passphrase as an argument | rejected before anything is read |
| Mistyped command | rejected before stdin is read |
| Existing output file | not overwritten without `--forzar` |
| UTF-16LE from `Askpass.ps1` | full round-trip |
| UTF-8 read as UTF-16LE | fails, as it must |
| Empty passphrase on stdin | rejected |
| Output format | all four paths begin with the `age` header |
| Envelope closed via UTF-8, opened via UTF-16LE, and the reverse | opens both ways |
| Same, with a 480-character passphrase | opens both ways |
| `--stdin` envelope | opens through the ordinary path |
| `--stdin --stdout` round-trip | returns identical |
| Framed passphrase containing `0x0A` | not split |
| Framed passphrase ending in a newline | preserved exactly |
| Header without a newline, non-numeric, oversized, or truncated | rejected, exit `1` |

The UTF-8/UTF-16LE pairs confirm that both decoding paths derive exactly the same string. If they did not, the envelope would not open.

---

## License

**Apache 2.0** — see [LICENSE](../LICENSE).

Earlier Spanish documentation: [`.old/README-ESP.md`](.old/README-ESP.md).
