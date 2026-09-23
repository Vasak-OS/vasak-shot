//! Lo que el frontend puede pedir.

use crate::anotada;
use crate::aviso::{self, Textos};
use crate::captura::{self, Monitor, Region, Salida};
use crate::destino;
use crate::pantalla::Copia;
use crate::preferencias::{self as prefs, AlSoltar};
use crate::subida::{self, Destino};
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
    /// Cuántos píxeles de verdad tiene **esta** pantalla por unidad del layout.
    ///
    /// Casi siempre es la misma que la del lienzo, y con una sola pantalla lo es
    /// siempre. Se separan porque el lienzo compone a la escala **mayor** de
    /// todas: en una pantalla de menor escala, la del lienzo dice el doble de
    /// píxeles de los que esa pantalla tiene de verdad.
    ///
    /// La usa el lienzo de anotación para armarse del tamaño que va a tener el
    /// archivo. Sin esto, anotar una captura en la pantalla de menor escala
    /// devolvía una imagen del doble de tamaño que la misma captura sin anotar.
    pub escala_propia_x: f64,
    pub escala_propia_y: f64,
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

    // La de esta pantalla, si es una de las que el lienzo estiró. Si no, la del
    // lienzo ya es la suya.
    let propia = geometria()
        .lock()
        .ok()
        .and_then(|g| *g)
        .and_then(|g| {
            c.copias
                .iter()
                .find(|copia| copia.salida.area() == g.salida)
                .and_then(|copia| copia.escala())
        })
        .unwrap_or(escala);

    Ok(Lienzo {
        ancho: c.ancho,
        alto: c.alto,
        salida,
        escala_x: escala.0,
        escala_y: escala.1,
        escala_propia_x: propia.0,
        escala_propia_y: propia.1,
    })
}

/// Los bytes del PNG congelado.
///
/// **Por el IPC y no por una URL `asset://`.** Los dos caminos traen la misma
/// imagen, pero no con el mismo origen: `asset://` es otro, y una imagen de otro
/// origen **contamina** el canvas en el que se dibuja. Un canvas contaminado no
/// deja leer sus píxeles ni exportarlos, que es exactamente lo que necesitan las
/// herramientas que tapan y lo que necesita entregar una captura anotada.
///
/// El navegador lo dejaría pasar si el servidor contestara con las cabeceras de
/// CORS y la imagen se pidiera con `crossOrigin`, y Tauri **manda** esa cabecera
/// — pero WebKitGTK sólo la mira en los esquemas que se registran como
/// habilitados para CORS, y wry registra los suyos como seguros y nada más. Así
/// que por ahí no hay arreglo desde acá.
///
/// Con los bytes en la mano, el frontend arma un `blob:` que **hereda su propio
/// origen**, y el canvas queda limpio. Cuestan una copia de unos pocos megabytes
/// una sola vez, al abrir.
///
/// Van como cuerpo crudo: adentro de un JSON serían una lista de números y
/// costarían un orden de magnitud más. Es el mismo trato que reciben los bytes
/// que viajan en la otra dirección al guardar una captura anotada.
///
/// **La lectura no pasa por el hilo principal.** Un comando síncrono corre ahí,
/// y esto lee del disco una captura de todas las pantallas: unos cuantos
/// megabytes. Mientras dura, el hilo principal es también el que atiende la
/// ventana del selector, así que la demora se vería. Del candado sale una copia
/// de la ruta y nada más; lo que tarda pasa afuera, sin él.
#[tauri::command]
pub async fn imagen() -> Result<tauri::ipc::Response, String> {
    let ruta = {
        let guardia = pendiente()
            .lock()
            .map_err(|_| "el estado de la captura quedó envenenado".to_string())?;
        guardia
            .as_ref()
            .ok_or_else(|| "todavía no hay ninguna captura".to_string())?
            .ruta
            .clone()
    };
    let bytes = tauri::async_runtime::spawn_blocking(move || leer_el_png(&ruta))
        .await
        .map_err(|e| format!("la lectura de la captura no llegó a terminar: {e}"))??;
    Ok(tauri::ipc::Response::new(bytes))
}

/// Los bytes del archivo, con el error diciendo **cuál**.
///
/// Aparte del comando para poder probarlo: `imagen` toca el estado global de la
/// captura pendiente, y una prueba que lo pise se cruza con las demás, que corren
/// en el mismo proceso.
fn leer_el_png(ruta: &std::path::Path) -> Result<Vec<u8>, String> {
    std::fs::read(ruta)
        .map_err(|e| format!("no se pudo leer la captura en {}: {e}", ruta.display()))
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

/// De dónde salen los píxeles de lo elegido, y qué rectángulo hay que sacar.
///
/// Dos fuentes y no una porque el lienzo compone **todo a la escala mayor**: una
/// pantalla de menor escala entra estirada, y recortar de ahí devuelve una
/// imagen más grande que la que se eligió y borrosa, porque sus píxeles son una
/// interpolación y no los que el compositor entregó. Cuando lo elegido cabe
/// entero en una pantalla de ésas, se recorta de los suyos.
#[derive(Debug, PartialEq, Eq)]
enum Fuente {
    /// De los píxeles sin estirar de esa pantalla, en su propia escala.
    Nativa { indice: usize, region: Region },
    /// De la imagen compuesta, que es de donde salió siempre.
    Compuesta { region: Region },
}

/// Si el rectángulo de afuera contiene entero al de adentro.
fn contiene(afuera: Salida, adentro: Region) -> bool {
    adentro.x >= afuera.x
        && adentro.y >= afuera.y
        && adentro.x + adentro.ancho <= afuera.x + afuera.ancho
        && adentro.y + adentro.alto <= afuera.y + afuera.alto
}

/// Elige la fuente y traduce la región, dado todo lo que hace falta.
///
/// Separada de los candados y de los estáticos para poder probarla: es la cuenta
/// que decide qué píxeles se guardan, y equivocarla no falla — entrega una
/// imagen que parece la pedida.
fn fuente_de(elegida: Region, geo: Geometria, escala: (f64, f64), copias: &[Copia]) -> Fuente {
    let n = elegida.normalizada();
    // La selección llega en coordenadas del selector; las pantallas están en las
    // del layout. Se suman los orígenes para poder compararlas.
    let absoluta = Region {
        x: geo.salida.x + n.x,
        y: geo.salida.y + n.y,
        ..n
    };

    for (indice, copia) in copias.iter().enumerate() {
        let Some(propia) = copia.escala() else {
            // Esta pantalla no se estiró: sus píxeles de verdad ya están en el
            // lienzo, así que no hay ninguna ventaja en tratarla aparte.
            continue;
        };
        if !contiene(copia.salida.area(), absoluta) {
            continue;
        }

        // Relativa a **su** esquina, y en su escala: el rectángulo que se pide
        // es el de sus píxeles, no el del lienzo.
        let en_la_pantalla = Region {
            x: absoluta.x - copia.salida.x,
            y: absoluta.y - copia.salida.y,
            ..absoluta
        };
        return Fuente::Nativa {
            indice,
            region: en_la_pantalla.en_la_captura(
                Salida::entera(copia.salida.ancho, copia.salida.alto),
                propia,
            ),
        };
    }

    Fuente::Compuesta {
        region: elegida.en_la_captura(geo.salida.relativa_a(geo.layout), escala),
    }
}

/// Recorta lo elegido y lo deja en `destino`.
///
/// Todos los comandos pasan por acá: `guardar`, `copiar` y `guardar_y_copiar`. Que
/// sea uno solo es a propósito — cuando la traducción faltaba, faltaba en los tres
/// y había que arreglarla tres veces.
fn recortar_en(destino: &std::path::Path, elegida: Region) -> Result<(), String> {
    let guardia = pendiente()
        .lock()
        .map_err(|_| "el estado de la captura quedó envenenado".to_string())?;
    let c = guardia
        .as_ref()
        .ok_or_else(|| "todavía no hay ninguna captura".to_string())?;

    let anotada = geometria().lock().ok().and_then(|g| *g);
    let (_, escala) = salida_y_escala(c.ancho, c.alto);

    let fuente = match anotada {
        Some(geo) => fuente_de(elegida, geo, escala, &c.copias),
        // Sin geometría no se sabe en qué pantalla está el selector, así que
        // tampoco cuál de las copias le corresponde: queda el lienzo, que es lo
        // que había antes de que las copias existieran.
        None => Fuente::Compuesta {
            region: elegida.en_la_captura(Salida::entera(c.ancho as i32, c.alto as i32), escala),
        },
    };

    match fuente {
        Fuente::Nativa { indice, region } => {
            let nativos = c.copias[indice]
                .nativos
                .as_ref()
                .ok_or_else(|| "esa pantalla no guardó sus píxeles".to_string())?;
            captura::recortar_imagen(nativos, region, destino)
        }
        Fuente::Compuesta { region } => captura::recortar(&c.ruta, region, destino),
    }
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
    /// A dónde se suben, o nulo si no se sube a ningún lado.
    pub subir_a: Option<String>,
    /// El campo del formulario, si se eligió uno distinto del de siempre.
    pub subir_campo: Option<String>,
    /// El servidor de esa dirección, que es lo que hay que preguntar antes.
    ///
    /// Ya extraído de este lado y no en el frontend: el que va a ejecutar la
    /// subida es éste, y lo que se pregunta tiene que ser lo que de verdad va a
    /// pasar. Dos maneras de sacar el anfitrión de una dirección son dos que se
    /// separan, y ésta se separaría justo en la pregunta.
    pub subir_servidor: Option<String>,
}

/// Arma la respuesta del panel a partir de las preferencias dadas.
fn ajustes_de(preferencias: prefs::Preferencias) -> Result<Ajustes, String> {
    let subida = subida::destino_de(
        preferencias.subir_a.as_deref(),
        preferencias.subir_campo.as_deref(),
    )
    .ok()
    .flatten();

    Ok(Ajustes {
        al_soltar: preferencias.al_soltar,
        carpeta: preferencias
            .carpeta
            .as_ref()
            .map(|c| c.to_string_lossy().into_owned()),
        carpeta_efectiva: destino::carpeta_sin_crear()?.to_string_lossy().into_owned(),
        subir_a: preferencias.subir_a.clone(),
        subir_campo: preferencias.subir_campo.clone(),
        subir_servidor: subida.map(|d| d.servidor().to_string()),
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
pub fn guardar_ajustes(
    al_soltar: AlSoltar,
    carpeta: Option<String>,
    subir_a: Option<String>,
    subir_campo: Option<String>,
) -> Result<Ajustes, String> {
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

    // La dirección se comprueba **antes** de guardarla: una que no sea `https`
    // tiene que fallar con el panel abierto y a la vista, no en el momento de
    // apretar «subir», que es cuando ya hay una captura esperando.
    let destino = subida::destino_de(subir_a.as_deref(), subir_campo.as_deref())?;

    let preferencias = prefs::Preferencias {
        al_soltar,
        carpeta,
        subir_a: destino.as_ref().map(|d| d.url.clone()),
        subir_campo: destino.map(|d| d.campo),
    };
    prefs::escribir(&preferencias)?;
    ajustes_de(preferencias)
}

/// La dirección configurada, o un error que dice que no hay ninguna.
///
/// Se lee del archivo en el momento de subir y no de lo que el frontend crea
/// tener: lo que se publica tiene que salir de lo que está guardado.
fn destino_configurado() -> Result<Destino, String> {
    let preferencias = prefs::leer();
    subida::destino_de(
        preferencias.subir_a.as_deref(),
        preferencias.subir_campo.as_deref(),
    )?
    .ok_or_else(|| "no hay ninguna dirección configurada para subir".to_string())
}

/// Sube lo elegido y deja el enlace en el portapapeles.
///
/// El recorte va a un temporal y se borra: subir no es guardar, y quien sube no
/// pidió además un archivo en la carpeta.
#[tauri::command]
pub fn subir(region: Region) -> Result<String, String> {
    let destino = destino_configurado()?;
    let temporal = anotada::temporal("subir")?;
    let resultado =
        recortar_en(&temporal, region).and_then(|()| subida::subir(&temporal, &destino));
    let _ = std::fs::remove_file(&temporal);
    con_enlace(resultado?)
}

/// Lo mismo, con la captura ya compuesta por el selector.
#[tauri::command]
pub fn subir_anotada(pedido: tauri::ipc::Request<'_>) -> Result<String, String> {
    let bytes = bytes_de(&pedido)?;
    let destino = destino_configurado()?;
    let temporal = anotada::temporal("subir")?;
    let resultado =
        anotada::escribir(bytes, &temporal).and_then(|()| subida::subir(&temporal, &destino));
    let _ = std::fs::remove_file(&temporal);
    con_enlace(resultado?)
}

/// Deja el enlace en el portapapeles y lo devuelve.
///
/// Que el portapapeles falle no cancela la subida: la captura **ya está
/// publicada**, y perder el enlace sería lo peor que podría pasar a esa altura.
/// Se devuelve igual, que es lo que el selector muestra.
fn con_enlace(enlace: String) -> Result<String, String> {
    if let Err(e) = captura::copiar_texto_al_portapapeles(&enlace) {
        eprintln!("vasak-shot: no se pudo copiar el enlace: {e}");
    }
    Ok(enlace)
}

/// Recorta a la región elegida y devuelve el archivo final.
fn producir(region: Region) -> Result<PathBuf, String> {
    let final_ = destino::carpeta()?.join(destino::nombre_de_ahora());
    recortar_en(&final_, region)?;
    Ok(final_)
}

/// Guarda la región elegida en la carpeta de capturas.
#[tauri::command]
pub fn guardar(region: Region) -> Result<String, String> {
    let final_ = producir(region)?;
    avisar(&final_, false);
    Ok(final_.to_string_lossy().into_owned())
}

/// Copia la región elegida al portapapeles, sin dejar archivo en la carpeta.
///
/// El recorte va a un temporal: quien copia quiere pegar, no acumular archivos
/// que después hay que borrar a mano.
#[tauri::command]
pub fn copiar(region: Region) -> Result<(), String> {
    // Creado en exclusiva y en `0o600`: `/tmp` lo comparte toda la máquina, y
    // una captura que se pidió **sólo copiar** no puede quedar un rato legible
    // por otra cuenta. El recorte escribe sobre el archivo que ya existe y no le
    // cambia los permisos.
    let temporal = anotada::temporal("copia")?;
    recortar_en(&temporal, region)?;
    let resultado = captura::copiar_al_portapapeles(&temporal);
    let _ = std::fs::remove_file(&temporal);
    resultado
}

/// Los textos del aviso, tal como el frontend los tradujo.
///
/// Un estático y no un argumento de cada comando: son los mismos para toda la
/// vida del proceso, y pasarlos en cada entrega sería repetir cuatro cadenas en
/// cuatro comandos. El frontend los deja acá al abrirse; el camino de la línea
/// de órdenes no pasa por ninguna ventana y se queda con los de reserva.
fn textos_del_aviso() -> &'static Mutex<Textos> {
    static TEXTOS: OnceLock<Mutex<Textos>> = OnceLock::new();
    TEXTOS.get_or_init(|| Mutex::new(Textos::default()))
}

/// Anota los textos del aviso, ya traducidos.
#[tauri::command]
pub fn traducir_aviso(textos: Textos) {
    if let Ok(mut guardia) = textos_del_aviso().lock() {
        *guardia = textos;
    }
}

/// Avisa que la captura quedó guardada, con la miniatura y qué hacer con ella.
///
/// Lo muestra **otro proceso**: los botones del aviso implican esperar a que
/// alguien los apriete, y esta ventana se cierra sola en cuanto la captura está
/// entregada. Ver `aviso`.
///
/// Si falla, no se dice nada más: la captura ya está guardada y el aviso es un
/// lujo, no el resultado.
fn avisar(ruta: &std::path::Path, copiada: bool) {
    let textos = textos_del_aviso()
        .lock()
        .map(|t| t.clone())
        .unwrap_or_default();
    aviso::lanzar(ruta, copiada, &textos);
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
    avisar(&final_, false);
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
    // archivo igual en lugar de perder las dos cosas por una. Y el aviso se
    // entera, porque si la copia falló lo que hace falta es justamente el botón
    // de copiar.
    let copiada = match captura::copiar_al_portapapeles(&final_) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("vasak-shot: no se pudo copiar al portapapeles: {e}");
            false
        }
    };
    avisar(&final_, copiada);
    Ok(final_.to_string_lossy().into_owned())
}

/// Guarda **y** copia, que es lo que se quiere casi siempre.
#[tauri::command]
pub fn guardar_y_copiar(region: Region) -> Result<String, String> {
    let final_ = producir(region)?;
    // Si el portapapeles falla, la captura ya está guardada: se informa el
    // archivo igual en lugar de perder las dos cosas por una. Y el aviso se
    // entera, porque si la copia falló lo que hace falta es justamente el botón
    // de copiar.
    let copiada = match captura::copiar_al_portapapeles(&final_) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("vasak-shot: no se pudo copiar al portapapeles: {e}");
            false
        }
    };
    avisar(&final_, copiada);
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

    /// Un monitor al 100 % y otro al 200 %, apilados, como un portátil HiDPI
    /// con una pantalla externa. El lienzo compone a la mayor, así que la
    /// externa entra estirada al doble.
    fn escalas_distintas() -> Vec<Copia> {
        vec![
            Copia {
                salida: Monitor::nuevo(
                    "HDMI-A-1",
                    Salida {
                        x: 0,
                        y: 0,
                        ancho: 1920,
                        alto: 1080,
                    },
                ),
                // 1920x1080 de verdad: el lienzo la estiró a 3840x2160.
                nativos: Some(image::RgbaImage::new(1920, 1080)),
            },
            Copia {
                salida: Monitor::nuevo(
                    "eDP-1",
                    Salida {
                        x: 0,
                        y: 1080,
                        ancho: 1920,
                        alto: 1080,
                    },
                ),
                // La que manda la escala: sus píxeles ya son los del lienzo.
                nativos: None,
            },
        ]
    }

    /// El encuadre de esa sesión: layout de 1920x2160 desde el origen.
    fn geometria_en(salida: Salida) -> Geometria {
        Geometria {
            salida,
            layout: Salida {
                x: 0,
                y: 0,
                ancho: 1920,
                alto: 2160,
            },
        }
    }

    #[test]
    fn en_la_pantalla_estirada_se_recorta_de_sus_pixeles() {
        // Es el arreglo: elegir 400x300 en la pantalla al 100 % tiene que dar un
        // archivo de 400x300 con los píxeles que el compositor entregó, y no uno
        // de 800x600 interpolado del lienzo — que es lo que salía, con las
        // medidas que el selector mostró diciendo otra cosa.
        let copias = escalas_distintas();
        let geo = geometria_en(copias[0].salida.area());

        let fuente = fuente_de(
            Region {
                x: 100,
                y: 50,
                ancho: 400,
                alto: 300,
            },
            geo,
            (2.0, 2.0),
            &copias,
        );

        assert_eq!(
            fuente,
            Fuente::Nativa {
                indice: 0,
                region: Region {
                    x: 100,
                    y: 50,
                    ancho: 400,
                    alto: 300
                }
            }
        );
    }

    #[test]
    fn en_la_pantalla_que_manda_la_escala_sale_del_lienzo() {
        // Ahí los píxeles del lienzo **son** los suyos, así que no hay nada que
        // guardar aparte ni ningún camino distinto que tomar.
        let copias = escalas_distintas();
        let geo = geometria_en(copias[1].salida.area());

        let fuente = fuente_de(
            Region {
                x: 100,
                y: 50,
                ancho: 400,
                alto: 300,
            },
            geo,
            (2.0, 2.0),
            &copias,
        );

        assert_eq!(
            fuente,
            Fuente::Compuesta {
                // La salida empieza en y=1080 del layout, o sea en 2160 de la
                // imagen; la selección cae 100 más abajo, ya en píxeles.
                region: Region {
                    x: 200,
                    y: 2260,
                    ancho: 800,
                    alto: 600
                }
            }
        );
    }

    #[test]
    fn lo_que_no_cabe_en_una_sola_pantalla_sale_del_lienzo() {
        // `--pantalla` pide la composición entera, y una región que se pasa del
        // borde no está entera en ninguna. El lienzo es el único lugar donde
        // están todas.
        let copias = escalas_distintas();
        let geo = geometria_en(copias[0].salida.area());

        let fuente = fuente_de(
            Region {
                x: 0,
                y: 0,
                ancho: 1920,
                alto: 2160,
            },
            geo,
            (2.0, 2.0),
            &copias,
        );

        assert!(matches!(fuente, Fuente::Compuesta { .. }), "{fuente:?}");
    }

    #[test]
    fn con_una_sola_pantalla_todo_sigue_saliendo_del_lienzo() {
        // El caso de todos los días: nada de esto se activa, y el recorte es el
        // mismo que antes de que las copias existieran.
        let copias = vec![Copia {
            salida: Monitor::nuevo(
                "eDP-1",
                Salida {
                    x: 0,
                    y: 0,
                    ancho: 1920,
                    alto: 1080,
                },
            ),
            nativos: None,
        }];
        let geo = Geometria {
            salida: copias[0].salida.area(),
            layout: copias[0].salida.area(),
        };

        let fuente = fuente_de(
            Region {
                x: 10,
                y: 20,
                ancho: 30,
                alto: 40,
            },
            geo,
            (1.0, 1.0),
            &copias,
        );

        assert_eq!(
            fuente,
            Fuente::Compuesta {
                region: Region {
                    x: 10,
                    y: 20,
                    ancho: 30,
                    alto: 40
                }
            }
        );
    }

    #[test]
    fn una_seleccion_al_reves_se_endereza_antes_de_elegir_la_fuente() {
        // Arrastrar hacia arriba y a la izquierda da medidas negativas. Sin
        // normalizar primero, la comprobación de «cabe en esta pantalla» falla
        // y el recorte se iría al lienzo — o sea al camino borroso, y justo en
        // la pantalla donde se nota.
        let copias = escalas_distintas();
        let geo = geometria_en(copias[0].salida.area());

        let fuente = fuente_de(
            Region {
                x: 500,
                y: 350,
                ancho: -400,
                alto: -300,
            },
            geo,
            (2.0, 2.0),
            &copias,
        );

        assert_eq!(
            fuente,
            Fuente::Nativa {
                indice: 0,
                region: Region {
                    x: 100,
                    y: 50,
                    ancho: 400,
                    alto: 300
                }
            }
        );
    }

    /// El PNG que el frontend va a convertir en `blob:`.
    ///
    /// Que los bytes lleguen **enteros e iguales** es todo lo que este camino
    /// tiene que garantizar: lo que el canvas dibuje después sale de acá, y una
    /// copia a medias se vería como una imagen cortada y no como un error.
    #[test]
    fn los_bytes_del_png_llegan_tal_cual() {
        let dir = std::env::temp_dir().join(format!("vasak-shot-prueba-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("no se pudo crear el directorio de la prueba");
        let ruta = dir.join("captura.png");
        // Un encabezado de PNG de verdad: bytes que no son texto, que es lo que
        // distingue un camino crudo de uno que pasa por JSON.
        let contenido: Vec<u8> = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0xff];
        std::fs::write(&ruta, &contenido).expect("no se pudo escribir la captura de la prueba");

        assert_eq!(leer_el_png(&ruta), Ok(contenido));

        std::fs::remove_dir_all(&dir).ok();
    }

    /// Sin archivo, el error dice **qué** ruta faltó.
    ///
    /// Un «no such file or directory» pelado manda a buscar en todo el proceso
    /// cuál de los archivos es; con la ruta adentro, el aviso que ve la persona
    /// ya nombra el que hay que mirar.
    #[test]
    fn si_el_png_no_esta_el_error_nombra_la_ruta() {
        let ruta = std::env::temp_dir().join("vasak-shot-que-no-existe-jamas.png");
        let e = leer_el_png(&ruta).expect_err("leer un archivo que no existe tiene que fallar");
        assert!(e.contains("vasak-shot-que-no-existe-jamas.png"), "{e}");
    }
}
