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
//! con scrypt o x25519 para llegar a la clave del archivo. Esta especificado,
//! auditado y lo implementa el crate `age`. Este archivo solo mueve bytes de un
//! lado al otro y se ocupa de que el secreto no toque la linea de comandos.
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
//! (2) de "esto no es un sobre" (3) de "pide mas trabajo del que puedo gastar"
//! (4) y del resto (1). Quien envuelve a `sobre` decide si volver a pedir la
//! clave, rendirse o subir el tope, sin parsear el texto del error, que es la
//! parte que no promete quedarse quieta entre versiones.
//!
//! **Pagar un KDF que no necesita.** Una passphrase cuesta ~1,9 s por corrida en
//! las dos puntas, y eso es scrypt haciendo su trabajo. Con `--para` el sobre se
//! cierra a nombre de una clave publica x25519: no hay derivacion, no hay
//! calibracion, no hay segundo perdido. Es la diferencia entre cerrar un sobre
//! grande por dia y cerrar mil chicos por minuto.
//!
//! **Regalar el tamano.** Un sobre mide lo que mide el contenido mas 166 bytes
//! y 16 por cada bloque de 64 KiB, asi que su tamano lo delata. `--bloque`
//! acolcha el contenido a un multiplo antes de cerrarlo.
//!
//! **Abrir Cargo.toml para saber que version corre.** `--version` contesta.

use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::process::ExitCode;
use std::str::FromStr;

// El trait se importa sin nombre (`as _`) para poder llamar a `.source()` sin
// que `std::error::Error` choque con nuestro `Error` de mas abajo.
use std::error::Error as _;

use age::secrecy::zeroize::Zeroizing;
use age::secrecy::{ExposeSecret, SecretString};

const AYUDA: &str = "\
sobre - archivo cifrado con un secreto que nunca toca la linea de comandos

ORDENES:
    sobre cifrar      <entrada> <salida>   cerrar el sobre
    sobre abrir       <entrada> <salida>   abrirlo
    sobre generar     <salida>             crear una identidad x25519
    sobre rellenar    <entrada> <salida>   acolchar el tamano  (pide --bloque)
    sobre desrellenar <entrada> <salida>   sacarle el acolchado

CON QUE SE CIERRA:
    (nada)             la clave se lee de stdin. Deriva con scrypt, que cuesta
                       un par de segundos en cada punta. Para pocos sobres.
    --para <age1...>   se cierra a nombre de una clave publica x25519. Sin KDF:
                       microsegundos. Se puede repetir para varios destinatarios.
    --identidad <ruta> se abre con la identidad privada de ese archivo.

    Un destinatario es publico, asi que va como argumento sin problema. Una
    clave no: no hay -p ni --clave, nunca. Los argumentos de un proceso los
    puede leer cualquier otro proceso.

    Con --para o --identidad no se lee ninguna clave de stdin, asi que stdin
    queda libre para el contenido y --stdin no necesita marco.

FLUJOS:
    --stdin        la entrada llega por stdin, atras de la clave (ver EL MARCO)
    --stdout       el resultado sale por stdout en vez de a un archivo
    --forzar       sobreescribir la salida si ya existe

    Cada flag de flujo saca su argumento posicional:
        sobre cifrar --stdin <salida>
        sobre abrir  --stdout <entrada>
        sobre cifrar --stdin --stdout          filtro puro: nada toca disco

OPCIONES:
    --utf16le          la clave llega en UTF-16LE (lo que escupe Askpass.ps1)
    --bloque <tamano>  acolchar el contenido a un multiplo antes de cerrarlo
                       (4096, 4K, 1M, 16M). Vale en cifrar y en rellenar.
    --desrellenar      sacarle el acolchado al abrir
    --trabajo <n>      factor scrypt fijo al cerrar, 1..63. Sin esto, `age`
                       mide la maquina y apunta a un segundo.
    --max-trabajo <n>  cuanto trabajo se acepta gastar al abrir, 1..63
    -h, --help         esto
    -V, --version      numero de version y nada mas

EL MARCO (--stdin con clave):
    Cuando la clave y el contenido comparten stdin hay que decir donde termina
    una y empieza el otro:

        <largo de la clave en bytes, ASCII decimal> \\n <clave> <contenido>

    El largo es exacto: a la clave enmarcada no se le saca ningun salto de
    linea, porque vos ya dijiste cuantos bytes son. Sin marco, stdin entero es
    la clave y si se le sacan los saltos del final, que son del pipe.

    Se eligio el largo explicito y no un separador porque en UTF-16LE un
    caracter cualquiera puede tener 0x0A como byte bajo: cortar en el primer
    salto de linea partiria la clave al medio, y la clave partida tambien es
    una clave valida, solo que otra.

EL ACOLCHADO:
    ISO/IEC 7816-4: un byte 0x80 y despues ceros, hasta el proximo multiplo del
    bloque. Siempre se agrega algo, aun si el contenido ya cae justo, porque si
    no no habria como saber donde termina.

    El acolchado va adentro del texto en claro, que es el unico lugar donde
    sirve. Un sobre acolchado lo abre cualquier implementacion de age; lo que
    devuelve es el contenido con la cola pegada. Solo `sobre desrellenar` (o
    `abrir --desrellenar`) la saca.

CODIGOS DE SALIDA:
    0   salio bien
    1   error de uso, de disco, de permisos
    2   la clave no abre ese sobre
    3   la entrada no es un sobre que se pueda abrir asi
    4   el sobre pide mas trabajo del que esta maquina acepta gastar

EJEMPLOS:
    # con la ventanita de Parchecker
    powershell -File Askpass.ps1 -Confirmar | sobre cifrar --utf16le notas.txt notas.sobre

    # un secreto que nunca aterriza en claro: 13 bytes de clave y atras el resto
    printf '13\\nclave-secretalo que sea' | sobre cifrar --stdin notas.sobre

    # sin KDF: una identidad, y despues mil sobres a full velocidad
    sobre generar mi.identidad                 # imprime la publica por stderr
    tar cz /datos | sobre cifrar --para age1... --stdin --stdout > backup.sobre
    sobre abrir --identidad mi.identidad --stdout backup.sobre | tar xz

    # acolchado, encadenado o de una
    cat v.txt | sobre rellenar --bloque 4K --stdin --stdout | sobre cifrar --para age1... --stdin r.sobre
    sobre cifrar --para age1... --bloque 4K v.txt r.sobre
";

/// Cuanto puede medir la cabecera de `--stdin`, en bytes.
///
/// `"1048576\n"` son ocho. Veinte es holgado, y el tope existe porque sin el
/// `read_until` se quedaria leyendo para siempre buscando un salto de linea que
/// nunca va a llegar si de arriba mandan cualquier cosa.
const CABECERA_MAX: u64 = 20;

/// Cuanto puede medir la clave enmarcada, en bytes.
///
/// Una passphrase de 64 KiB no existe. El tope esta para que una cabecera
/// mentirosa (`"4000000000\n"`) no reserve cuatro gigas antes de fallar.
const CLAVE_MAX: usize = 64 * 1024;

/// El bloque de acolchado mas grande que se acepta.
///
/// Manda tambien del lado de sacarlo: para encontrar la cola hay que retener el
/// final del stream hasta saber si es relleno o contenido, y lo retenido nunca
/// pasa de un bloque. El tope es, en los hechos, cuanta memoria puede pedir
/// `desrellenar`.
const BLOQUE_MAX: u64 = 16 * 1024 * 1024;

/// La marca que abre la cola de acolchado, de ISO/IEC 7816-4.
const MARCA_RELLENO: u8 = 0x80;

// Los codigos de salida son la unica parte de este programa que otro programa
// puede leer sin parsear texto. Por eso son constantes con nombre: un
// `ExitCode::from(3)` suelto en medio del codigo no le dice nada a nadie.
const SALIDA_ERROR: u8 = 1;
const SALIDA_CLAVE: u8 = 2;
const SALIDA_NO_ES_SOBRE: u8 = 3;
const SALIDA_TRABAJO: u8 = 4;

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
/// programa, es tipar los que alguien de afuera necesita mirar.
#[derive(Debug)]
enum Error {
    /// El sobre esta bien; la clave no es esa. Sale con 2.
    ClaveIncorrecta {
        origen: String,
        causa: age::DecryptError,
    },
    /// Estos bytes no son un sobre age que se pueda abrir asi. Sale con 3.
    ///
    /// La causa es `Box<dyn Error>` y no `age::DecryptError` porque a este caso
    /// tambien se llega por un `io::Error`: un sobre cortado a la mitad falla
    /// leyendo, no descifrando, y sigue siendo un sobre invalido.
    NoEsSobre {
        origen: String,
        causa: Box<dyn std::error::Error>,
    },
    /// El sobre fue cerrado en una maquina mucho mas rapida. Sale con 4.
    ///
    /// Ni la clave ni el sobre estan mal: el que no da es este equipo. Por eso
    /// no es un 2 ni un 3, y por eso el mensaje trae los dos numeros: son los
    /// que hacen falta para elegir un `--max-trabajo`.
    TrabajoExcesivo {
        origen: String,
        pide: u8,
        acepta: u8,
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
            Error::TrabajoExcesivo { .. } => SALIDA_TRABAJO,
            Error::Otro(_) => SALIDA_ERROR,
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::ClaveIncorrecta { origen, .. } => write!(f, "la clave no abre {origen}"),
            Error::NoEsSobre { origen, .. } => {
                write!(f, "{origen} no es un sobre que se pueda abrir asi")
            }
            Error::TrabajoExcesivo {
                origen,
                pide,
                acepta,
            } => write!(
                f,
                "{origen} fue cerrado con un factor de trabajo de {pide} y esta maquina \
                 acepta hasta {acepta}. Usá --max-trabajo {pide} si estás dispuesto a esperar \
                 {}x lo que tarda normalmente.",
                1u64 << pide.saturating_sub(*acepta)
            ),
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
            // El mensaje ya trae los dos numeros: no hay nada mas abajo.
            Error::TrabajoExcesivo { .. } => None,
            // Aca es al reves: Display imprimio el mensaje de la caja, asi que
            // seguir la cadena desde la caja misma lo repetiria. Se arranca un
            // eslabon mas abajo.
            Error::Otro(e) => e.source(),
        }
    }
}

// Estos `From` son lo unico que hace falta para que todos los `?` y todos los
// `.map_err(|e| format!(...))?` sigan compilando sin tocarlos: `?` no hace
// magia, llama a `From::from` sobre el error y sigue.
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

impl From<age::EncryptError> for Error {
    fn from(e: age::EncryptError) -> Self {
        Error::Otro(Box::new(e))
    }
}

/// Traduce un error de `age` a una de nuestras categorias.
///
/// Esta funcion es la unica parte del programa que sabe algo sobre como falla
/// `age`, y esta escrita mirando el crate, no adivinando por el nombre de las
/// variantes. La trampa esta en `NoMatchingKeys`: suena a "la clave no anda" y
/// no es eso.
fn clasificar(e: age::DecryptError, origen: &str) -> Error {
    match e {
        // El AEAD que envuelve la file key no cerro. Con una sola identidad en
        // juego eso significa una sola cosa: el secreto no es ese.
        // (age/src/error.rs: `From<chacha20poly1305::aead::Error>` mapea el
        // fallo del AEAD a esta variante.)
        age::DecryptError::DecryptionFailed => Error::ClaveIncorrecta {
            origen: origen.to_string(),
            causa: e,
        },

        // Ninguna identidad reconocio el stanza: el sobre no esta cerrado de la
        // forma con la que lo estamos tratando de abrir. Una passphrase contra
        // un sobre x25519 cae aca, y tambien al reves.
        age::DecryptError::NoMatchingKeys => Error::NoEsSobre {
            origen: origen.to_string(),
            causa: Box::new(e),
        },

        // scrypt pide mas trabajo del que esta maquina acepta gastar.
        age::DecryptError::ExcessiveWork { required, target } => Error::TrabajoExcesivo {
            origen: origen.to_string(),
            pide: required,
            acepta: target,
        },

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
    Ejecutar(Orden, Opciones),
    Ayuda,
    Version,
}

/// Que operacion se pidio. Se valida antes de tocar stdin.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Orden {
    Cifrar,
    Abrir,
    Generar,
    Rellenar,
    Desrellenar,
}

impl Orden {
    fn desde(nombre: &str) -> Option<Orden> {
        Some(match nombre {
            "cifrar" => Orden::Cifrar,
            "abrir" => Orden::Abrir,
            "generar" => Orden::Generar,
            "rellenar" => Orden::Rellenar,
            "desrellenar" => Orden::Desrellenar,
            _ => return None,
        })
    }

    /// `generar` no lee nada: se saca la identidad de la nada.
    fn lee_entrada(self) -> bool {
        self != Orden::Generar
    }

    /// Si la orden usa un secreto que puede venir por stdin.
    fn tiene_llave(self) -> bool {
        matches!(self, Orden::Cifrar | Orden::Abrir)
    }
}

struct Opciones {
    /// `None` = la entrada llega por stdin.
    entrada: Option<String>,
    /// `None` = la salida sale por stdout.
    salida: Option<String>,
    utf16le: bool,
    forzar: bool,
    /// Claves publicas x25519 a las que cerrar el sobre. Vacio = passphrase.
    para: Vec<String>,
    /// Archivo con la identidad privada x25519. `None` = passphrase.
    identidad: Option<String>,
    trabajo: Option<u8>,
    max_trabajo: Option<u8>,
    bloque: Option<u64>,
    desrellenar: bool,
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
fn forma(orden: Orden, por_stdin: bool, por_stdout: bool) -> String {
    let mut s = String::from("sobre <orden>");
    if orden.lee_entrada() && !por_stdin {
        s.push_str(" <entrada>");
    }
    if !por_stdout {
        s.push_str(" <salida>");
    }
    s
}

/// Convierte `4096`, `4K`, `1M`, `16MiB` a bytes.
fn parsear_tamano(texto: &str) -> Result<u64, Error> {
    let arriba = texto.trim().to_ascii_uppercase();
    let sufijos: [(&str, u64); 9] = [
        ("GIB", 1 << 30),
        ("GB", 1 << 30),
        ("G", 1 << 30),
        ("MIB", 1 << 20),
        ("MB", 1 << 20),
        ("M", 1 << 20),
        ("KIB", 1 << 10),
        ("KB", 1 << 10),
        ("K", 1 << 10),
    ];
    let (digitos, factor) = sufijos
        .iter()
        .find_map(|(suf, f)| arriba.strip_suffix(suf).map(|d| (d, *f)))
        .unwrap_or((arriba.as_str(), 1));

    let n: u64 = digitos
        .trim()
        .parse()
        .map_err(|_| format!("no entiendo el tamano {texto:?}. Probá 4096, 4K, 1M."))?;

    let bytes = n
        .checked_mul(factor)
        .ok_or_else(|| Error::from(format!("el tamano {texto:?} no entra en 64 bits")))?;

    if bytes == 0 {
        return Err("un bloque de cero bytes no acolcha nada".into());
    }
    if bytes > BLOQUE_MAX {
        return Err(format!(
            "el bloque mas grande que acepto es {BLOQUE_MAX} bytes (16M), y pediste {bytes}"
        )
        .into());
    }
    Ok(bytes)
}

/// El factor de trabajo de scrypt, que es un exponente: el costo es 2^n.
///
/// El rango no es capricho nuestro: `age` panickea si se le pasa un `log_n`
/// fuera de `0 < n < 64`. Validar aca convierte un panic en un mensaje.
fn parsear_trabajo(texto: &str) -> Result<u8, Error> {
    let n: u8 = texto
        .trim()
        .parse()
        .map_err(|_| format!("el factor de trabajo {texto:?} no es un numero de 1 a 63"))?;
    if n == 0 || n >= 64 {
        return Err(format!("el factor de trabajo va de 1 a 63, y pediste {n}").into());
    }
    Ok(n)
}

fn parsear() -> Result<Accion, Error> {
    let mut sueltos: Vec<String> = Vec::new();
    let mut utf16le = false;
    let mut forzar = false;
    let mut por_stdin = false;
    let mut por_stdout = false;
    let mut para: Vec<String> = Vec::new();
    let mut identidad: Option<String> = None;
    let mut trabajo: Option<u8> = None;
    let mut max_trabajo: Option<u8> = None;
    let mut bloque: Option<u64> = None;
    let mut desrellenar = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        // `--flag=valor` y `--flag valor` son la misma cosa. Se parte una vez
        // sola aca y despues el match no se entera.
        let (nombre, pegado) = match arg.split_once('=') {
            Some((n, v)) if n.starts_with("--") => (n.to_string(), Some(v.to_string())),
            _ => (arg.clone(), None),
        };

        // Saca el valor de un flag que lo lleva, venga pegado o suelto.
        let valor = |args: &mut dyn Iterator<Item = String>| -> Result<String, Error> {
            match pegado.clone() {
                Some(v) => Ok(v),
                None => args
                    .next()
                    .ok_or_else(|| Error::from(format!("a {nombre:?} le falta el valor"))),
            }
        };

        match nombre.as_str() {
            "-h" | "--help" => return Ok(Accion::Ayuda),
            "-V" | "--version" => return Ok(Accion::Version),
            "--utf16le" => utf16le = true,
            "--forzar" => forzar = true,
            "--stdin" => por_stdin = true,
            "--stdout" => por_stdout = true,
            "--desrellenar" => desrellenar = true,
            "--para" => para.push(valor(&mut args)?),
            "--identidad" => identidad = Some(valor(&mut args)?),
            "--trabajo" => trabajo = Some(parsear_trabajo(&valor(&mut args)?)?),
            "--max-trabajo" => max_trabajo = Some(parsear_trabajo(&valor(&mut args)?)?),
            "--bloque" => bloque = Some(parsear_tamano(&valor(&mut args)?)?),
            // Cortar de raiz cualquier intento de pasar la clave por argumento.
            a if a.starts_with("-p") || a.starts_with("--clave") || a.starts_with("--password") => {
                return Err(
                    "la clave no se pasa por argumento, nunca. Mandala por stdin: \
                     los argumentos de un proceso los puede leer cualquier otro proceso. \
                     (Un destinatario x25519 si va por argumento: es publico. Ver --para.)"
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

    let orden = Orden::desde(&sueltos[0])
        .ok_or_else(|| Error::from(format!("no conozco la orden {:?}. Probá con --help", sueltos[0])))?;

    // La orden siempre esta; cada flag de flujo se lleva un posicional, y
    // `generar` no tiene entrada que llevarse.
    let esperados = 1
        + usize::from(orden.lee_entrada() && !por_stdin)
        + usize::from(!por_stdout);
    if sueltos.len() != esperados {
        return Err(format!(
            "esperaba: {}. Me pasaste {} cosa(s) y no {esperados}.",
            forma(orden, por_stdin, por_stdout),
            sueltos.len()
        )
        .into());
    }

    let mut resto = sueltos.into_iter().skip(1);
    let entrada = if orden.lee_entrada() && !por_stdin {
        resto.next()
    } else {
        None
    };
    let salida = if por_stdout { None } else { resto.next() };

    let opciones = Opciones {
        entrada,
        salida,
        utf16le,
        forzar,
        para,
        identidad,
        trabajo,
        max_trabajo,
        bloque,
        desrellenar,
    };
    validar(orden, &opciones, por_stdin)?;
    Ok(Accion::Ejecutar(orden, opciones))
}

/// Rechaza las combinaciones que no quieren decir nada.
///
/// Un flag que la orden ignora en silencio es peor que un error: quien lo
/// escribio cree que hizo algo. Vale la misma regla que con `-p`, hacer
/// imposible lo incorrecto en vez de documentar que no se hace.
fn validar(orden: Orden, o: &Opciones, por_stdin: bool) -> Result<(), Error> {
    if !o.para.is_empty() && o.identidad.is_some() {
        return Err("--para cierra y --identidad abre. No van juntos.".into());
    }
    if !o.para.is_empty() && orden != Orden::Cifrar {
        return Err("--para es de cifrar: dice a nombre de quien se cierra el sobre.".into());
    }
    if o.identidad.is_some() && orden != Orden::Abrir {
        return Err("--identidad es de abrir: dice con que se abre el sobre.".into());
    }
    if o.trabajo.is_some() {
        if orden != Orden::Cifrar {
            return Err("--trabajo es de cifrar.".into());
        }
        if !o.para.is_empty() {
            return Err("--trabajo es el costo de scrypt, y --para no usa scrypt.".into());
        }
    }
    if o.max_trabajo.is_some() {
        if orden != Orden::Abrir {
            return Err("--max-trabajo es de abrir.".into());
        }
        if o.identidad.is_some() {
            return Err("--max-trabajo es el tope de scrypt, y --identidad no usa scrypt.".into());
        }
    }
    match orden {
        Orden::Rellenar if o.bloque.is_none() => {
            return Err("rellenar necesita --bloque <tamano>. Sin el no sabe hasta donde.".into())
        }
        Orden::Cifrar | Orden::Rellenar => {}
        _ if o.bloque.is_some() => {
            return Err("--bloque acolcha antes de cerrar: va en cifrar o en rellenar.".into())
        }
        _ => {}
    }
    if o.desrellenar && orden != Orden::Abrir {
        return Err("--desrellenar es de abrir. Suelto, la orden es `sobre desrellenar`.".into());
    }
    if o.utf16le && !orden.tiene_llave() {
        return Err("--utf16le habla de la clave, y esa orden no usa clave.".into());
    }
    if por_stdin && !orden.lee_entrada() {
        return Err("generar no lee nada: --stdin no tiene sentido ahi.".into());
    }
    Ok(())
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

    let texto =
        std::str::from_utf8(&cabecera).map_err(|_| "la cabecera de --stdin no es texto ASCII")?;
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
// Identidades x25519
// ---------------------------------------------------------------------------

/// Lee una identidad privada de un archivo con formato age.
///
/// El archivo es una linea `AGE-SECRET-KEY-1...` y, opcionalmente, comentarios
/// que arrancan con `#`. Se lee entero a un `Zeroizing<String>` porque la linea
/// util es tan secreta como una passphrase, y el archivo la trae en claro.
///
/// El `Err` de `FromStr` es un `&'static str`, asi que no arrastra ningun
/// pedazo de la clave adentro del mensaje. Vale la pena confirmarlo antes de
/// propagar un error de parseo de algo secreto.
fn leer_identidad(ruta: &str) -> Result<age::x25519::Identity, Error> {
    let mut texto = Zeroizing::new(String::new());
    File::open(ruta)
        .map_err(|e| format!("no pude abrir la identidad {ruta}: {e}"))?
        .read_to_string(&mut texto)
        .map_err(|e| format!("no pude leer la identidad {ruta}: {e}"))?;

    let linea = texto
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#'))
        .ok_or_else(|| Error::from(format!("{ruta} no tiene ninguna identidad adentro")))?;

    age::x25519::Identity::from_str(linea)
        .map_err(|e| Error::from(format!("{ruta} no tiene una identidad age valida: {e}")))
}

/// Convierte los `--para` en destinatarios x25519.
fn leer_destinatarios(para: &[String]) -> Result<Vec<age::x25519::Recipient>, Error> {
    para.iter()
        .map(|t| {
            age::x25519::Recipient::from_str(t.trim())
                .map_err(|e| Error::from(format!("{t:?} no es una clave publica age valida: {e}")))
        })
        .collect()
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
// Acolchado
// ---------------------------------------------------------------------------

/// Escritor que lleva la cuenta y, al cerrar, acolcha hasta el proximo multiplo.
///
/// Es un `Write` y no una funcion porque asi se enchufa en el medio del camino
/// que ya existe: el acolchado tiene que pasar ANTES del cifrado, dentro del
/// texto en claro, que es el unico lugar donde sirve de algo. Acolchar el sobre
/// ya cerrado no serviria: `age` verifica el largo y rechaza lo que sobra.
struct Rellenador<W: Write> {
    interior: W,
    bloque: u64,
    escritos: u64,
}

impl<W: Write> Rellenador<W> {
    fn nuevo(interior: W, bloque: u64) -> Self {
        Rellenador {
            interior,
            bloque,
            escritos: 0,
        }
    }

    /// Escribe la cola y devuelve el escritor de adentro.
    ///
    /// ISO/IEC 7816-4: un `0x80` y despues ceros. Siempre se agrega algo, aun si
    /// el contenido ya cae justo en el multiplo -ahi se agrega un bloque
    /// entero-, porque si el acolchado pudiera medir cero no habria forma de
    /// distinguir un contenido acolchado de uno que no lo esta.
    fn finalizar(mut self) -> Result<W, Error> {
        let objetivo = (self.escritos / self.bloque + 1) * self.bloque;
        let mut faltan = objetivo - self.escritos;

        self.interior.write_all(&[MARCA_RELLENO])?;
        faltan -= 1;

        let ceros = [0u8; 8192];
        while faltan > 0 {
            let n = faltan.min(ceros.len() as u64) as usize;
            self.interior.write_all(&ceros[..n])?;
            faltan -= n as u64;
        }
        Ok(self.interior)
    }
}

impl<W: Write> Write for Rellenador<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.interior.write(buf)?;
        self.escritos += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.interior.flush()
    }
}

/// Copia sacando la cola de acolchado, sin cargar el contenido entero.
///
/// El unico dato que no se puede emitir es el que todavia podria ser relleno, y
/// eso es exactamente el sufijo `0x80 0x00*` de lo leido hasta ahora. Se retiene
/// ese sufijo y se emite todo lo de mas atras; cuando llega un byte que lo
/// desmiente, el sufijo deja de ser candidato y sale.
///
/// Por eso el tope de `--bloque` es tambien el tope de memoria de esta funcion:
/// lo retenido nunca puede pasar el largo de un acolchado.
fn copiar_sin_relleno(origen: &mut dyn Read, destino: &mut dyn Write) -> Result<u64, Error> {
    let mut retenido: Vec<u8> = Vec::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut emitidos = 0u64;

    loop {
        let n = origen.read(&mut buf)?;
        if n == 0 {
            break;
        }
        retenido.extend_from_slice(&buf[..n]);

        // Donde arranca el sufijo `0x80 0x00*`: se camina hacia atras sobre los
        // ceros y se mira si justo antes hay una marca.
        let mut i = retenido.len();
        while i > 0 && retenido[i - 1] == 0x00 {
            i -= 1;
        }
        let corte = if i > 0 && retenido[i - 1] == MARCA_RELLENO {
            i - 1
        } else {
            retenido.len()
        };

        if corte > 0 {
            destino.write_all(&retenido[..corte])?;
            emitidos += corte as u64;
            retenido.drain(..corte);
        }

        if retenido.len() as u64 > BLOQUE_MAX {
            return Err(format!(
                "la cola de ceros pasa los {BLOQUE_MAX} bytes: ningun acolchado puede medir tanto"
            )
            .into());
        }
    }

    if retenido.is_empty() {
        return Err("esto no termina en acolchado: no encontre la marca 0x80".into());
    }
    Ok(emitidos)
}

// ---------------------------------------------------------------------------
// Las ordenes
// ---------------------------------------------------------------------------

/// Arma el cifrador segun con que se este cerrando.
///
/// `with_user_passphrase` no deja tocar el factor de trabajo, asi que cuando hay
/// `--trabajo` hay que armar el `scrypt::Recipient` a mano. Es lo mismo que hace
/// el atajo del crate, con un `set_work_factor` en el medio.
fn cifrar(
    opciones: &Opciones,
    clave: Option<SecretString>,
    mut origen: Box<dyn BufRead>,
) -> Result<(), Error> {
    let destino = abrir_destino(opciones)?;

    let cifrador = match clave {
        Some(c) => {
            let mut receptor = age::scrypt::Recipient::new(c);
            if let Some(n) = opciones.trabajo {
                receptor.set_work_factor(n);
            }
            age::Encryptor::with_recipients(std::iter::once(&receptor as &dyn age::Recipient))?
        }
        None => {
            let destinatarios = leer_destinatarios(&opciones.para)?;
            age::Encryptor::with_recipients(
                destinatarios.iter().map(|r| r as &dyn age::Recipient),
            )?
        }
    };

    let flujo = cifrador.wrap_output(destino)?;

    // El acolchado va adentro del cifrado, no afuera. Si el Rellenador
    // envolviera al sobre en vez de al contenido, `age` rechazaria los bytes de
    // mas al abrirlo.
    let copiados = match opciones.bloque {
        Some(b) => {
            let mut relleno = Rellenador::nuevo(flujo, b);
            let n = io::copy(&mut origen, &mut relleno)?;
            // Sin este finish el archivo queda truncado y no abre nunca mas. Es
            // el mismo tipo de error silencioso que mandarle la clave dos veces
            // a rar.
            let mut cerrado = relleno.finalizar()?.finish()?;
            cerrado.flush()?;
            n
        }
        None => {
            let mut flujo = flujo;
            let n = io::copy(&mut origen, &mut flujo)?;
            let mut cerrado = flujo.finish()?;
            // Y sin este flush explicito, un BufWriter que se cae solo se lleva
            // el ultimo bloque sin avisar: su Drop ignora el error de escritura.
            cerrado.flush()?;
            n
        }
    };

    informar("cerrado", opciones, copiados)
}

fn abrir(
    opciones: &Opciones,
    clave: Option<SecretString>,
    origen: Box<dyn BufRead>,
) -> Result<(), Error> {
    let nombre = opciones.nombre_entrada();

    // Fallar aca significa que el header no se pudo leer: sea lo que sea, no es
    // un sobre age.
    let descifrador = age::Decryptor::new_buffered(origen).map_err(|e| clasificar(e, &nombre))?;

    // Los dos caminos terminan en un `&dyn Identity`, que es lo unico que
    // `decrypt` mira. Que abajo haya scrypt o curva eliptica no se filtra hasta
    // aca: es la misma forma con dos rellenos.
    let identidad: Box<dyn age::Identity> = match clave {
        Some(c) => {
            let mut id = age::scrypt::Identity::new(c);
            if let Some(n) = opciones.max_trabajo {
                id.set_max_work_factor(n);
            }
            Box::new(id)
        }
        None => Box::new(leer_identidad(
            opciones
                .identidad
                .as_deref()
                .expect("validar() ya garantizo que hay identidad si no hay clave"),
        )?),
    };

    // Fallar aca es lo interesante: el sobre existe y esta bien armado, asi que
    // lo que puede estar mal es el secreto. `clasificar` separa ese caso.
    let mut flujo = descifrador
        .decrypt(std::iter::once(identidad.as_ref()))
        .map_err(|e| clasificar(e, &nombre))?;

    // El destino se abre despues de que el descifrado arranco, no antes: al
    // reves queda un archivo vacio en disco cada vez que alguien se equivoca de
    // clave.
    let mut destino = abrir_destino(opciones)?;
    let copiados = if opciones.desrellenar {
        copiar_sin_relleno(&mut flujo, &mut destino)?
    } else {
        io::copy(&mut flujo, &mut destino).map_err(|e| clasificar_copia(e, &nombre))?
    };
    destino.flush()?;

    informar("abierto", opciones, copiados)
}

/// Crea una identidad x25519 y avisa cual es su mitad publica.
///
/// La privada sale por el canal de datos y la publica por stderr, a proposito:
/// asi `sobre generar --stdout > mi.identidad` guarda solo lo que hay que
/// guardar, y la publica igual queda a la vista para copiarla al `--para`.
fn generar(opciones: &Opciones) -> Result<(), Error> {
    let identidad = age::x25519::Identity::generate();
    let publica = identidad.to_public();

    let mut destino = abrir_destino(opciones)?;
    writeln!(destino, "# clave publica: {publica}")?;
    writeln!(destino, "{}", identidad.to_string().expose_secret())?;
    destino.flush()?;

    eprintln!("clave publica: {publica}");
    match &opciones.salida {
        Some(ruta) => eprintln!("identidad privada guardada en {ruta} — no la compartas"),
        None => eprintln!("identidad privada por stdout — no la compartas"),
    }
    Ok(())
}

fn rellenar(opciones: &Opciones, mut origen: Box<dyn BufRead>) -> Result<(), Error> {
    let bloque = opciones
        .bloque
        .expect("validar() ya garantizo que rellenar tiene --bloque");
    let destino = abrir_destino(opciones)?;

    let mut relleno = Rellenador::nuevo(destino, bloque);
    let copiados = io::copy(&mut origen, &mut relleno)?;
    let mut cerrado = relleno.finalizar()?;
    cerrado.flush()?;

    informar("acolchado", opciones, copiados)
}

fn desrellenar(opciones: &Opciones, mut origen: Box<dyn BufRead>) -> Result<(), Error> {
    let mut destino = abrir_destino(opciones)?;
    let copiados = copiar_sin_relleno(&mut origen, &mut destino)?;
    destino.flush()?;

    informar("desacolchado", opciones, copiados)
}

/// El renglon de stderr que cuenta como salio. Nunca va a stdout: ahi van datos.
fn informar(verbo: &str, opciones: &Opciones, copiados: u64) -> Result<(), Error> {
    match &opciones.salida {
        Some(ruta) => {
            let bytes = std::fs::metadata(ruta)?.len();
            eprintln!("{verbo}: {ruta} ({bytes} bytes)");
        }
        None => eprintln!("{verbo}: {copiados} bytes de contenido -> stdout"),
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
    let (orden, opciones) = match parsear()? {
        Accion::Ejecutar(o, op) => (o, op),
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

    // Con --para o --identidad no hay passphrase, y entonces stdin no lleva
    // ningun secreto: queda entero para el contenido y el marco no hace falta.
    // Es la simplificacion mas linda que trajo x25519, y sale gratis.
    let usa_clave = match orden {
        Orden::Cifrar => opciones.para.is_empty(),
        Orden::Abrir => opciones.identidad.is_none(),
        _ => false,
    };

    // Un solo lector para todo stdin, y tiene que ser uno solo: con --stdin y
    // clave, la clave y el contenido viajan por el mismo cano.
    let mut entrada_estandar = io::stdin().lock();
    let clave = if usa_clave {
        let enmarcada = opciones.entrada.is_none();
        Some(leer_clave(
            &mut entrada_estandar,
            opciones.utf16le,
            enmarcada,
        )?)
    } else {
        None
    };

    match orden {
        Orden::Generar => generar(&opciones),
        Orden::Cifrar => cifrar(&opciones, clave, abrir_origen(&opciones, entrada_estandar)?),
        Orden::Abrir => abrir(&opciones, clave, abrir_origen(&opciones, entrada_estandar)?),
        Orden::Rellenar => rellenar(&opciones, abrir_origen(&opciones, entrada_estandar)?),
        Orden::Desrellenar => desrellenar(&opciones, abrir_origen(&opciones, entrada_estandar)?),
    }
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un `&[u8]` ya es BufRead, asi que el marco se puede probar sin proceso,
    /// sin pipe y sin archivo.
    fn leer(entrada: &[u8], utf16le: bool, enmarcada: bool) -> Result<(String, Vec<u8>), Error> {
        let mut lector = entrada;
        let clave = leer_clave(&mut lector, utf16le, enmarcada)?;
        let mut resto = Vec::new();
        lector.read_to_end(&mut resto).unwrap();
        Ok((clave.expose_secret().to_string(), resto))
    }

    // --- el marco --------------------------------------------------------

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
        let utf16: Vec<u8> = clave.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();

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

    // --- codigos de salida -----------------------------------------------

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
        assert_eq!(
            Error::TrabajoExcesivo {
                origen: "x".into(),
                pide: 22,
                acepta: 18
            }
            .codigo(),
            SALIDA_TRABAJO
        );
    }

    #[test]
    fn clave_incorrecta_se_distingue_de_sobre_invalido() {
        // El nombre engania: NoMatchingKeys NO es "la clave esta mal", es "este
        // sobre no esta cerrado de la forma con la que lo estoy abriendo". Si
        // esto se invierte, el codigo 2 deja de significar lo que dice la ayuda.
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
    }

    #[test]
    fn trabajo_excesivo_tiene_su_propio_codigo_y_dice_los_numeros() {
        let e = clasificar(
            age::DecryptError::ExcessiveWork {
                required: 22,
                target: 18,
            },
            "notas.sobre",
        );
        assert_eq!(e.codigo(), SALIDA_TRABAJO);
        let m = e.to_string();
        // Los dos numeros tienen que estar: son los que hacen falta para elegir
        // un --max-trabajo sin adivinar.
        assert!(m.contains("22"), "{m}");
        assert!(m.contains("18"), "{m}");
        assert!(m.contains("16x"), "{m}");
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

    // --- tamanos y factores ----------------------------------------------

    #[test]
    fn los_tamanos_se_entienden_con_y_sin_sufijo() {
        assert_eq!(parsear_tamano("4096").unwrap(), 4096);
        assert_eq!(parsear_tamano("4K").unwrap(), 4096);
        assert_eq!(parsear_tamano("4k").unwrap(), 4096);
        assert_eq!(parsear_tamano("4KB").unwrap(), 4096);
        assert_eq!(parsear_tamano("4KiB").unwrap(), 4096);
        assert_eq!(parsear_tamano("1M").unwrap(), 1024 * 1024);
        assert!(parsear_tamano("0").is_err());
        assert!(parsear_tamano("banana").is_err());
        assert!(parsear_tamano("1G").is_err(), "1G pasa el tope de 16M");
    }

    #[test]
    fn el_factor_de_trabajo_se_valida_antes_de_que_age_panickee() {
        // `set_work_factor` hace assert!(0 < n && n < 64). Validar aca convierte
        // un panic en un mensaje.
        assert_eq!(parsear_trabajo("18").unwrap(), 18);
        assert!(parsear_trabajo("0").is_err());
        assert!(parsear_trabajo("64").is_err());
        assert!(parsear_trabajo("-1").is_err());
        assert!(parsear_trabajo("ocho").is_err());
    }

    // --- acolchado -------------------------------------------------------

    fn acolchar(datos: &[u8], bloque: u64) -> Vec<u8> {
        let mut r = Rellenador::nuevo(Vec::new(), bloque);
        r.write_all(datos).unwrap();
        r.finalizar().unwrap()
    }

    fn desacolchar(datos: &[u8]) -> Result<Vec<u8>, Error> {
        let mut salida = Vec::new();
        copiar_sin_relleno(&mut &datos[..], &mut salida)?;
        Ok(salida)
    }

    #[test]
    fn acolchar_lleva_al_proximo_multiplo() {
        assert_eq!(acolchar(b"hola", 16).len(), 16);
        assert_eq!(acolchar(&[0u8; 15], 16).len(), 16);
        assert_eq!(acolchar(&[0u8; 17], 16).len(), 32);
    }

    #[test]
    fn un_contenido_que_ya_cae_justo_igual_se_acolcha() {
        // Si el acolchado pudiera medir cero no habria forma de saber si hay
        // acolchado o no. Un multiplo exacto se lleva un bloque entero.
        let salida = acolchar(&[0u8; 16], 16);
        assert_eq!(salida.len(), 32);
        assert_eq!(salida[16], MARCA_RELLENO);
    }

    #[test]
    fn la_vuelta_completa_devuelve_lo_mismo() {
        for largo in [0usize, 1, 15, 16, 17, 100, 4095, 4096, 4097] {
            let datos: Vec<u8> = (0..largo).map(|i| (i % 251) as u8).collect();
            let ida = acolchar(&datos, 4096);
            assert_eq!(ida.len() % 4096, 0, "largo {largo}");
            assert_eq!(desacolchar(&ida).unwrap(), datos, "largo {largo}");
        }
    }

    #[test]
    fn contenido_que_termina_en_marca_y_ceros_no_confunde() {
        // El caso que rompe una implementacion ingenua: el contenido REAL
        // termina igual que un acolchado. El sufijo retenido tiene que ceder
        // cuando llega el acolchado de verdad.
        let datos = b"antes\x80\x00\x00\x00".to_vec();
        let ida = acolchar(&datos, 16);
        assert_eq!(desacolchar(&ida).unwrap(), datos);
    }

    #[test]
    fn contenido_de_puros_ceros_vuelve_entero() {
        let datos = vec![0u8; 300];
        let ida = acolchar(&datos, 64);
        assert_eq!(desacolchar(&ida).unwrap(), datos);
    }

    #[test]
    fn contenido_que_termina_en_ceros_sin_marca_vuelve_entero() {
        let datos = b"hola\x00\x00\x00".to_vec();
        let ida = acolchar(&datos, 8);
        assert_eq!(desacolchar(&ida).unwrap(), datos);
    }

    #[test]
    fn desacolchar_algo_sin_marca_falla() {
        let e = desacolchar(b"sin ninguna marca").unwrap_err();
        assert_eq!(e.codigo(), SALIDA_ERROR);
        assert!(e.to_string().contains("0x80"));
    }

    #[test]
    fn el_acolchado_cruza_los_bordes_de_lectura() {
        // El buffer interno es de 64 KiB: esto obliga a que el sufijo retenido
        // sobreviva de una lectura a la siguiente.
        let datos = vec![7u8; 200_000];
        let ida = acolchar(&datos, 1024);
        assert_eq!(desacolchar(&ida).unwrap(), datos);
    }

    // --- identidades -----------------------------------------------------

    #[test]
    fn una_identidad_generada_va_y_vuelve_de_texto() {
        let id = age::x25519::Identity::generate();
        let publica = id.to_public().to_string();
        let privada = id.to_string().expose_secret().to_string();

        assert!(publica.starts_with("age1"), "{publica}");
        assert!(privada.starts_with("AGE-SECRET-KEY-1"), "publica: {publica}");

        assert!(age::x25519::Recipient::from_str(&publica).is_ok());
        assert!(age::x25519::Identity::from_str(&privada).is_ok());
    }

    #[test]
    fn los_destinatarios_mal_escritos_se_rechazan_temprano() {
        assert!(leer_destinatarios(&["no-es-una-clave".to_string()]).is_err());
        // Una identidad PRIVADA donde va una publica tambien tiene que fallar:
        // el prefijo es distinto y bech32 lo nota.
        let id = age::x25519::Identity::generate();
        let privada = id.to_string().expose_secret().to_string();
        assert!(leer_destinatarios(&[privada]).is_err());
    }
}
