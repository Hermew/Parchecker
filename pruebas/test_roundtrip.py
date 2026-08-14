#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Prueba la mecanica de cifrado sin abrir la ventana.

Lo que se valida aca es lo que no se puede ver a ojo: que la clave viaje entera
por el pipe, que el archivo abra con ella, que no abra con otra, y que sin clave
no se pueda ni listar el contenido. La ventana se prueba a mano, una vez.

    python pruebas/test_roundtrip.py
"""

import shutil
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "cifrar"))
import cifrar  # noqa: E402

CLAVES = [
    ("ascii",   "Prueba123!"),
    ("espanol", "Cerrajeria-Nandu-2026"),
    ("acentos", "Cerrajería-Ñandú-2026"),
    ("simbolos", "a b\"c'd\\e|f&g<h>i^j%k"),
    ("largo",   "x" * 120),
]

fallos = []


def check(condicion, texto):
    estado = "ok  " if condicion else "FALLA"
    print("   [%s] %s" % (estado, texto))
    if not condicion:
        fallos.append(texto)


def probar(etiqueta, clave, carpeta):
    print("\n== %s ==" % etiqueta)
    destino = carpeta / ("%s.rar" % etiqueta)
    vacio = carpeta / "vacio.txt"
    vacio.write_bytes(b"")

    cifrar.crear(destino, [vacio], clave)
    check(destino.exists() and destino.stat().st_size > 0, "se creo el .rar")

    ok, _ = cifrar.verificar(destino, clave)
    check(ok, "abre con la clave correcta")

    mal, _ = cifrar.verificar(destino, clave + "x")
    check(not mal, "NO abre con la clave equivocada")

    # Con -hp ni siquiera se puede listar el contenido sin la clave.
    codigo, _ = cifrar.correr_rar(["l", "-y", "-idq", str(destino)], "clave-cualquiera")
    check(codigo != 0, "sin la clave no se puede listar el contenido")


def main():
    carpeta = Path(tempfile.mkdtemp(prefix="pc3r-test-"))
    print("Carpeta de prueba: %s" % carpeta)
    print("Codificacion para rar: %s" % cifrar.CODIGO_ANSI)
    print("Rar.exe: %s" % cifrar.buscar_rar())
    try:
        for etiqueta, clave in CLAVES:
            try:
                probar(etiqueta, clave, carpeta)
            except cifrar.ErrorCifrar as e:
                print("   [FALLA] %s reviento: %s" % (etiqueta, e))
                fallos.append(etiqueta)

        print("\n== nombres visibles (-p en vez de -hp) ==")
        destino = carpeta / "visible.rar"
        cifrar.crear(destino, [carpeta / "vacio.txt"], "Prueba123!", ocultar_nombres=False)
        # -idc y no -idq: el modo silencioso se come tambien el listado.
        codigo, salida = cifrar.correr_rar(["l", "-y", "-idc", str(destino)], "")
        check("vacio.txt" in salida, "con -p el nombre se lista sin clave (comparacion)")
    finally:
        shutil.rmtree(carpeta, ignore_errors=True)

    print("")
    if fallos:
        print("FALLARON %d chequeo(s): %s" % (len(fallos), ", ".join(fallos)))
        return 1
    print("Todo OK.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
