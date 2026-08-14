//! sobre - Mete un archivo en un sobre cerrado. La clave entra por stdin y nada mas.
//!
//! Es la version en Rust de la idea de Parchecker, sin depender de WinRAR.
//!
//! Por que existe
//! --------------
//! La version original tuvo que descubrir que `rar.exe -p` acepta la clave por
//! stdin, y despues medir cinco codificaciones para dar con la que espera. Todo
//! ese trabajo fue negociar con un programa ajeno que no fue pensado para esto.
//!
//! Aca no hay con quien negociar: la interfaz la definimos nosotros. La clave
//! entra por stdin, en UTF-8, y listo. El hack deja de ser necesario porque deja
//! de haber un formato ajeno del otro lado.
//!
//! Lo que NO se hace aca
//! ---------------------
//! Criptografia propia. El formato es age (age-encryption.org/v1): ChaCha20-Poly1305
//! con scrypt para derivar la clave de la passphrase. Esta especificado, auditado
//! y lo implementa el crate `age`. Este archivo solo mueve bytes de un lado al
//! otro y se ocupa de que la clave no toque la linea de comandos.

use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::process::ExitCode;

use age::secrecy::SecretString;

const AYUDA: &str = "\
sobre - archivo cifrado con clave que nunca toca la linea de comandos

USO:
    sobre cifrar <entrada> <salida>
    sobre abrir  <entrada> <salida>

La clave se lee de stdin. No hay -p ni --clave: si la clave se pudiera pasar
como argumento, quedaria en la lista de procesos y en el historial del shell,
que es exactamente lo que este programa existe para evitar.

OPCIONES:
    --utf16le    la clave llega en UTF-16LE (es lo que escupe Askpass.ps1)
    --forzar     sobreescribir la salida si ya existe
    -h, --help   esto

EJEMPLOS:
    # con la ventanita de Parchecker
    powershell -File Askpass.ps1 -Confirmar | sobre cifrar --utf16le notas.txt notas.sobre

    # a mano, desde un script
    Get-Content clave.txt | sobre abrir notas.sobre notas.txt
";

struct Opciones {
    orden: String,
    entrada: String,
    salida: String,
    utf16le: bool,
    forzar: bool,
}

fn main() -> ExitCode {
    match correr() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sobre: {e}");
            // Las causas encadenadas cuentan la historia completa: "no pude abrir
            // la entrada" no sirve sin el "porque no existe" de abajo.
            let mut fuente = e.source();
            while let Some(c) = fuente {
                eprintln!("  causa: {c}");
                fuente = c.source();
            }
            ExitCode::from(1)
        }
    }
}

fn correr() -> Result<(), Box<dyn std::error::Error>> {
    let opciones = match parsear()? {
        Some(o) => o,
        None => {
            print!("{AYUDA}");
            return Ok(());
        }
    };

    let clave = leer_clave(opciones.utf16le)?;

    match opciones.orden.as_str() {
        "cifrar" => cifrar(&opciones, clave),
        "abrir" => abrir(&opciones, clave),
        otra => Err(format!("no conozco la orden {otra:?}. Probá con --help").into()),
    }
}

fn parsear() -> Result<Option<Opciones>, Box<dyn std::error::Error>> {
    let mut sueltos: Vec<String> = Vec::new();
    let mut utf16le = false;
    let mut forzar = false;

    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "--utf16le" => utf16le = true,
            "--forzar" => forzar = true,
            // Cortar de raiz cualquier intento de pasar la clave por argumento.
            a if a.starts_with("-p") || a.starts_with("--clave") || a.starts_with("--password") => {
                return Err(
                    "la clave no se pasa por argumento, nunca. Mandala por stdin: \
                     los argumentos de un proceso los puede leer cualquier otro proceso."
                        .into(),
                )
            }
            a if a.starts_with('-') => return Err(format!("no conozco la opcion {a:?}").into()),
            _ => sueltos.push(arg),
        }
    }

    if sueltos.is_empty() {
        return Ok(None);
    }
    if sueltos.len() != 3 {
        return Err(format!(
            "esperaba: sobre <orden> <entrada> <salida>. Me pasaste {} cosa(s).",
            sueltos.len()
        )
        .into());
    }

    Ok(Some(Opciones {
        orden: sueltos[0].clone(),
        entrada: sueltos[1].clone(),
        salida: sueltos[2].clone(),
        utf16le,
        forzar,
    }))
}

/// Lee la clave de stdin. Devuelve un SecretString, que se borra solo al morir.
///
/// Esto es lo que Python no podia hacer: alla un `str` es inmutable y queda en el
/// heap hasta que el recolector se digne. Aca `SecretString` sobreescribe con
/// ceros cuando sale de alcance, sin que haya que acordarse.
fn leer_clave(utf16le: bool) -> Result<SecretString, Box<dyn std::error::Error>> {
    let mut crudo = Vec::new();
    io::stdin()
        .read_to_end(&mut crudo)
        .map_err(|e| format!("no pude leer la clave de stdin: {e}"))?;

    let mut texto = if utf16le {
        if crudo.len() % 2 != 0 {
            return Err("la clave en UTF-16LE tiene un numero impar de bytes".into());
        }
        let unidades: Vec<u16> = crudo
            .chunks_exact(2)
            .map(|par| u16::from_le_bytes([par[0], par[1]]))
            .collect();
        String::from_utf16(&unidades).map_err(|_| "la clave no es UTF-16LE valido")?
    } else {
        String::from_utf8(crudo).map_err(|_| "la clave no es UTF-8 valido")?
    };

    // Los saltos del final son del pipe, no de la clave. Adentro se respetan.
    while texto.ends_with('\n') || texto.ends_with('\r') {
        texto.pop();
    }

    if texto.is_empty() {
        return Err("no llego ninguna clave por stdin".into());
    }

    Ok(SecretString::from(texto))
}

fn abrir_salida(opciones: &Opciones) -> Result<File, Box<dyn std::error::Error>> {
    let destino = Path::new(&opciones.salida);
    if destino.exists() && !opciones.forzar {
        return Err(format!(
            "{} ya existe. Usa --forzar si de verdad lo querés pisar.",
            opciones.salida
        )
        .into());
    }
    File::create(destino).map_err(|e| format!("no pude crear {}: {e}", opciones.salida).into())
}

fn cifrar(opciones: &Opciones, clave: SecretString) -> Result<(), Box<dyn std::error::Error>> {
    let mut entrada = BufReader::new(
        File::open(&opciones.entrada)
            .map_err(|e| format!("no pude abrir {}: {e}", opciones.entrada))?,
    );
    let salida = BufWriter::new(abrir_salida(opciones)?);

    let cifrador = age::Encryptor::with_user_passphrase(clave);
    let mut flujo = cifrador.wrap_output(salida)?;
    io::copy(&mut entrada, &mut flujo)?;
    // Sin este finish el archivo queda truncado y no abre nunca mas. Es el mismo
    // tipo de error silencioso que mandarle la clave dos veces a rar.
    flujo.finish()?.flush()?;

    let bytes = std::fs::metadata(&opciones.salida)?.len();
    eprintln!("cerrado: {} ({} bytes)", opciones.salida, bytes);
    Ok(())
}

fn abrir(opciones: &Opciones, clave: SecretString) -> Result<(), Box<dyn std::error::Error>> {
    let entrada = BufReader::new(
        File::open(&opciones.entrada)
            .map_err(|e| format!("no pude abrir {}: {e}", opciones.entrada))?,
    );

    let descifrador = age::Decryptor::new_buffered(entrada)
        .map_err(|e| format!("{} no parece un sobre valido: {e}", opciones.entrada))?;

    let identidad = age::scrypt::Identity::new(clave);
    let mut flujo = descifrador
        .decrypt(std::iter::once(&identidad as &dyn age::Identity))
        .map_err(|e| format!("no pude abrirlo: {e}"))?;

    let mut salida = BufWriter::new(abrir_salida(opciones)?);
    io::copy(&mut flujo, &mut salida)?;
    salida.flush()?;

    eprintln!("abierto: {}", opciones.salida);
    Ok(())
}
