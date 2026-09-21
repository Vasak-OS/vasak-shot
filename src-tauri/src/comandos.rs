//! Lo que el frontend puede pedir.

use crate::captura::{self, Region, Salida};
use crate::destino;
use crate::preferencias::{self as prefs, AlSoltar};
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

/// Dónde quedó el selector: su salida, y el layout del que forma parte.
///
/// Las dos cosas juntas son lo que permite traducir la selección — la ventana
/// cubre **una** pantalla y la captura contiene **todas**—, así que viajan juntas
/// en lugar de como una tupla que nadie puede leer.
#[derive(Debug, Clone, Copy)]
struct Geometria {
    salida: Salida,
    /// El rectángulo que abarcan todas las salidas, **con su origen**: `grim`
    /// compone desde ahí, y no siempre es (0, 0).
    layout: Salida,
}

/// La geometría del selector, anotada desde `setup` cuando ya existe la ventana.
fn geometria() -> &'static Mutex<Option<Geometria>> {
    static GEOMETRIA: OnceLock<Mutex<Option<Geometria>>> = OnceLock::new();
    GEOMETRIA.get_or_init(|| Mutex::new(None))
}

/// Anota la salida que el selector tapó y el tamaño del layout entero.
pub fn recordar_salida(salida: Salida, layout: Salida) {
    if let Ok(mut guardia) = geometria().lock() {
        *guardia = Some(Geometria { salida, layout });
    }
}

/// Guarda la captura recién tomada para que el selector la muestre.
pub fn recordar(c: captura::Captura) {
    if let Ok(mut guardia) = pendiente().lock() {
        *guardia = Some(c);
    }
}

/// Lo que el selector necesita para dibujarse.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Lienzo {
    /// La ruta del PNG congelado. El frontend la convierte con `convertFileSrc`:
    /// `file://` no está permitido por la política de contenido, y está bien que
    /// no lo esté.
    pub ruta: String,
    /// El tamaño de la captura **entera**, con todas las salidas.
    pub ancho: u32,
    pub alto: u32,
    /// La salida que el selector está tapando, **relativa al origen del layout**.
    ///
    /// El frontend la necesita para mostrar **su pedazo** de la captura. Sin esto
    /// estiraba la composición completa dentro de una pantalla, y con dos
    /// monitores apilados eso significa verlos los dos achatados a la mitad.
    ///
    /// Relativa y no absoluta a propósito: el origen del layout puede ser
    /// negativo, y así el frontend usa estos números tal cual para desplazar el
    /// fondo, sin tener que saber nada del layout.
    pub salida: Salida,
    /// Cuántos píxeles de la captura hay por unidad del layout, por eje.
    pub escala_x: f64,
    pub escala_y: f64,
}

#[tauri::command]
pub fn lienzo() -> Result<Lienzo, String> {
    let guardia = pendiente()
        .lock()
        .map_err(|_| "el estado de la captura quedó envenenado".to_string())?;
    let c = guardia
        .as_ref()
        .ok_or_else(|| "todavía no hay ninguna captura".to_string())?;
    let (salida, escala) = salida_y_escala(c.ancho, c.alto);

    Ok(Lienzo {
        ruta: c.ruta.to_string_lossy().into_owned(),
        ancho: c.ancho,
        alto: c.alto,
        salida,
        escala_x: escala.0,
        escala_y: escala.1,
    })
}

/// La salida del selector y la escala de la captura, con respaldo razonable.
///
/// Si la geometría no se pudo averiguar —`setup` no encontró el puntero, o la
/// ventana no expuso su GtkWindow— se supone **una sola pantalla del tamaño de la
/// captura**. Eso es exactamente lo que había antes de este arreglo y funciona
/// bien en ese caso, que es el más común; lo que no hace es inventar un origen.
fn salida_y_escala(ancho: u32, alto: u32) -> (Salida, (f64, f64)) {
    let anotada = geometria().lock().ok().and_then(|g| *g);
    match anotada {
        // Relativa al origen del layout: es el espacio en el que `grim` compone.
        Some(g) => (
            g.salida.relativa_a(g.layout),
            captura::escala_de((ancho, alto), g.layout),
        ),
        None => (Salida::entera(ancho as i32, alto as i32), (1.0, 1.0)),
    }
}

/// Lleva la selección de la ventana a píxeles de la captura.
///
/// Todos los comandos pasan por acá: `guardar`, `copiar` y `guardar_y_copiar`. Que
/// sea uno solo es a propósito — cuando la traducción faltaba, faltaba en los tres
/// y había que arreglarla tres veces.
fn traducir(region: Region) -> Result<Region, String> {
    let (ancho, alto) = {
        let guardia = pendiente()
            .lock()
            .map_err(|_| "el estado de la captura quedó envenenado".to_string())?;
        let c = guardia
            .as_ref()
            .ok_or_else(|| "todavía no hay ninguna captura".to_string())?;
        (c.ancho, c.alto)
    };
    let (salida, escala) = salida_y_escala(ancho, alto);
    Ok(region.en_la_captura(salida, escala))
}

/// Las preferencias, más la carpeta que de verdad se está usando.
///
/// Las dos cosas en una respuesta: el panel necesita mostrar adónde van las
/// capturas ahora mismo, y eso no es la preferencia —que puede estar vacía— sino
/// el resultado de resolverla. Preguntarlo por separado abriría la puerta a que
/// una de las dos llegue vieja.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ajustes {
    pub al_soltar: AlSoltar,
    /// La carpeta elegida a mano, o nula si no se eligió ninguna.
    pub carpeta: Option<String>,
    /// Dónde van las capturas ahora mismo, ya resuelto.
    pub carpeta_efectiva: String,
}

/// Arma la respuesta del panel a partir de las preferencias dadas.
fn ajustes_de(preferencias: prefs::Preferencias) -> Result<Ajustes, String> {
    Ok(Ajustes {
        al_soltar: preferencias.al_soltar,
        carpeta: preferencias
            .carpeta
            .as_ref()
            .map(|c| c.to_string_lossy().into_owned()),
        carpeta_efectiva: destino::carpeta_sin_crear()?.to_string_lossy().into_owned(),
    })
}

/// Lo que el panel de preferencias necesita para dibujarse.
#[tauri::command]
pub fn ajustes() -> Result<Ajustes, String> {
    ajustes_de(prefs::leer())
}

/// Guarda las preferencias y devuelve cómo quedaron.
///
/// Devuelve en lugar de contestar que sí: la carpeta que se escribió no es
/// necesariamente la que se guardó —`~` se expande, los espacios se recortan— y
/// el panel tiene que mostrar lo que quedó, no lo que se tecleó.
///
/// La carpeta se **crea** acá, no al guardar la primera captura. Una ruta mal
/// escrita tiene que fallar mientras el panel está abierto y se la puede
/// corregir, no media hora después cuando lo que se quería era capturar algo.
#[tauri::command]
pub fn guardar_ajustes(al_soltar: AlSoltar, carpeta: Option<String>) -> Result<Ajustes, String> {
    let home = std::env::var("HOME").map_err(|_| "no hay HOME".to_string())?;
    let carpeta = match carpeta {
        Some(bruta) => prefs::carpeta_escrita(&bruta, &home)?,
        None => None,
    };

    if let Some(elegida) = carpeta.as_ref() {
        std::fs::create_dir_all(elegida)
            .map_err(|e| format!("no se pudo crear {}: {e}", elegida.display()))?;
    }

    let preferencias = prefs::Preferencias { al_soltar, carpeta };
    prefs::escribir(&preferencias)?;
    ajustes_de(preferencias)
}

/// Recorta a la región elegida y devuelve el archivo final.
fn producir(region: Region) -> Result<PathBuf, String> {
    let region = traducir(region)?;
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
    let region = traducir(region)?;
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

    let temporal =
        std::env::temp_dir().join(format!("vasak-shot-copia-{}.png", std::process::id()));
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
