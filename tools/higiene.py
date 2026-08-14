#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
higiene.py - Busca caracteres invisibles en los archivos de texto del repo.

Por que existe
--------------
Zero-width, marcas bidi, espacios raros y tags Unicode son invisibles en el
editor pero cambian los bytes. Rompen diffs, hacen que `grep` no encuentre lo que
esta ahi, y a veces revientan el parser. En un repo publico, ademas, es la clase
de cosa que alguien va a mirar con lupa. Se corre solo en cada commit.

    python tools/higiene.py .            revisa y falla si encuentra algo
    python tools/higiene.py . --arreglar los saca y reescribe los archivos

Codigos de salida: 0 limpio, 1 encontro algo, 2 error de uso.
"""

import argparse
import sys
import unicodedata
from pathlib import Path

# Lo que se busca. El nombre es el que se le muestra al usuario.
INVISIBLES = {
    0x200B: "ZERO WIDTH SPACE",
    0x200C: "ZERO WIDTH NON-JOINER",
    0x200D: "ZERO WIDTH JOINER",
    0x2060: "WORD JOINER",
    0xFEFF: "BOM / ZERO WIDTH NO-BREAK SPACE",
    0x00AD: "SOFT HYPHEN",
    0x00A0: "NO-BREAK SPACE",
    0x202F: "NARROW NO-BREAK SPACE",
    0x2007: "FIGURE SPACE",
    0x2009: "THIN SPACE",
    0x200A: "HAIR SPACE",
    0x2028: "LINE SEPARATOR",
    0x2029: "PARAGRAPH SEPARATOR",
    0x200E: "LEFT-TO-RIGHT MARK",
    0x200F: "RIGHT-TO-LEFT MARK",
    0x061C: "ARABIC LETTER MARK",
    0x2062: "INVISIBLE TIMES",
    0x2063: "INVISIBLE SEPARATOR",
    0x2064: "INVISIBLE PLUS",
}

# U+E0000-E007F: los "tags". Son el vector clasico para esconder texto adentro de
# otro texto, porque no se ven en ningun lado.
TAGS = range(0xE0000, 0xE0080)

EXTENSIONES = {
    ".py", ".ps1", ".psm1", ".cmd", ".bat", ".md", ".txt", ".json", ".yml",
    ".yaml", ".toml", ".cfg", ".ini", ".gitignore", ".sh", ".xml", ".html", ".css", ".js",
}

SALTAR = {".git", "__pycache__", ".venv", "node_modules"}


def es_texto(ruta):
    return ruta.suffix.lower() in EXTENSIONES or ruta.name in (".gitignore", "LICENSE")


def bom_permitido(ruta):
    """PowerShell 5.1 lee los .ps1 como ANSI si no tienen BOM y rompe los acentos.
    Ahi el BOM no es basura: es obligatorio. En cualquier otro lado, sobra."""
    return ruta.suffix.lower() in (".ps1", ".psm1")


def revisar(ruta):
    """Devuelve [(linea, columna, codigo, nombre)] de lo que encontro."""
    crudo = ruta.read_bytes()
    tiene_bom = crudo[:3] == b"\xef\xbb\xbf"
    texto = crudo.decode("utf-8", "replace")

    hallazgos = []
    inicio = 0
    if tiene_bom and bom_permitido(ruta):
        inicio = 1  # el BOM del principio esta permitido; uno en el medio no

    for i, ch in enumerate(texto):
        if i < inicio:
            continue
        codigo = ord(ch)
        nombre = None
        if codigo in INVISIBLES:
            nombre = INVISIBLES[codigo]
        elif codigo in TAGS:
            nombre = "UNICODE TAG (texto escondido)"
        elif unicodedata.category(ch) == "Cf":
            nombre = unicodedata.name(ch, "FORMATO INVISIBLE")
        elif 0xFE00 <= codigo <= 0xFE0F:
            # Los selectores de variacion son legitimos pegados a un emoji
            # (hacen que se dibuje en color). Sueltos, no.
            anterior = texto[i - 1] if i else ""
            if anterior and ord(anterior) > 0x2000:
                continue
            nombre = "VARIATION SELECTOR suelto"

        if nombre:
            linea = texto.count("\n", 0, i) + 1
            columna = i - (texto.rfind("\n", 0, i) + 1) + 1
            hallazgos.append((linea, columna, codigo, nombre))
    return hallazgos


def limpiar(ruta):
    crudo = ruta.read_bytes()
    texto = crudo.decode("utf-8", "replace")
    prefijo = ""
    if crudo[:3] == b"\xef\xbb\xbf" and bom_permitido(ruta):
        prefijo, texto = texto[0], texto[1:]

    salida = []
    for i, ch in enumerate(texto):
        codigo = ord(ch)
        if codigo == 0x00A0 or codigo in (0x2007, 0x2009, 0x200A, 0x202F):
            salida.append(" ")  # los espacios raros se vuelven un espacio normal
            continue
        if codigo in INVISIBLES or codigo in TAGS or unicodedata.category(ch) == "Cf":
            continue
        if 0xFE00 <= codigo <= 0xFE0F:
            anterior = texto[i - 1] if i else ""
            if not (anterior and ord(anterior) > 0x2000):
                continue
        salida.append(ch)

    ruta.write_bytes((prefijo + "".join(salida)).encode("utf-8"))


def main():
    p = argparse.ArgumentParser(prog="higiene.py", description="Caza caracteres invisibles.")
    p.add_argument("ruta", nargs="?", default=".", help="archivo o carpeta (por defecto: .)")
    p.add_argument("--arreglar", action="store_true", help="sacarlos en vez de solo avisar")
    args = p.parse_args()

    raiz = Path(args.ruta)
    if not raiz.exists():
        print("No existe %s" % raiz, file=sys.stderr)
        return 2

    archivos = [raiz] if raiz.is_file() else [
        f for f in sorted(raiz.rglob("*"))
        if f.is_file() and es_texto(f) and not any(s in f.parts for s in SALTAR)
    ]

    total = 0
    for f in archivos:
        hallazgos = revisar(f)
        if not hallazgos:
            continue
        total += len(hallazgos)
        rel = f.relative_to(raiz) if raiz.is_dir() else f.name
        for linea, columna, codigo, nombre in hallazgos:
            print("%s:%d:%d  U+%04X  %s" % (rel, linea, columna, codigo, nombre))
        if args.arreglar:
            limpiar(f)
            print("  -> limpiado")

    print("\n%d archivo(s) revisado(s)." % len(archivos))
    if not total:
        print("Limpio: ni un caracter invisible.")
        return 0
    if args.arreglar:
        print("Se sacaron %d caracter(es)." % total)
        return 0
    print("Encontrados %d caracter(es) invisible(s). Corre con --arreglar para sacarlos." % total)
    return 1


if __name__ == "__main__":
    sys.exit(main())
