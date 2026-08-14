# Dependencias de terceros

`sobre` compila estas librerias adentro del binario. Este archivo se genera
desde `cargo metadata`; no se escribe a mano.

## Resumen

| Licencia | Paquetes |
|---|--:|
| `MIT OR Apache-2.0` | 79 |
| `Apache-2.0 OR MIT` | 31 |
| `MIT` | 15 |
| `MIT/Apache-2.0` | 7 |
| `BSD-3-Clause` | 3 |
| `Unicode-3.0` | 3 |
| `Unlicense OR MIT` | 2 |
| `Unlicense/MIT` | 2 |
| `BSD-2-Clause OR Apache-2.0 OR MIT` | 2 |
| `MIT OR Apache-2.0 OR BSD-1-Clause` | 1 |
| `Apache-2.0/MIT` | 1 |
| `Apache-2.0 OR GPL-2.0-only` | 1 |
| `(MIT OR Apache-2.0) AND Unicode-3.0` | 1 |
| `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | 1 |

Todas permisivas y compatibles con Apache-2.0.

**Una merece aclaración:** `self_cell` declara `Apache-2.0 OR GPL-2.0-only`. El
`OR` significa que quien la usa **elige** bajo cuál licencia la toma, no que
apliquen las dos. Acá se elige **Apache-2.0**, así que no entra nada de copyleft
al binario. Está anotado porque una búsqueda automática de "GPL" la va a marcar,
y conviene tener la respuesta a mano en vez de tener que averiguarla dos veces.

## Detalle

| Paquete | Version | Licencia |
|---|---|---|
| `aead` | 0.5.2 | MIT OR Apache-2.0 |
| `aes` | 0.8.4 | MIT OR Apache-2.0 |
| `aes-gcm` | 0.10.3 | Apache-2.0 OR MIT |
| `age` | 0.12.1 | MIT OR Apache-2.0 |
| `age-core` | 0.12.0 | MIT OR Apache-2.0 |
| `arc-swap` | 1.9.2 | MIT OR Apache-2.0 |
| `base16ct` | 0.2.0 | Apache-2.0 OR MIT |
| `base64` | 0.22.1 | MIT OR Apache-2.0 |
| `basic-toml` | 0.1.10 | MIT OR Apache-2.0 |
| `bech32` | 0.11.1 | MIT |
| `bitflags` | 2.13.1 | MIT OR Apache-2.0 |
| `block-buffer` | 0.10.4 | MIT OR Apache-2.0 |
| `block-buffer` | 0.12.1 | MIT OR Apache-2.0 |
| `cfg-if` | 1.0.4 | MIT OR Apache-2.0 |
| `chacha20` | 0.9.1 | Apache-2.0 OR MIT |
| `chacha20poly1305` | 0.10.1 | Apache-2.0 OR MIT |
| `cipher` | 0.4.4 | MIT OR Apache-2.0 |
| `const-oid` | 0.9.6 | Apache-2.0 OR MIT |
| `const-oid` | 0.10.2 | Apache-2.0 OR MIT |
| `cookie-factory` | 0.3.3 | MIT |
| `cpufeatures` | 0.2.17 | MIT OR Apache-2.0 |
| `cpufeatures` | 0.3.0 | MIT OR Apache-2.0 |
| `crypto-bigint` | 0.5.5 | Apache-2.0 OR MIT |
| `crypto-common` | 0.1.7 | MIT OR Apache-2.0 |
| `crypto-common` | 0.2.2 | MIT OR Apache-2.0 |
| `ctr` | 0.9.2 | MIT OR Apache-2.0 |
| `curve25519-dalek` | 4.1.3 | BSD-3-Clause |
| `curve25519-dalek-derive` | 0.1.1 | MIT/Apache-2.0 |
| `der` | 0.7.10 | Apache-2.0 OR MIT |
| `digest` | 0.10.7 | MIT OR Apache-2.0 |
| `digest` | 0.11.3 | MIT OR Apache-2.0 |
| `displaydoc` | 0.2.7 | MIT OR Apache-2.0 |
| `elliptic-curve` | 0.13.8 | Apache-2.0 OR MIT |
| `ff` | 0.13.1 | MIT/Apache-2.0 |
| `fiat-crypto` | 0.2.9 | MIT OR Apache-2.0 OR BSD-1-Clause |
| `find-crate` | 0.6.3 | Apache-2.0 OR MIT |
| `fluent` | 0.17.0 | Apache-2.0 OR MIT |
| `fluent-bundle` | 0.16.0 | Apache-2.0 OR MIT |
| `fluent-langneg` | 0.13.1 | Apache-2.0 OR MIT |
| `fluent-syntax` | 0.12.0 | Apache-2.0 OR MIT |
| `futures` | 0.3.34 | MIT OR Apache-2.0 |
| `futures-channel` | 0.3.34 | MIT OR Apache-2.0 |
| `futures-core` | 0.3.34 | MIT OR Apache-2.0 |
| `futures-executor` | 0.3.34 | MIT OR Apache-2.0 |
| `futures-io` | 0.3.34 | MIT OR Apache-2.0 |
| `futures-macro` | 0.3.34 | MIT OR Apache-2.0 |
| `futures-sink` | 0.3.34 | MIT OR Apache-2.0 |
| `futures-task` | 0.3.34 | MIT OR Apache-2.0 |
| `futures-util` | 0.3.34 | MIT OR Apache-2.0 |
| `generic-array` | 0.14.7 | MIT |
| `getrandom` | 0.2.17 | MIT OR Apache-2.0 |
| `ghash` | 0.5.1 | Apache-2.0 OR MIT |
| `group` | 0.13.0 | MIT/Apache-2.0 |
| `hkdf` | 0.12.4 | MIT OR Apache-2.0 |
| `hmac` | 0.12.1 | MIT OR Apache-2.0 |
| `hpke` | 0.12.0 | MIT/Apache-2.0 |
| `hybrid-array` | 0.2.3 | MIT OR Apache-2.0 |
| `hybrid-array` | 0.4.14 | MIT OR Apache-2.0 |
| `i18n-config` | 0.4.8 | MIT |
| `i18n-embed` | 0.16.0 | MIT |
| `i18n-embed-fl` | 0.10.1 | MIT |
| `i18n-embed-impl` | 0.8.4 | MIT |
| `inout` | 0.1.4 | MIT OR Apache-2.0 |
| `intl-memoizer` | 0.5.3 | Apache-2.0 OR MIT |
| `intl_pluralrules` | 7.0.2 | Apache-2.0/MIT |
| `io_tee` | 0.1.1 | MIT OR Apache-2.0 |
| `keccak` | 0.1.6 | Apache-2.0 OR MIT |
| `kem` | 0.3.0-pre.0 | Apache-2.0 OR MIT |
| `lazy_static` | 1.5.0 | MIT OR Apache-2.0 |
| `libc` | 0.2.189 | MIT OR Apache-2.0 |
| `lock_api` | 0.4.14 | MIT OR Apache-2.0 |
| `log` | 0.4.33 | MIT OR Apache-2.0 |
| `memchr` | 2.8.3 | Unlicense OR MIT |
| `mime` | 0.3.17 | MIT OR Apache-2.0 |
| `mime_guess` | 2.0.5 | MIT |
| `ml-kem` | 0.2.3 | Apache-2.0 OR MIT |
| `nom` | 8.0.0 | MIT |
| `opaque-debug` | 0.3.1 | MIT OR Apache-2.0 |
| `p256` | 0.13.2 | Apache-2.0 OR MIT |
| `parking_lot` | 0.12.5 | MIT OR Apache-2.0 |
| `parking_lot_core` | 0.9.12 | MIT OR Apache-2.0 |
| `pbkdf2` | 0.12.2 | MIT OR Apache-2.0 |
| `pin-project` | 1.1.13 | Apache-2.0 OR MIT |
| `pin-project-internal` | 1.1.13 | Apache-2.0 OR MIT |
| `pin-project-lite` | 0.2.17 | Apache-2.0 OR MIT |
| `poly1305` | 0.8.0 | Apache-2.0 OR MIT |
| `polyval` | 0.6.2 | Apache-2.0 OR MIT |
| `ppv-lite86` | 0.2.21 | MIT OR Apache-2.0 |
| `primeorder` | 0.13.6 | Apache-2.0 OR MIT |
| `proc-macro-error-attr3` | 3.1.0 | MIT OR Apache-2.0 |
| `proc-macro-error3` | 3.1.0 | MIT OR Apache-2.0 |
| `proc-macro2` | 1.0.107 | MIT OR Apache-2.0 |
| `quote` | 1.0.47 | MIT OR Apache-2.0 |
| `rand` | 0.8.7 | MIT OR Apache-2.0 |
| `rand_chacha` | 0.3.1 | MIT OR Apache-2.0 |
| `rand_core` | 0.6.4 | MIT OR Apache-2.0 |
| `redox_syscall` | 0.5.18 | MIT |
| `rust-embed` | 8.12.0 | MIT |
| `rust-embed-impl` | 8.12.0 | MIT |
| `rust-embed-utils` | 8.12.0 | MIT |
| `rustc-hash` | 2.1.3 | Apache-2.0 OR MIT |
| `rustc_version` | 0.4.1 | MIT OR Apache-2.0 |
| `rustversion` | 1.0.23 | MIT OR Apache-2.0 |
| `salsa20` | 0.10.2 | MIT OR Apache-2.0 |
| `same-file` | 1.0.6 | Unlicense/MIT |
| `scopeguard` | 1.2.0 | MIT OR Apache-2.0 |
| `scrypt` | 0.11.0 | MIT OR Apache-2.0 |
| `sec1` | 0.7.3 | Apache-2.0 OR MIT |
| `secrecy` | 0.10.3 | Apache-2.0 OR MIT |
| `self_cell` | 1.3.0 | Apache-2.0 OR GPL-2.0-only |
| `semver` | 1.0.28 | MIT OR Apache-2.0 |
| `serde` | 1.0.229 | MIT OR Apache-2.0 |
| `serde_core` | 1.0.229 | MIT OR Apache-2.0 |
| `serde_derive` | 1.0.229 | MIT OR Apache-2.0 |
| `sha2` | 0.10.9 | MIT OR Apache-2.0 |
| `sha2` | 0.11.0 | MIT OR Apache-2.0 |
| `sha3` | 0.10.9 | MIT OR Apache-2.0 |
| `slab` | 0.4.12 | MIT |
| `smallvec` | 1.15.2 | MIT OR Apache-2.0 |
| `strsim` | 0.11.1 | MIT |
| `subtle` | 2.6.1 | BSD-3-Clause |
| `syn` | 2.0.119 | MIT OR Apache-2.0 |
| `syn` | 3.0.3 | MIT OR Apache-2.0 |
| `thiserror` | 1.0.69 | MIT OR Apache-2.0 |
| `thiserror` | 2.0.20 | MIT OR Apache-2.0 |
| `thiserror-impl` | 1.0.69 | MIT OR Apache-2.0 |
| `thiserror-impl` | 2.0.20 | MIT OR Apache-2.0 |
| `tinystr` | 0.8.4 | Unicode-3.0 |
| `toml` | 0.5.11 | MIT/Apache-2.0 |
| `type-map` | 0.5.1 | MIT/Apache-2.0 |
| `typenum` | 1.20.1 | MIT OR Apache-2.0 |
| `unic-langid` | 0.9.6 | MIT OR Apache-2.0 |
| `unic-langid-impl` | 0.9.6 | MIT OR Apache-2.0 |
| `unicase` | 2.9.0 | MIT OR Apache-2.0 |
| `unicode-ident` | 1.0.24 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| `universal-hash` | 0.5.1 | MIT OR Apache-2.0 |
| `version_check` | 0.9.5 | MIT/Apache-2.0 |
| `walkdir` | 2.5.0 | Unlicense/MIT |
| `wasi` | 0.11.1+wasi-snapshot-preview1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| `winapi-util` | 0.1.11 | Unlicense OR MIT |
| `windows-link` | 0.2.1 | MIT OR Apache-2.0 |
| `windows-sys` | 0.61.2 | MIT OR Apache-2.0 |
| `x25519-dalek` | 2.0.1 | BSD-3-Clause |
| `zerocopy` | 0.8.56 | BSD-2-Clause OR Apache-2.0 OR MIT |
| `zerocopy-derive` | 0.8.56 | BSD-2-Clause OR Apache-2.0 OR MIT |
| `zerofrom` | 0.1.8 | Unicode-3.0 |
| `zeroize` | 1.9.0 | Apache-2.0 OR MIT |
| `zeroize_derive` | 1.5.0 | Apache-2.0 OR MIT |
| `zerovec` | 0.11.7 | Unicode-3.0 |
