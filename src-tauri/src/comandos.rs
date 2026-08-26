//! Lo que el frontend puede pedir.

use crate::captura::{self, Region};
use crate::destino;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// La captura que está esperando a que se elija una región.
///
/// Vive en el proceso y no en el frontend porque la imagen puede pesar varios
/// megabytes: lo que cruza el IPC es la ruta, no los píxeles. La misma regla que
/// en el gestor de archivos con las miniaturas.
fn pendiente() -> &'static Mutex<Option<captura::Captura>> {
    static PENDIENTE: OnceLock<Mutex<Option<captura::Captura>>> = OnceLock::new();
    PENDIENTE.get_or_init(|| Mutex::new(None))
}

/// Guarda la captura recién tomada para que el selector la muestre.
pub fn recordar(c: captura::Captura) {
    if let Ok(mut guardia) = pendiente().lock() {
        *guardia = Some(c);
    }
}

/// Lo que el selector necesita para dibujarse.
#[derive(serde::Serialize)]
pub struct Lienzo {
    /// La ruta del PNG congelado. El frontend la convierte con `convertFileSrc`:
    /// `file://` no está permitido por la política de contenido, y está bien que
    /// no lo esté.
    pub ruta: String,
    pub ancho: u32,
    pub alto: u32,
}

#[tauri::command]
pub fn lienzo() -> Result<Lienzo, String> {
    let guardia = pendiente()
        .lock()
        .map_err(|_| "el estado de la captura quedó envenenado".to_string())?;
    let c = guardia
        .as_ref()
        .ok_or_else(|| "todavía no hay ninguna captura".to_string())?;
    Ok(Lienzo {
        ruta: c.ruta.to_string_lossy().into_owned(),
        ancho: c.ancho,
        alto: c.alto,
    })
}

/// Recorta a la región elegida y devuelve el archivo final.
fn producir(region: Region) -> Result<PathBuf, String> {
    let origen = {
        let guardia = pendiente()
            .lock()
            .map_err(|_| "el estado de la captura quedó envenenado".to_string())?;
        guardia
            .as_ref()
            .ok_or_else(|| "todavía no hay ninguna captura".to_string())?
            .ruta
            .clone()
    };

    let final_ = destino::carpeta()?.join(destino::nombre_de_ahora());
    captura::recortar(&origen, region, &final_)?;
    Ok(final_)
}

/// Guarda la región elegida en la carpeta de capturas.
#[tauri::command]
pub fn guardar(region: Region) -> Result<String, String> {
    let final_ = producir(region)?;
    avisar(&final_);
    Ok(final_.to_string_lossy().into_owned())
}

/// Copia la región elegida al portapapeles, sin dejar archivo en la carpeta.
///
/// El recorte va a un temporal: quien copia quiere pegar, no acumular archivos
/// que después hay que borrar a mano.
#[tauri::command]
pub fn copiar(region: Region) -> Result<(), String> {
    let origen = {
        let guardia = pendiente()
            .lock()
            .map_err(|_| "el estado de la captura quedó envenenado".to_string())?;
        guardia
            .as_ref()
            .ok_or_else(|| "todavía no hay ninguna captura".to_string())?
            .ruta
            .clone()
    };

    let temporal = std::env::temp_dir().join(format!("vasak-shot-copia-{}.png", std::process::id()));
    captura::recortar(&origen, region, &temporal)?;
    let resultado = captura::copiar_al_portapapeles(&temporal);
    let _ = std::fs::remove_file(&temporal);
    resultado
}

/// Avisa que la captura quedó guardada, con la miniatura.
///
/// Por `notify-send` y no por la API de notificaciones de Tauri: el aviso tiene
/// que aparecer **después** de que esta ventana se cierre, y una notificación
/// emitida por un proceso que está terminando puede irse con él. El demonio de
/// notificaciones del escritorio la recibe y la muestra por su cuenta.
///
/// Si falla, no se dice nada más: la captura ya está guardada y el aviso es un
/// lujo, no el resultado.
fn avisar(ruta: &std::path::Path) {
    let _ = std::process::Command::new("notify-send")
        .args([
            "--app-name=vasak-shot",
            "--icon",
            &ruta.to_string_lossy(),
            "Captura guardada",
        ])
        .arg(
            ruta.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        )
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

/// Guarda **y** copia, que es lo que se quiere casi siempre.
#[tauri::command]
pub fn guardar_y_copiar(region: Region) -> Result<String, String> {
    let final_ = producir(region)?;
    // Si el portapapeles falla, la captura ya está guardada: se informa el
    // archivo igual en lugar de perder las dos cosas por una.
    if let Err(e) = captura::copiar_al_portapapeles(&final_) {
        eprintln!("vasak-shot: no se pudo copiar al portapapeles: {e}");
    }
    avisar(&final_);
    Ok(final_.to_string_lossy().into_owned())
}
