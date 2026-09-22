//! Lo que el frontend puede pedir.

use crate::anotada;
use crate::captura::{self, Monitor, Region, Salida};
use crate::destino;
use crate::preferencias::{self as prefs, AlSoltar};
use crate::ventanas::Ventana;
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

/// Las ventanas que había cuando se tomó la captura, en coordenadas del layout.
///
/// Se anotan al arrancar, igual que la captura y por la misma razón: lo que se
/// señala tiene que coincidir con lo que la imagen congelada muestra. Un mapa
/// pedido después sería el de un escritorio que ya cambió.
fn ventanas_de_entonces() -> &'static Mutex<Vec<Ventana>> {
    static VENTANAS: OnceLock<Mutex<Vec<Ventana>>> = OnceLock::new();
    VENTANAS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Anota las ventanas que había al capturar.
pub fn recordar_ventanas(ventanas: Vec<Ventana>) {
    if let Ok(mut guardia) = ventanas_de_entonces().lock() {
        *guardia = ventanas;
    }
}

/// Las ventanas que se ven en esta pantalla, en coordenadas del selector.
///
/// Una lista vacía no es un error: sin wayfire —otro compositor, el IPC
/// apagado— no se puede señalar una ventana y queda el arrastre, que es lo que
/// había antes. El selector apaga el resaltado y sigue.
#[tauri::command]
pub fn ventanas() -> Vec<Ventana> {
    let Ok(guardia) = ventanas_de_entonces().lock() else {
        return Vec::new();
    };
    let Some(salida) = geometria().lock().ok().and_then(|g| *g).map(|g| g.salida) else {
        // Sin geometría no se sabe qué pantalla tapa el selector, y traducir
        // con una supuesta pondría los recuadros donde no va ninguno.
        return Vec::new();
    };
    crate::ventanas::en_la_pantalla(&guardia, salida)
}

/// Las salidas que había cuando se tomó la captura.
///
/// Las mismas que entraron en la imagen, y por eso van anotadas y no
/// preguntadas de nuevo: si alguien desenchufa un monitor mientras el selector
/// está abierto, lo que se puede recortar sigue siendo lo que la captura tiene.
fn salidas_de_entonces() -> &'static Mutex<Vec<Monitor>> {
    static SALIDAS: OnceLock<Mutex<Vec<Monitor>>> = OnceLock::new();
    SALIDAS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Anota las salidas que entraron en la captura.
pub fn recordar_salidas(salidas: Vec<Monitor>) {
    if let Ok(mut guardia) = salidas_de_entonces().lock() {
        *guardia = salidas;
    }
}

/// Las pantallas, separadas en la que se está mirando y las demás.
///
/// Separadas y no una lista con una marca adentro porque el selector las usa
/// para dos cosas distintas: la de acá se resalta y se nombra, y las otras se
/// ofrecen en un menú. Una lista que hay que filtrar en los dos lugares es una
/// condición repetida que en algún momento se escribe al revés.
#[derive(Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Salidas {
    /// La que el selector está tapando, ya en (0, 0). Nula si no se sabe cuál es.
    pub actual: Option<Monitor>,
    /// Las demás, en coordenadas de esta pantalla: las de arriba y las de la
    /// izquierda quedan en negativo, que es exactamente lo que
    /// `Region::en_la_captura` deshace al traducir.
    pub otras: Vec<Monitor>,
}

/// Reparte las salidas en la que ocupa `actual` y el resto.
///
/// Todas se llevan a coordenadas del selector. Es lo que permite pedir **otra
/// pantalla** sin mover el puntero hasta ella: la región viaja en el mismo
/// espacio que una selección hecha con el ratón, y el backend la traduce con la
/// misma cuenta de siempre en lugar de con un camino aparte.
fn repartir(salidas: &[Monitor], actual: Salida) -> Salidas {
    let mut resultado = Salidas::default();
    for salida in salidas {
        let relativa = salida.relativo_a(actual);
        if salida.area() == actual {
            resultado.actual = Some(relativa);
        } else {
            resultado.otras.push(relativa);
        }
    }
    resultado
}

/// Las pantallas que hay, en coordenadas del selector.
#[tauri::command]
pub fn salidas() -> Salidas {
    let Ok(guardia) = salidas_de_entonces().lock() else {
        return Salidas::default();
    };
    let Some(actual) = geometria().lock().ok().and_then(|g| *g).map(|g| g.salida) else {
        // Sin saber qué pantalla tapa el selector no se puede traducir ninguna,
        // y ofrecerlas en coordenadas del layout haría que elegir una recortara
        // de otra.
        return Salidas::default();
    };
    repartir(&guardia, actual)
}

/// Vuelve a capturar dentro de unos segundos, con esta ventana ya cerrada.
///
/// Lanza **otro proceso** en lugar de capturar de nuevo acá. No es un rodeo: la
/// captura se toma al arrancar, antes de que exista ninguna ventana, y eso es
/// justamente lo que hace falta para fotografiar un menú abierto. Reusar este
/// proceso significaría capturar con el selector ya montado, o sea con una
/// ventana tapando lo que se quiere.
///
/// Quien cierra esta ventana es el frontend, en cuanto esto contesta.
#[tauri::command]
pub fn recapturar(retardo: u64) -> Result<(), String> {
    if retardo == 0 || retardo > crate::retardo::TECHO {
        return Err(format!(
            "un retardo de {retardo} s no sirve para volver a capturar; van de 1 a {}",
            crate::retardo::TECHO
        ));
    }

    let exe = std::env::current_exe()
        .map_err(|e| format!("no se pudo saber qué ejecutable es éste: {e}"))?;

    std::process::Command::new(exe)
        .args(["--retardo", &retardo.to_string()])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("no se pudo volver a lanzar la captura: {e}"))?;

    Ok(())
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
        // Que exista no alcanza: `create_dir_all` se conforma con una carpeta
        // ajena o de sólo lectura, y ahí el problema saldría recién al guardar
        // la primera captura, con el panel ya cerrado.
        prefs::probar_escritura(elegida)?;
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

    // Creado en exclusiva y en `0o600`: `/tmp` lo comparte toda la máquina, y
    // una captura que se pidió **sólo copiar** no puede quedar un rato legible
    // por otra cuenta. `recortar` escribe sobre el archivo que ya existe y no le
    // cambia los permisos.
    let temporal = anotada::temporal("copia")?;
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

/// Los bytes crudos de un pedido, o un error que se entienda.
///
/// El cuerpo llega crudo y no adentro de un JSON: son megabytes, y como lista
/// de números costarían un orden de magnitud más. Ver `anotada`.
fn bytes_de<'a>(pedido: &'a tauri::ipc::Request<'a>) -> Result<&'a [u8], String> {
    match pedido.body() {
        tauri::ipc::InvokeBody::Raw(bytes) => Ok(bytes),
        tauri::ipc::InvokeBody::Json(_) => {
            Err("la captura anotada tiene que viajar como bytes".to_string())
        }
    }
}

/// Guarda la captura ya compuesta por el selector.
#[tauri::command]
pub fn guardar_anotada(pedido: tauri::ipc::Request<'_>) -> Result<String, String> {
    let bytes = bytes_de(&pedido)?;
    let final_ = destino::carpeta()?.join(destino::nombre_de_ahora());
    anotada::escribir(bytes, &final_)?;
    avisar(&final_);
    Ok(final_.to_string_lossy().into_owned())
}

/// Copia la captura ya compuesta, sin dejar archivo en la carpeta.
#[tauri::command]
pub fn copiar_anotada(pedido: tauri::ipc::Request<'_>) -> Result<(), String> {
    let bytes = bytes_de(&pedido)?;
    let temporal = anotada::temporal("anotada")?;
    anotada::escribir(bytes, &temporal)?;
    let resultado = captura::copiar_al_portapapeles(&temporal);
    let _ = std::fs::remove_file(&temporal);
    resultado
}

/// Guarda **y** copia la captura ya compuesta.
#[tauri::command]
pub fn guardar_y_copiar_anotada(pedido: tauri::ipc::Request<'_>) -> Result<String, String> {
    let bytes = bytes_de(&pedido)?;
    let final_ = destino::carpeta()?.join(destino::nombre_de_ahora());
    anotada::escribir(bytes, &final_)?;
    // Si el portapapeles falla, la captura ya está guardada: se informa el
    // archivo igual en lugar de perder las dos cosas por una.
    if let Err(e) = captura::copiar_al_portapapeles(&final_) {
        eprintln!("vasak-shot: no se pudo copiar al portapapeles: {e}");
    }
    avisar(&final_);
    Ok(final_.to_string_lossy().into_owned())
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

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Dos monitores de 1920x1080 apilados en vertical, como los de la máquina
    /// donde se encontró el bug del recorte.
    fn apilados() -> Vec<Monitor> {
        vec![
            Monitor::nuevo(
                "DP-1",
                Salida {
                    x: 0,
                    y: 0,
                    ancho: 1920,
                    alto: 1080,
                },
            ),
            Monitor::nuevo(
                "HDMI-A-1",
                Salida {
                    x: 0,
                    y: 1080,
                    ancho: 1920,
                    alto: 1080,
                },
            ),
        ]
    }

    #[test]
    fn la_pantalla_del_selector_queda_en_el_origen() {
        // Es la que el frontend dibuja con su propio tamaño: si no quedara en
        // (0, 0), el resaltado de «esta pantalla entera» saldría corrido.
        let salidas = apilados();
        let repartidas = repartir(&salidas, salidas[1].area());

        assert_eq!(
            repartidas.actual,
            Some(Monitor::nuevo(
                "HDMI-A-1",
                Salida {
                    x: 0,
                    y: 0,
                    ancho: 1920,
                    alto: 1080
                }
            ))
        );
    }

    #[test]
    fn la_otra_pantalla_queda_donde_el_layout_la_tiene() {
        // Estando en la de abajo, la de arriba cae en negativo. Ese número es el
        // que hace que pedirla recorte de la mitad de arriba de la captura y no
        // de la de abajo.
        let salidas = apilados();
        let repartidas = repartir(&salidas, salidas[1].area());

        assert_eq!(repartidas.otras.len(), 1);
        assert_eq!(repartidas.otras[0].nombre, "DP-1");
        assert_eq!((repartidas.otras[0].x, repartidas.otras[0].y), (0, -1080));
    }

    #[test]
    fn las_dos_no_se_pisan_y_cubren_la_captura_entera() {
        let salidas = apilados();
        let layout = captura::layout_de(&salidas);

        // Cubren todo: la suma de las áreas es la del encuadre.
        let suma: i64 = salidas
            .iter()
            .map(|s| i64::from(s.ancho) * i64::from(s.alto))
            .sum();
        assert_eq!(suma, i64::from(layout.ancho) * i64::from(layout.alto));

        // Y no se pisan: la de abajo empieza donde termina la de arriba.
        assert_eq!(salidas[0].y + salidas[0].alto, salidas[1].y);
    }

    #[test]
    fn sin_saber_en_cual_esta_no_se_marca_ninguna() {
        // Una geometría que no es la de ninguna salida —un monitor
        // desenchufado, una ventana que no se pudo ubicar— tiene que dejar
        // `actual` vacía en lugar de elegir cualquiera: el frontend apaga el
        // resaltado y sigue, que es lo que hace sin salidas.
        let salidas = apilados();
        let repartidas = repartir(
            &salidas,
            Salida {
                x: 500,
                y: 500,
                ancho: 800,
                alto: 600,
            },
        );

        assert_eq!(repartidas.actual, None);
        assert_eq!(repartidas.otras.len(), 2);
    }

    #[test]
    fn un_retardo_de_cero_no_vuelve_a_lanzar_nada() {
        // Sería lanzar un proceso que captura en el acto, o sea con el selector
        // todavía en pantalla: la foto saldría de la propia herramienta.
        assert!(recapturar(0).is_err());
        assert!(recapturar(crate::retardo::TECHO + 1).is_err());
    }
}
