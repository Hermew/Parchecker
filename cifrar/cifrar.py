#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
cifrar.py - Arma un .rar cifrado con una clave que nunca toca la linea de comandos.

La clave se pide con askpass/Askpass.ps1 (una ventana de Windows) y se le entrega
a rar.exe por un pipe de stdin. No pasa por el chat, ni por el historial del
shell, ni por la lista de procesos, ni por un archivo temporal.

Uso:
    python cifrar.py backup.rar carpeta/ archivo.txt
    python cifrar.py prueba.rar --vacio
    python cifrar.py backup.rar carpeta/ --verificar

Codigos de salida:
    0  listo
    1  el usuario cancelo la ventana
    2  error de uso
    3  rar fallo
"""

import argparse
import ctypes
import os
import subprocess
import sys
import tempfile
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent

# Dos implementaciones del mismo contrato: piden un secreto, lo escriben por
# stdout como UTF-16LE y salen con 0/1/2. Son intercambiables, y esa es la
# prueba de que el askpass esta bien separado de quien lo usa.
ASKPASS = {
    "ventana": RAIZ / "askpass" / "Askpass.ps1",
    "consola": RAIZ / "askpass" / "AskpassConsola.ps1",
}

# rar.exe lee la clave de stdin y la interpreta con la codepage ANSI del sistema
# (ACP), no con la de la consola. Verificado creando un archivo con la clave por
# argv -que Windows entrega como Unicode, igual que la GUI- y probando cual
# codificacion de stdin lo abria: cp1252 si, cp850/cp437/utf-8/utf-16 no.
# Si esto no coincide, una clave con enie o tilde genera un .rar que la GUI de
# WinRAR no puede abrir. Por eso se usa la ACP real y no una constante fija.
CODIGO_ANSI = "cp%d" % ctypes.windll.kernel32.GetACP()


class ErrorCifrar(Exception):
    pass


def buscar_rar():
    candidatos = [
        Path(os.environ.get("ProgramFiles", r"C:\Program Files")) / "WinRAR" / "Rar.exe",
        Path(os.environ.get("ProgramFiles(x86)", r"C:\Program Files (x86)")) / "WinRAR" / "Rar.exe",
    ]
    for c in candidatos:
        if c.exists():
            return c
    from shutil import which
    hallado = which("rar") or which("Rar.exe")
    if hallado:
        return Path(hallado)
    raise ErrorCifrar(
        "No encontre Rar.exe. Instala WinRAR o agrega su carpeta al PATH.\n"
        "Busque en: %s" % ", ".join(str(c) for c in candidatos)
    )


def buscar_powershell():
    ruta = Path(os.environ.get("SystemRoot", r"C:\Windows")) / "System32" / "WindowsPowerShell" / "v1.0" / "powershell.exe"
    if ruta.exists():
        return ruta
    from shutil import which
    hallado = which("powershell")
    if not hallado:
        raise ErrorCifrar("No encontre powershell.exe, que es quien dibuja la ventana.")
    return Path(hallado)


def pedir_clave(titulo, mensaje, confirmar=False, interfaz="ventana"):
    """Pide la clave y la devuelve. None si el usuario cancelo.

    El secreto sale del script de PowerShell como UTF-16LE crudo por stdout, se
    decodifica aca y no se escribe a ningun lado. Python no puede sobreescribir
    un str -son inmutables-, y por eso la parte sensible vive en PowerShell, que
    si libera el buffer con ZeroFreeBSTR. Aca la clave existe por milisegundos.

    Solo se captura stdout. stderr se deja pasar a la terminal a proposito: es
    por donde la version de consola dibuja su ventana. Capturarlo la volveria
    invisible.
    """
    guion = ASKPASS[interfaz]
    if not guion.exists():
        raise ErrorCifrar("Falta %s" % guion)

    orden = [
        str(buscar_powershell()),
        "-NoProfile", "-STA",
        "-ExecutionPolicy", "Bypass",
        "-File", str(guion),
        "-Titulo", titulo,
        "-Mensaje", mensaje,
    ]
    if confirmar:
        orden.append("-Confirmar")

    # Sin shell: los argumentos van directo al proceso, no los expande nadie.
    proceso = subprocess.run(orden, stdout=subprocess.PIPE)

    if proceso.returncode == 1:
        return None
    if proceso.returncode != 0:
        raise ErrorCifrar(
            "El pedido de contrasena fallo (codigo %d). El detalle salio recien por pantalla."
            % proceso.returncode
        )

    return proceso.stdout.decode("utf-16-le")


def a_bytes_para_rar(clave):
    """Codifica la clave como la espera rar.exe, o explica por que no se puede."""
    try:
        return bytearray(clave.encode(CODIGO_ANSI)) + b"\r\n"
    except UnicodeEncodeError as e:
        malos = "".join(sorted(set(clave[e.start:e.end])))
        raise ErrorCifrar(
            "La contrasena tiene caracteres que rar.exe no puede recibir en este\n"
            "Windows (%s): %r\n"
            "Usa letras, numeros, simbolos comunes y acentos del espanol."
            % (CODIGO_ANSI, malos)
        )


def correr_rar(args, clave, cwd=None):
    """Llama a rar.exe entregandole la clave por stdin. Devuelve (codigo, salida)."""
    rar = buscar_rar()
    datos = a_bytes_para_rar(clave)
    try:
        proceso = subprocess.Popen(
            [str(rar)] + args,
            cwd=str(cwd) if cwd else None,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        # Una sola vez. Probado: mandarla dos veces hace que rar tome las dos
        # lineas como una sola clave y despues el archivo no abre con ninguna.
        salida, _ = proceso.communicate(bytes(datos), timeout=300)
    finally:
        for i in range(len(datos)):
            datos[i] = 0
    return proceso.returncode, salida.decode(CODIGO_ANSI, "replace")


def crear(destino, entradas, clave, ocultar_nombres=True):
    destino = Path(destino).resolve()
    destino.parent.mkdir(parents=True, exist_ok=True)

    # -hp cifra tambien las cabeceras: sin la clave no se puede ni listar que hay
    # adentro. -p deja los nombres a la vista.
    modificador = "-hp" if ocultar_nombres else "-p"
    args = [
        "a", modificador,
        "-y",       # no preguntar nada por consola
        "-idq",     # modo silencioso
        "-ep1",     # no guardar la ruta completa dentro del archivo
        str(destino),
    ] + [str(e) for e in entradas]

    codigo, salida = correr_rar(args, clave)
    if codigo != 0 or not destino.exists():
        raise ErrorCifrar("rar fallo al crear (codigo %d):\n%s" % (codigo, salida.strip()))
    return destino


def verificar(destino, clave):
    """rar t con la clave: confirma que el archivo abre con lo que se tipeo."""
    codigo, salida = correr_rar(["t", "-p", "-y", "-idq", str(destino)], clave)
    return codigo == 0, salida.strip()


def main():
    p = argparse.ArgumentParser(
        prog="cifrar.py",
        description="Arma un .rar cifrado pidiendo la clave en una ventana de Windows.",
    )
    p.add_argument("destino", help="el .rar a crear")
    p.add_argument("entradas", nargs="*", help="archivos o carpetas a meter adentro")
    p.add_argument("--vacio", action="store_true",
                   help="crear el archivo con un solo archivo vacio adentro (para probar)")
    p.add_argument("--verificar", action="store_true",
                   help="despues de crear, abrirlo con la misma clave para confirmar")
    p.add_argument("--mostrar-nombres", action="store_true",
                   help="usar -p en vez de -hp: la lista de archivos queda visible sin clave")
    p.add_argument("--consola", action="store_true",
                   help="pedir la clave dibujando la ventana en la terminal, sin abrir nada")
    args = p.parse_args()

    if not args.entradas and not args.vacio:
        p.error("dame algo para meter adentro, o usa --vacio")

    temporal = None
    entradas = list(args.entradas)
    try:
        if args.vacio:
            temporal = tempfile.mkdtemp(prefix="pc3r-")
            vacio = Path(temporal) / "vacio.txt"
            vacio.write_bytes(b"")
            entradas.append(str(vacio))

        clave = pedir_clave(
            titulo="Cifrar %s" % Path(args.destino).name,
            mensaje="Contrasena para el archivo:",
            confirmar=True,
            interfaz="consola" if args.consola else "ventana",
        )
        if clave is None:
            print("Cancelado. No se creo nada.", file=sys.stderr)
            return 1

        destino = crear(args.destino, entradas, clave,
                        ocultar_nombres=not args.mostrar_nombres)
        print("Creado: %s (%d bytes)" % (destino, destino.stat().st_size))

        if args.verificar:
            ok, detalle = verificar(destino, clave)
            if ok:
                print("Verificado: abre con la contrasena que escribiste.")
            else:
                print("OJO: no pude abrirlo con esa contrasena.\n%s" % detalle, file=sys.stderr)
                return 3

        if not args.mostrar_nombres:
            print("Los nombres de los archivos tambien estan cifrados (-hp).")
        return 0
    finally:
        if temporal:
            from shutil import rmtree
            rmtree(temporal, ignore_errors=True)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except ErrorCifrar as e:
        print("\n%s" % e, file=sys.stderr)
        sys.exit(3)
    except KeyboardInterrupt:
        print("\nCancelado.", file=sys.stderr)
        sys.exit(1)
