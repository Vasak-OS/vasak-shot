//! El IPC de wayfire: cuatro bytes con el largo, después el JSON.
//!
//! Vive acá y no dentro de quien lo usa porque ya hay dos que preguntan —dónde
//! está el puntero y qué ventanas hay— y en el taller hay cuatro copias más de
//! este mismo enmarcado, en vasak-desktop, vasak-resonance y
//! vasak-press-and-hold. Sumar la quinta adentro de una función que además
//! parsea su respuesta sería garantizar que la sexta también se escriba a mano.
//!
//! **Ningún error se informa hacia arriba, a propósito.** No hay socket, no
//! contesta, contesta otra cosa: para quien llama significan todos lo mismo, que
//! es «no se puede saber». Quien pregunta tiene que tener un camino sin wayfire,
//! y tenerlo es lo que hace que esto pueda callarse.

use std::io::{Read, Write};
use std::path::Path;

/// Cuánto se espera a cada operación del socket.
///
/// Sin esto, un compositor que deja de contestar cuelga la captura para
/// siempre: `read_exact` bloquea y el camino alternativo no se ejecuta nunca.
/// Un segundo es larguísimo para una consulta local; lo que importa es que
/// exista.
const ESPERA: std::time::Duration = std::time::Duration::from_secs(1);

/// Techo del cuerpo de la respuesta.
///
/// El largo lo dice el otro extremo, y `WAYFIRE_SOCKET` es una variable de
/// entorno: quien la controle puede anunciar un marco de gigabytes y hacer que
/// esto reserve memoria hasta morirse. Una lista de ventanas real son unos
/// kilobytes.
const TECHO: u32 = 256 * 1024;

/// La ruta del socket de esta sesión, si la hay.
pub fn socket() -> Option<std::ffi::OsString> {
    std::env::var_os("WAYFIRE_SOCKET")
}

/// Llama a un método sin argumentos y devuelve la respuesta ya parseada.
///
/// La ruta entra por argumento y no se lee acá para poder probarlo sin tocar el
/// entorno del proceso: `cargo test` corre en hilos que lo comparten, así que
/// una prueba que lo modifica le cambia el mundo a las otras.
pub fn pedir(ruta: impl AsRef<Path>, metodo: &str) -> Option<serde_json::Value> {
    let mut sock = std::os::unix::net::UnixStream::connect(ruta).ok()?;
    sock.set_read_timeout(Some(ESPERA)).ok()?;
    sock.set_write_timeout(Some(ESPERA)).ok()?;

    let pedido = serde_json::json!({ "method": metodo, "data": {} }).to_string();
    sock.write_all(&(pedido.len() as u32).to_ne_bytes()).ok()?;
    sock.write_all(pedido.as_bytes()).ok()?;

    let mut largo = [0u8; 4];
    sock.read_exact(&mut largo).ok()?;
    let largo = u32::from_ne_bytes(largo);
    if largo > TECHO {
        return None;
    }

    let mut cuerpo = vec![0u8; largo as usize];
    sock.read_exact(&mut cuerpo).ok()?;
    serde_json::from_slice(&cuerpo).ok()
}
