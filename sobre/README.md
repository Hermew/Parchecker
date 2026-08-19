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

## Memory

The passphrase is wiped from every buffer it passes through, not only from the final one.

| Stage | Covered by |
|---|---|
| Raw bytes from stdin | `Zeroizing<Vec<u8>>` |
| 16-bit units, `--utf16le` only | `Zeroizing<Vec<u16>>` |
| Final string | `SecretString` |

A plain `Vec` or `String` is freed without being overwritten, so its contents stay in the heap until something else reuses that memory. The `--utf16le` path matters most: it is the one the askpass uses, and it makes two more copies than the other, converting bytes to `u16` and `u16` to text.

On the UTF-8 path validation uses `str::from_utf8`, which inspects without consuming. `String::from_utf8` would save a copy but returns an error **holding the bytes**, so the passphrase would travel inside the error message.

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

| | |
|---|---|
| Round-trip with accents and `ñ` | returns identical |
| Wrong passphrase | rejected |
| Passphrase as an argument | rejected before anything is read |
| Existing output file | not overwritten without `--forzar` |
| UTF-16LE from `Askpass.ps1` | full round-trip |
| UTF-8 read as UTF-16LE | fails, as it must |
| Empty passphrase on stdin | rejected |
| Output format | file begins with the `age` header |
| Envelope closed via UTF-8, opened via UTF-16LE, and the reverse | opens both ways |
| Same, with a 480-character passphrase | opens both ways |

The last two confirm that both decoding paths derive exactly the same string. If they did not, the envelope would not open.

---

## License

**Apache 2.0** — see [LICENSE](../LICENSE).

Earlier Spanish documentation: [`.old/README-ESP.md`](.old/README-ESP.md).
