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
//!
//! Lo que este programa no te obliga a hacer
//! -----------------------------------------
//! **Escribir el secreto en claro.** Con `--stdin` y `--stdout` el contenido
//! entra y sale por el mismo cano que la clave, asi que un secreto que solo
//! existe en memoria no tiene que aterrizar en disco para poder ser cifrado.
//! Cuidar la clave en tres buffers y despues obligar a pasar el contenido por el
//! filesystem seria cerrar la puerta y dejar la ventana abierta.
//!
//! **Adivinar por que fallo.** El codigo de salida separa "la clave no es esa"
//! (2) de "esto no es un sobre" (3) del resto (1). Quien envuelve a `sobre`
//! decide si volver a pedir la clave o rendirse sin parsear el texto del error,
//! que es la parte que no promete quedarse quieta entre versiones.
//!
//! **Abrir Cargo.toml para saber que version corre.** `--version` contesta.

use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::process::ExitCode;

// El trait se importa sin nombre (`as _`) para poder llamar a `.source()` sin
// que `std::error::Error` choque con nuestro `Error` de mas abajo.
use std::error::Error as _;

use age::secrecy::zeroize::Zeroizing;
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
    --utf16le      la clave llega en UTF-16LE (es lo que escupe Askpass.ps1)
    --forzar       sobreescribir la salida si ya existe
    --stdin        la entrada llega por stdin, atras de la clave (ver EL MARCO)
    --stdout       el resultado sale por stdout en vez de a un archivo
    -h, --help     esto
    -V, --version  numero de version y nada mas

Cada flag de flujo saca su argumento posicional:

    sobre cifrar --stdin <salida>
    sobre abrir  --stdout <entrada>
    sobre cifrar --stdin --stdout

EL MARCO (--stdin):
    Con --stdin la clave y el contenido comparten el mismo stdin, asi que hay
    que decir donde termina una y empieza el otro:

        <largo de la clave en bytes, ASCII decimal> \\n <clave> <contenido>

    El largo es exacto: a la clave enmarcada no se le saca ningun salto de
    linea, porque vos ya dijiste cuantos bytes son. Sin marco (el modo de
    siempre) stdin entero es la clave y si se le sacan los saltos del final,
    que son del pipe y no de la clave.

    Se eligio el largo explicito y no un separador porque en UTF-16LE un
    caracter cualquiera puede tener 0x0A como byte bajo: cortar en el primer
    salto de linea partiria la clave al medio sin avisar.

CODIGOS DE SALIDA:
    0   salio bien
    1   error de uso, de disco, de permisos
    2   la clave no abre ese sobre
    3   la entrada no es un sobre age que se pueda abrir con clave

EJEMPLOS:
    # con la ventanita de Parchecker
    powershell -File Askpass.ps1 -Confirmar | sobre cifrar --utf16le notas.txt notas.sobre

    # a mano, desde un script
    Get-Content clave.txt | sobre abrir notas.sobre notas.txt

    # un secreto que nunca aterriza en claro: 13 bytes de clave y atras el resto
    printf '13\\nclave-secretalo que sea' | sobre cifrar --stdin notas.sobre

    # sobre como filtro, sin tocar disco de ningun lado
    printf '13\\nclave-secreta' | sobre abrir --stdout notas.sobre
";

/// Cuanto puede medir la cabecera de `--stdin`, en bytes.
///
/// `\"1048576\\n\"` son ocho. Veinte es holgado, y el tope existe porque sin el
/// `read_until` se quedaria leyendo para siempre buscando un salto de linea que
/// nunca va a llegar si de arriba mandan cualquier cosa.
const CABECERA_MAX: u64 = 20;

/// Cuanto puede medir la clave enmarcada, en bytes.
///
/// Una passphrase de 64 KiB no existe. El tope esta para que una cabecera
/// mentirosa (`\"4000000000\\n\"`) no reserve cuatro gigas antes de fallar.
const CLAVE_MAX: usize = 64 * 1024;

// Los codigos de salida son la unica parte de este programa que otro programa
// puede leer sin parsear texto. Por eso son constantes con nombre: un
// `ExitCode::from(3)` suelto en medio del codigo no le dice nada a nadie.
const SALIDA_ERROR: u8 = 1;
const SALIDA_CLAVE: u8 = 2;
const SALIDA_NO_ES_SOBRE: u8 = 3;

// ---------------------------------------------------------------------------
// Errores
// ---------------------------------------------------------------------------

/// El error de este programa.
///
/// Un `Box<dyn Error>` alcanza para imprimir un mensaje y para nada mas: es una
/// caja opaca, asi que `main` sabe que algo fallo pero no que. Y "algo fallo" no
/// le sirve a un script, porque no alcanza para elegir entre volver a pedir la
/// clave y rendirse porque el archivo no era un sobre.
///
/// El enum obliga a nombrar de antemano los casos que vale la pena distinguir.
/// El resto vive en `Otro`, sin ceremonia: la idea no es tipar cada error del
/// programa, es tipar los dos que alguien de afuera necesita mirar.
#[derive(Debug)]
enum Error {
    /// El sobre esta bien; la clave no es esa. Sale con 2.
    ClaveIncorrecta {
        origen: String,
        causa: age::DecryptError,
    },
    /// Estos bytes no son un sobre age que se pueda abrir con clave. Sale con 3.
    ///
    /// La causa es `Box<dyn Error>` y no `age::DecryptError` porque a este caso
    /// tambien se llega por un `io::Error`: un sobre cortado a la mitad falla
    /// leyendo, no descifrando, y sigue siendo un sobre invalido.
    NoEsSobre {
        origen: String,
        causa: Box<dyn std::error::Error>,
    },
    /// Todo lo demas: argumentos, permisos, disco. Sale con 1.
    Otro(Box<dyn std::error::Error>),
}

impl Error {
    /// El numero que ve el shell.
    fn codigo(&self) -> u8 {
        match self {
            Error::ClaveIncorrecta { .. } => SALIDA_CLAVE,
            Error::NoEsSobre { .. } => SALIDA_NO_ES_SOBRE,
            Error::Otro(_) => SALIDA_ERROR,
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::ClaveIncorrecta { origen, .. } => write!(f, "la clave no abre {origen}"),
            Error::NoEsSobre { origen, .. } => {
                write!(f, "{origen} no es un sobre que se pueda abrir con clave")
            }
            Error::Otro(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            // Display ya conto la version en castellano; `source` deja abajo la
            // version tecnica, que es la que sirve para reportar un bug.
            Error::ClaveIncorrecta { causa, .. } => Some(causa),
            Error::NoEsSobre { causa, .. } => Some(causa.as_ref()),
            // Aca es al reves: Display imprimio el mensaje de la caja, asi que
            // seguir la cadena desde la caja misma lo repetiria. Se arranca un
            // eslabon mas abajo.
            Error::Otro(e) => e.source(),
        }
    }
}

// Estos tres `From` son lo unico que hace falta para que todos los `?` y todos
// los `.map_err(|e| format!(...))?` que ya estaban escritos sigan compilando sin
// tocarlos: `?` no hace magia, llama a `From::from` sobre el error y sigue.
impl From<String> for Error {
    fn from(m: String) -> Self {
        Error::Otro(m.into())
    }
}

impl From<&str> for Error {
    fn from(m: &str) -> Self {
        Error::Otro(m.into())
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Otro(Box::new(e))
    }
}

/// Traduce un error de `age` a una de nuestras tres categorias.
///
/// Esta funcion es la unica parte del programa que sabe algo sobre como falla
/// `age`, y esta escrita mirando el crate, no adivinando por el nombre de las
/// variantes. La trampa esta en `NoMatchingKeys`: suena a "la clave no anda" y
/// no es eso.
fn clasificar(e: age::DecryptError, origen: &str) -> Error {
    match e {
        // El AEAD que envuelve la file key no cerro. Con una sola identidad
        // scrypt en juego eso significa una sola cosa: la clave no es esa.
        // (age/src/error.rs: `From<chacha20poly1305::aead::Error>` mapea el
        // fallo del AEAD a esta variante.)
        age::DecryptError::DecryptionFailed => Error::ClaveIncorrecta {
            origen: origen.to_string(),
            causa: e,
        },

        // Ninguna identidad reconocio el stanza. Como la unica que le pasamos es
        // la passphrase, esto quiere decir que el sobre no esta cerrado con
        // clave sino a nombre de un destinatario x25519 o ssh. Es un sobre age
        // perfectamente valido; simplemente no hay clave que lo abra.
        age::DecryptError::NoMatchingKeys => Error::NoEsSobre {
            origen: origen.to_string(),
            causa: Box::new(e),
        },

        // scrypt pide mas trabajo del que esta maquina acepta gastar. Ni la
        // clave ni el sobre estan mal: el que no da es este equipo.
        age::DecryptError::ExcessiveWork { .. } => Error::Otro(Box::new(e)),

        // Se corto la lectura a mitad del header: el sobre esta truncado.
        age::DecryptError::Io(ref io) if io.kind() == io::ErrorKind::UnexpectedEof => {
            Error::NoEsSobre {
                origen: origen.to_string(),
                causa: Box::new(e),
            }
        }

        // Cualquier otro problema de lectura es del disco, no del sobre.
        age::DecryptError::Io(_) => Error::Otro(Box::new(e)),

        // Header ilegible, MAC que no cierra, formato de otra version.
        _ => Error::NoEsSobre {
            origen: origen.to_string(),
            causa: Box::new(e),
        },
    }
}

/// Clasifica un fallo ocurrido copiando el contenido ya descifrado.
///
/// `age` verifica cada bloque del payload al leerlo, asi que un `InvalidData`
/// aca no es el disco fallando: es el MAC de un bloque que no cierra. Un corte
/// de stream es un sobre al que le falta la cola.
fn clasificar_copia(e: io::Error, origen: &str) -> Error {
    match e.kind() {
        io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof => Error::NoEsSobre {
            origen: origen.to_string(),
            causa: Box::new(e),
        },
        _ => Error::Otro(Box::new(e)),
    }
}

// ---------------------------------------------------------------------------
// Argumentos
// ---------------------------------------------------------------------------

/// Que hacer despues de leer la linea de comandos.
///
/// Son tres salidas y no dos, y por eso no es un `Option<Opciones>`: ahi `None`
/// tendria que significar "mostra la ayuda" y "deci la version" al mismo tiempo.
/// El enum le pone nombre a cada una, y el `match` de `correr` deja de compilar
/// el dia que aparezca una cuarta.
enum Accion {
    Ejecutar(Opciones),
    Ayuda,
    Version,
}

/// Que operacion se pidio. Se valida antes de tocar stdin.
enum Orden {
    Cifrar,
    Abrir,
}

struct Opciones {
    orden: String,
    /// `None` = la entrada llega por stdin, enmarcada atras de la clave.
    entrada: Option<String>,
    /// `None` = la salida sale por stdout.
    salida: Option<String>,
    utf16le: bool,
    forzar: bool,
}

impl Opciones {
    /// Como nombrar la entrada dentro de un mensaje de error.
    fn nombre_entrada(&self) -> String {
        match &self.entrada {
            Some(ruta) => ruta.clone(),
            None => "la entrada estandar".to_string(),
        }
    }
}

/// La forma que tiene que tener la linea de comandos con estos flags puestos.
fn forma(por_stdin: bool, por_stdout: bool) -> &'static str {
    match (por_stdin, por_stdout) {
        (false, false) => "sobre <orden> <entrada> <salida>",
        (true, false) => "sobre <orden> --stdin <salida>",
        (false, true) => "sobre <orden> --stdout <entrada>",
        (true, true) => "sobre <orden> --stdin --stdout",
    }
}

fn parsear() -> Result<Accion, Error> {
    let mut sueltos: Vec<String> = Vec::new();
    let mut utf16le = false;
    let mut forzar = false;
    let mut por_stdin = false;
    let mut por_stdout = false;

    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "-h" | "--help" => return Ok(Accion::Ayuda),
            "-V" | "--version" => return Ok(Accion::Version),
            "--utf16le" => utf16le = true,
            "--forzar" => forzar = true,
            "--stdin" => por_stdin = true,
            "--stdout" => por_stdout = true,
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
        return Ok(Accion::Ayuda);
    }

    // La orden siempre esta; cada flag de flujo se lleva un posicional.
    let esperados = 1 + usize::from(!por_stdin) + usize::from(!por_stdout);
    if sueltos.len() != esperados {
        return Err(format!(
            "esperaba: {}. Me pasaste {} cosa(s) y no {esperados}.",
            forma(por_stdin, por_stdout),
            sueltos.len()
        )
        .into());
    }

    let mut resto = sueltos.into_iter();
    let orden = resto.next().expect("ya verifique que hay al menos uno");
    let entrada = if por_stdin { None } else { resto.next() };
    let salida = if por_stdout { None } else { resto.next() };

    Ok(Accion::Ejecutar(Opciones {
        orden,
        entrada,
        salida,
        utf16le,
        forzar,
    }))
}

// ---------------------------------------------------------------------------
// La clave
// ---------------------------------------------------------------------------

/// Lee la clave de stdin. Devuelve un SecretString, que se borra solo al morir.
///
/// Esto es lo que Python no podia hacer: alla un `str` es inmutable y queda en el
/// heap hasta que el recolector se digne. Aca `SecretString` sobreescribe con
/// ceros cuando sale de alcance, sin que haya que acordarse.
///
/// Pero el SecretString es el final del camino, no el camino entero. Antes de
/// llegar ahi la clave pasa por los buffers donde se la lee y se la convierte, y
/// esos son Vec y String comunes: se liberan sin borrarse, asi que el contenido
/// queda en el heap hasta que otra cosa lo pise. Por eso cada paso intermedio va
/// envuelto en `Zeroizing`, que hace lo mismo que `SecretString` pero para un
/// buffer cualquiera.
///
/// El crate da la herramienta, no la garantia: hay que seguir el dato por todos
/// los lugares por donde pasa. Y "todos" incluye los que no se ven, como la
/// realocacion que hace crecer un String: ver los comentarios de abajo.
///
/// `lector` se toma prestado en vez de abrir stdin aca adentro, porque con
/// `--stdin` el contenido viene atras de la clave por el mismo cano y quien
/// llama necesita seguir leyendo de ese mismo lector, con su buffer intacto.
fn leer_clave(
    lector: &mut impl BufRead,
    utf16le: bool,
    enmarcada: bool,
) -> Result<SecretString, Error> {
    let crudo = if enmarcada {
        leer_enmarcada(lector)?
    } else {
        // Sin marco, stdin entero es la clave: no hay nada mas viajando por ahi.
        //
        // El Zeroizing se pone ANTES de leer, no despues. Si `read_to_end` falla
        // a mitad de camino, lo que ya entro se borra igual; envolver el Vec
        // recien cuando la lectura salio bien deja ese caso afuera.
        let mut v = Zeroizing::new(Vec::new());
        lector
            .read_to_end(&mut v)
            .map_err(|e| format!("no pude leer la clave de stdin: {e}"))?;
        v
    };

    let mut texto = if utf16le {
        if crudo.len() % 2 != 0 {
            return Err("la clave en UTF-16LE tiene un numero impar de bytes".into());
        }
        // Este es el camino que usa Askpass.ps1, y el que mas copias hace: una
        // para pasar a u16 y otra para pasar a String.
        let unidades: Zeroizing<Vec<u16>> = Zeroizing::new(
            crudo
                .chunks_exact(2)
                .map(|par| u16::from_le_bytes([par[0], par[1]]))
                .collect(),
        );

        // El buffer se reserva en el peor caso -3 bytes UTF-8 por unidad
        // UTF-16- para que el String no crezca ni una vez.
        //
        // Un String que crece realoca, y realocar copia los bytes a un bloque
        // nuevo y libera el viejo TAL CUAL ESTABA. Ese bloque abandonado tiene
        // la clave y ningun Zeroizing lo alcanza: Zeroizing borra el buffer que
        // conoce, no el que el allocator ya se llevo. El propio zeroize lo
        // aclara en su documentacion de `Vec`, "cannot ensure that previous
        // reallocations did not leave values on the heap".
        //
        // `String::from_utf16` no sirve por eso: reserva una unidad UTF-16 por
        // byte UTF-8, una cuenta que solo cierra en ASCII. Una `ñ` ocupa una
        // unidad y dos bytes, un acento igual, y ahi el String crece.
        let mut acumulado = Zeroizing::new(String::with_capacity(unidades.len() * 3));
        for c in char::decode_utf16(unidades.iter().copied()) {
            // El error de `decode_utf16` se queda con la unidad suelta adentro,
            // que es un pedazo de la clave. Se lo cambia por un texto fijo antes
            // de que pueda viajar a ningun lado, igual que en el camino UTF-8.
            acumulado.push(c.map_err(|_| "la clave no es UTF-16LE valido")?);
        }
        acumulado
    } else {
        // Validar sin consumir y despues copiar. `String::from_utf8` seria una
        // copia menos, pero cuando falla devuelve un error que se queda con los
        // bytes adentro: la clave terminaria viajando dentro del error.
        Zeroizing::new(
            std::str::from_utf8(&crudo)
                .map_err(|_| "la clave no es UTF-8 valido")?
                .to_owned(),
        )
    };

    if !enmarcada {
        // Los saltos del final son del pipe, no de la clave. Adentro se respetan.
        //
        // Con marco no se toca nada: el largo ya dijo donde termina la clave, y
        // recortarla despues seria desmentirlo. Ademas hace expresable una clave
        // que de verdad termine en salto de linea, que sin marco es imposible.
        while texto.ends_with('\n') || texto.ends_with('\r') {
            texto.pop();
        }
    }

    if texto.is_empty() {
        return Err("no llego ninguna clave por stdin".into());
    }

    // Ultimo paso: un String con la capacidad justa, por la misma razon dada
    // vuelta.
    //
    // `SecretString::from(String)` hace `into_boxed_str()`, que llama a
    // `shrink_to_fit()`. Si sobra capacidad -y sobra siempre que se haya usado
    // `pop`, o que el peor caso de arriba no se cumpla- eso realoca, y queda
    // otra copia de la clave en el heap que nadie va a borrar. Con capacidad
    // igual al largo no hay nada que encoger, asi que no realoca.
    let mut exacto = String::with_capacity(texto.len());
    exacto.push_str(&texto);
    Ok(SecretString::from(exacto))
}

/// Lee `<largo en ASCII>\n<largo bytes de clave>` y deja el resto del stream
/// intacto para el contenido.
///
/// El largo explicito no es capricho. Con un separador -"la clave es hasta el
/// primer salto de linea"- alcanzaria con que la clave tenga un caracter cuyo
/// byte bajo en UTF-16LE sea 0x0A para que el corte caiga en el lugar
/// equivocado, y el programa no tendria forma de darse cuenta: la clave partida
/// tambien es una clave valida, solo que otra. Fallaria al abrir el sobre, tres
/// pasos despues, con un mensaje que no apunta a nada.
fn leer_enmarcada(lector: &mut impl BufRead) -> Result<Zeroizing<Vec<u8>>, Error> {
    let mut cabecera = Vec::new();
    {
        // `take` acota cuanto se puede leer sin romper nada de abajo: lo que el
        // BufRead ya tenga adentro y no se consuma sigue ahi, esperando al
        // contenido. Por eso hay que pasar el MISMO lector y no abrir stdin de
        // nuevo: un lector nuevo empezaria despues de lo que el primero ya se
        // trajo a su buffer, y esos bytes se perderian sin que nadie se entere.
        let mut acotado = lector.by_ref().take(CABECERA_MAX);
        acotado
            .read_until(b'\n', &mut cabecera)
            .map_err(|e| format!("no pude leer la cabecera de --stdin: {e}"))?;
    }

    if cabecera.last() != Some(&b'\n') {
        return Err(format!(
            "--stdin espera el largo de la clave en bytes y un salto de linea antes de la \
             clave. No encontre el salto en los primeros {CABECERA_MAX} bytes."
        )
        .into());
    }
    cabecera.pop();

    let texto = std::str::from_utf8(&cabecera)
        .map_err(|_| "la cabecera de --stdin no es texto ASCII")?;
    let largo: usize = texto.parse().map_err(|_| {
        format!("la cabecera de --stdin dice {texto:?}, que no es un largo en bytes")
    })?;

    if largo == 0 {
        return Err("la cabecera de --stdin declara una clave de cero bytes".into());
    }
    if largo > CLAVE_MAX {
        return Err(format!(
            "la cabecera de --stdin declara {largo} bytes de clave y el tope es {CLAVE_MAX}"
        )
        .into());
    }

    // Reservado y envuelto antes de leer: si `read_exact` se corta a mitad, los
    // bytes que si llegaron se borran igual.
    //
    // `read_exact` y no `read_to_end` porque el largo ya se sabe y porque el
    // resto del stream no es nuestro: `read_to_end` se llevaria el contenido
    // puesto, que es exactamente lo que hay que dejar en su lugar.
    let mut clave = Zeroizing::new(vec![0u8; largo]);
    lector.read_exact(&mut clave).map_err(|e| {
        format!("--stdin prometio {largo} bytes de clave y el stream se corto antes: {e}")
    })?;

    Ok(clave)
}

// ---------------------------------------------------------------------------
// Entrada y salida
// ---------------------------------------------------------------------------

/// De donde salen los bytes a procesar.
///
/// Devuelve `Box<dyn BufRead>` y no un generico porque las dos ramas son tipos
/// distintos que se eligen recien en tiempo de ejecucion. La caja cuesta un
/// salto indirecto por llamada, y al lado de scrypt eso no se mide.
fn abrir_origen(
    opciones: &Opciones,
    entrada_estandar: io::StdinLock<'static>,
) -> Result<Box<dyn BufRead>, Error> {
    match &opciones.entrada {
        // El lector viene con la clave ya consumida: lo que queda adentro es
        // exactamente el contenido, sin una copia intermedia en ningun lado.
        None => Ok(Box::new(entrada_estandar)),
        Some(ruta) => {
            let f = File::open(ruta).map_err(|e| format!("no pude abrir {ruta}: {e}"))?;
            Ok(Box::new(BufReader::new(f)))
        }
    }
}

/// A donde van los bytes procesados.
///
/// Con `--stdout` no hay chequeo de "ya existe" ni `--forzar` que valga: stdout
/// es de quien nos llamo y el que decide donde cae es el.
fn abrir_destino(opciones: &Opciones) -> Result<Box<dyn Write>, Error> {
    match &opciones.salida {
        // BufWriter arriba del lock porque StdoutLock es un LineWriter: sin esto
        // vaciaria en cada salto de linea, y en binario los saltos de linea son
        // bytes cualquiera que caen cada tanto.
        None => Ok(Box::new(BufWriter::new(io::stdout().lock()))),
        Some(ruta) => {
            let destino = Path::new(ruta);
            if destino.exists() && !opciones.forzar {
                return Err(
                    format!("{ruta} ya existe. Usa --forzar si de verdad lo querés pisar.").into(),
                );
            }
            let f = File::create(destino).map_err(|e| format!("no pude crear {ruta}: {e}"))?;
            Ok(Box::new(BufWriter::new(f)))
        }
    }
}

// ---------------------------------------------------------------------------
// Las dos ordenes
// ---------------------------------------------------------------------------

fn cifrar(
    opciones: &Opciones,
    clave: SecretString,
    mut origen: Box<dyn BufRead>,
) -> Result<(), Error> {
    let destino = abrir_destino(opciones)?;

    let cifrador = age::Encryptor::with_user_passphrase(clave);
    let mut flujo = cifrador.wrap_output(destino)?;
    let copiados = io::copy(&mut origen, &mut flujo)?;
    // Sin este finish el archivo queda truncado y no abre nunca mas. Es el mismo
    // tipo de error silencioso que mandarle la clave dos veces a rar.
    let mut cerrado = flujo.finish()?;
    // Y sin este flush explicito, un BufWriter que se cae solo se lleva el
    // ultimo bloque sin avisar: su Drop ignora el error de escritura.
    cerrado.flush()?;

    match &opciones.salida {
        Some(ruta) => {
            let bytes = std::fs::metadata(ruta)?.len();
            eprintln!("cerrado: {ruta} ({bytes} bytes)");
        }
        None => eprintln!("cerrado: {copiados} bytes de contenido -> stdout"),
    }
    Ok(())
}

fn abrir(opciones: &Opciones, clave: SecretString, origen: Box<dyn BufRead>) -> Result<(), Error> {
    let nombre = opciones.nombre_entrada();

    // Fallar aca significa que el header no se pudo leer: sea lo que sea, no es
    // un sobre age.
    let descifrador =
        age::Decryptor::new_buffered(origen).map_err(|e| clasificar(e, &nombre))?;

    let identidad = age::scrypt::Identity::new(clave);
    // Fallar aca es lo interesante: el sobre existe y esta bien armado, asi que
    // lo que puede estar mal es la clave. `clasificar` separa ese caso del resto.
    let mut flujo = descifrador
        .decrypt(std::iter::once(&identidad as &dyn age::Identity))
        .map_err(|e| clasificar(e, &nombre))?;

    // El destino se abre despues de que el descifrado arranco, no antes: al
    // reves queda un archivo vacio en disco cada vez que alguien se equivoca de
    // clave.
    let mut destino = abrir_destino(opciones)?;
    let copiados = io::copy(&mut flujo, &mut destino).map_err(|e| clasificar_copia(e, &nombre))?;
    destino.flush()?;

    match &opciones.salida {
        Some(ruta) => eprintln!("abierto: {ruta}"),
        None => eprintln!("abierto: {copiados} bytes -> stdout"),
    }
    Ok(())
}

// ---------------------------------------------------------------------------

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
            ExitCode::from(e.codigo())
        }
    }
}

fn correr() -> Result<(), Error> {
    let opciones = match parsear()? {
        Accion::Ejecutar(o) => o,
        Accion::Ayuda => {
            print!("{AYUDA}");
            return Ok(());
        }
        Accion::Version => {
            // De Cargo.toml, en tiempo de compilacion. No hay dos numeros que
            // se puedan desincronizar porque hay uno solo.
            println!("sobre {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
    };

    // La orden se valida antes de tocar stdin. Al reves, un `sobre cifar` mal
    // tipeado se queda esperando una clave que despues va a tirar igual.
    let orden = match opciones.orden.as_str() {
        "cifrar" => Orden::Cifrar,
        "abrir" => Orden::Abrir,
        otra => return Err(format!("no conozco la orden {otra:?}. Probá con --help").into()),
    };

    // Un solo lector para todo stdin, y tiene que ser uno solo: con --stdin la
    // clave y el contenido viajan por el mismo cano.
    let mut entrada_estandar = io::stdin().lock();
    let enmarcada = opciones.entrada.is_none();
    let clave = leer_clave(&mut entrada_estandar, opciones.utf16le, enmarcada)?;

    let origen = abrir_origen(&opciones, entrada_estandar)?;

    match orden {
        Orden::Cifrar => cifrar(&opciones, clave, origen),
        Orden::Abrir => abrir(&opciones, clave, origen),
    }
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod pruebas {
    use super::*;
    use age::secrecy::ExposeSecret;

    /// Un `&[u8]` ya es BufRead, asi que el marco se puede probar sin proceso,
    /// sin pipe y sin archivo.
    fn leer(entrada: &[u8], utf16le: bool, enmarcada: bool) -> Result<(String, Vec<u8>), Error> {
        let mut lector = entrada;
        let clave = leer_clave(&mut lector, utf16le, enmarcada)?;
        let mut resto = Vec::new();
        lector.read_to_end(&mut resto).unwrap();
        Ok((clave.expose_secret().to_string(), resto))
    }

    #[test]
    fn marco_separa_clave_y_contenido() {
        let (clave, resto) = leer(b"5\nabcdeel contenido", false, true).unwrap();
        assert_eq!(clave, "abcde");
        assert_eq!(resto, b"el contenido");
    }

    #[test]
    fn marco_deja_el_contenido_intacto_aunque_empiece_con_salto() {
        let (clave, resto) = leer(b"3\nabc\n\nhola", false, true).unwrap();
        assert_eq!(clave, "abc");
        assert_eq!(resto, b"\n\nhola");
    }

    #[test]
    fn marco_respeta_el_largo_exacto_sin_recortar_saltos() {
        // La clave son 3 bytes y el tercero es un salto de linea. Sin marco esto
        // seria imposible de expresar: el recorte se lo comeria.
        let (clave, resto) = leer(b"3\nab\ncontenido", false, true).unwrap();
        assert_eq!(clave, "ab\n");
        assert_eq!(resto, b"contenido");
    }

    #[test]
    fn sin_marco_si_recorta_los_saltos_del_pipe() {
        let (clave, _) = leer(b"abcde\r\n", false, false).unwrap();
        assert_eq!(clave, "abcde");
    }

    #[test]
    fn marco_con_clave_que_contiene_0x0a_no_se_parte() {
        // "\u{0a0a}" en UTF-16LE es 0x0A 0x0A: los dos bytes son saltos de linea
        // y ninguno lo es. Este es el caso que hunde a un separador.
        let bytes = [0x0Au8, 0x0A, 0x41, 0x00];
        let mut entrada = b"4\n".to_vec();
        entrada.extend_from_slice(&bytes);
        entrada.extend_from_slice(b"payload");

        let (clave, resto) = leer(&entrada, true, true).unwrap();
        assert_eq!(clave, "\u{0a0a}A");
        assert_eq!(resto, b"payload");
    }

    #[test]
    fn los_dos_caminos_derivan_la_misma_clave() {
        // Si esto fallara, un sobre cerrado con --utf16le no abriria sin el flag.
        let clave = "contraseña ñandú 480";
        let utf16: Vec<u8> = clave
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();

        let mut a = format!("{}\n", clave.len()).into_bytes();
        a.extend_from_slice(clave.as_bytes());
        let mut b = format!("{}\n", utf16.len()).into_bytes();
        b.extend_from_slice(&utf16);

        assert_eq!(
            leer(&a, false, true).unwrap().0,
            leer(&b, true, true).unwrap().0
        );
    }

    #[test]
    fn cabecera_sin_salto_falla() {
        let e = leer(b"12345678901234567890123456", false, true).unwrap_err();
        assert_eq!(e.codigo(), SALIDA_ERROR);
        assert!(e.to_string().contains("salto"));
    }

    #[test]
    fn cabecera_no_numerica_falla() {
        assert!(leer(b"hola\nabc", false, true).is_err());
    }

    #[test]
    fn cabecera_de_cero_falla() {
        assert!(leer(b"0\ncontenido", false, true).is_err());
    }

    #[test]
    fn cabecera_mas_grande_que_el_tope_falla_sin_reservar() {
        let e = leer(b"4000000000\nx", false, true).unwrap_err();
        assert!(e.to_string().contains("tope"));
    }

    #[test]
    fn clave_truncada_falla() {
        let e = leer(b"100\nsolo unos pocos", false, true).unwrap_err();
        assert!(e.to_string().contains("se corto"));
    }

    #[test]
    fn utf16le_impar_falla() {
        assert!(leer(b"3\nabc", true, true).is_err());
    }

    #[test]
    fn los_codigos_de_salida_no_se_pisan() {
        assert_eq!(Error::from("x").codigo(), SALIDA_ERROR);
        assert_eq!(
            Error::ClaveIncorrecta {
                origen: "x".into(),
                causa: age::DecryptError::DecryptionFailed,
            }
            .codigo(),
            SALIDA_CLAVE
        );
        assert_eq!(
            Error::NoEsSobre {
                origen: "x".into(),
                causa: Box::new(age::DecryptError::InvalidHeader),
            }
            .codigo(),
            SALIDA_NO_ES_SOBRE
        );
    }

    #[test]
    fn clave_incorrecta_se_distingue_de_sobre_invalido() {
        // El nombre engania: NoMatchingKeys NO es "la clave esta mal", es "este
        // sobre no esta cerrado con clave". Si esto se invierte, el codigo 2
        // deja de significar lo que dice la ayuda.
        assert_eq!(
            clasificar(age::DecryptError::DecryptionFailed, "x").codigo(),
            SALIDA_CLAVE
        );
        assert_eq!(
            clasificar(age::DecryptError::NoMatchingKeys, "x").codigo(),
            SALIDA_NO_ES_SOBRE
        );
        assert_eq!(
            clasificar(age::DecryptError::InvalidMac, "x").codigo(),
            SALIDA_NO_ES_SOBRE
        );
        assert_eq!(
            clasificar(age::DecryptError::UnknownFormat, "x").codigo(),
            SALIDA_NO_ES_SOBRE
        );
        assert_eq!(
            clasificar(
                age::DecryptError::ExcessiveWork {
                    required: 30,
                    target: 18
                },
                "x"
            )
            .codigo(),
            SALIDA_ERROR
        );
    }

    #[test]
    fn un_sobre_truncado_es_sobre_invalido_y_no_error_de_disco() {
        let corte = io::Error::new(io::ErrorKind::UnexpectedEof, "corte");
        assert_eq!(
            clasificar(age::DecryptError::Io(corte), "x").codigo(),
            SALIDA_NO_ES_SOBRE
        );

        let disco = io::Error::new(io::ErrorKind::PermissionDenied, "permisos");
        assert_eq!(
            clasificar(age::DecryptError::Io(disco), "x").codigo(),
            SALIDA_ERROR
        );

        let mac = io::Error::new(io::ErrorKind::InvalidData, "mac");
        assert_eq!(clasificar_copia(mac, "x").codigo(), SALIDA_NO_ES_SOBRE);
    }

    #[test]
    fn el_mensaje_no_repite_la_causa() {
        let e = Error::from("no pude abrir notas.txt".to_string());
        assert_eq!(e.to_string(), "no pude abrir notas.txt");
        // `Otro` arranca la cadena un eslabon mas abajo justamente para esto.
        assert!(e.source().is_none());
    }
}
