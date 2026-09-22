//! Los píxeles de las pantallas, pedidos al compositor por nuestra cuenta.
//!
//! # Por qué dejamos de llamar a `grim`
//!
//! No porque `grim` esté mal: hace esto bien y es de donde salió la forma de
//! hacerlo. El motivo es de permisos. El escritorio limita qué programas pueden
//! pedirle al compositor los protocolos que ven la sesión —`permisos-globales`,
//! de `vasak-wayfire-plugins`— y esa lista es **por ejecutable**. Con `grim`
//! adentro, cualquier programa capturaba la pantalla entera con dos líneas:
//!
//! ```sh
//! #!/usr/bin/env bash
//! grim "$1"
//! ```
//!
//! Medido: capturó 290 950 bytes sin estar en ninguna lista. El permiso de
//! compartir pantalla se saltaba llamando a la herramienta que sí lo tenía. Que
//! los píxeles los tome esta aplicación es lo que deja sacar a `grim` de la
//! lista sin que el escritorio pierda las capturas.
//!
//! # Qué hay que hacer para capturar
//!
//! `zwlr_screencopy_manager_v1` copia **una salida** por vez a un buffer de
//! memoria compartida. Lo que no hace es componer: con dos pantallas hay que
//! pedir dos copias y pegarlas donde el layout las tiene.
//!
//! La posición y el tamaño lógicos salen de `xdg_output`, no de `wl_output`:
//! los de `wl_output` están en píxeles del dispositivo y no contemplan la
//! escala, así que con una pantalla al 125% el layout que se arma con ellos no
//! es el que usa el compositor.
//!
//! # El lienzo, y por qué su origen no es (0, 0)
//!
//! La imagen abarca el rectángulo que ocupan las salidas, que puede no empezar
//! en el origen: en esta máquina la única pantalla está en `0,1080` y la
//! captura mide 1920x1080, no 1920x2160. Es lo mismo que hace `grim`, y es lo
//! que `Region::en_la_captura` da por sentado cuando traduce una selección.
//!
//! Con escalas distintas entre pantallas, el lienzo va en la **mayor** de
//! todas: bajar todo a la menor tiraría píxeles que existen.

use crate::captura::{Monitor, Salida};
use std::collections::HashMap;
use std::fs::File;
use std::os::fd::{AsFd, OwnedFd};

use image::RgbaImage;
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{
    wl_buffer::WlBuffer,
    wl_output::WlOutput,
    wl_registry::WlRegistry,
    wl_shm::{Format, WlShm},
    wl_shm_pool::WlShmPool,
};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, WEnum};
use wayland_protocols::xdg::xdg_output::zv1::client::{
    zxdg_output_manager_v1::ZxdgOutputManagerV1,
    zxdg_output_v1::{self, ZxdgOutputV1},
};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1::{self, ZwlrScreencopyFrameV1},
    zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
};

/// Una pantalla, con lo que hace falta para ubicarla y copiarla.
#[derive(Default)]
struct Pantalla {
    /// Cómo la llama el compositor: `HDMI-A-1`, `eDP-1`. De `xdg_output`.
    ///
    /// Opcional porque el evento `name` existe desde la versión 2 del
    /// protocolo. Con una más vieja no hay nombre y `componer` le pone uno por
    /// posición, que es peor para escribirlo a mano pero deja que `--salida`
    /// siga sirviendo para algo.
    nombre: Option<String>,
    /// Dónde empieza en el layout. De `xdg_output`.
    ///
    /// Va separada del tamaño porque llegan en **dos eventos distintos**, y
    /// dar la geometría por completa con el primero fue un error de verdad: con
    /// la posición puesta y el tamaño todavía sin llegar, esto se daba por
    /// terminado y el lienzo salía de cero píxeles. Lo marcó la revisión.
    posicion: Option<(i32, i32)>,
    /// Cuánto ocupa en el layout. De `xdg_output`.
    tamanio: Option<(i32, i32)>,
    /// El compositor avisó que el contenido viene dado vuelta de arriba abajo.
    invertida: bool,
    /// Por qué no se pudo copiar ésta, si no se pudo.
    motivo: Option<String>,
    /// Lo que el compositor dijo del buffer: formato, ancho, alto y paso.
    buffer: Option<(Format, u32, u32, u32)>,
    /// La memoria donde el compositor escribe, mientras dura la copia.
    ///
    /// Se guarda acá y no en una variable local porque entre pedir la copia y
    /// que el compositor avise `ready` pasan varias vueltas del despacho de
    /// eventos: si el mapa se suelta antes, lo que se lee es memoria liberada.
    mapa: Option<memmap2::MmapMut>,
    /// El pool y el buffer de Wayland, vivos por la misma razón que el mapa.
    recursos: Option<(WlShmPool, WlBuffer)>,
    /// Los píxeles ya copiados, en RGBA.
    pixeles: Option<RgbaImage>,
    /// El compositor avisó que esta copia no se va a poder hacer.
    fallada: bool,
}

/// Todo lo que el despacho de eventos va llenando.
struct Estado {
    shm: WlShm,
    pantallas: HashMap<u32, Pantalla>,
    /// De cada objeto a la pantalla que le corresponde, para saber a quién anotar.
    de_xdg: HashMap<u32, u32>,
    de_frame: HashMap<u32, u32>,
}

/// El lienzo compuesto: lo que se guarda como PNG, y qué hay adentro.
pub struct Lienzo {
    pub imagen: RgbaImage,
    /// Las salidas que entraron, con sus píxeles de verdad si hizo falta.
    ///
    /// Sólo las que se pudieron copiar: una que el compositor no quiso copiar
    /// no está en la imagen, y ofrecerla para `--salida` sería ofrecer un
    /// recorte de píxeles negros.
    pub copias: Vec<Copia>,
}

/// Una salida copiada, y sus píxeles sin estirar cuando no son los del lienzo.
///
/// El lienzo se compone en la escala **mayor** de todas las pantallas, así que
/// una de menor escala entra estirada. Recortar de ahí devuelve una imagen del
/// doble de tamaño y borrosa: los píxeles que se guardan no son los que el
/// compositor entregó sino una interpolación de ellos, y el archivo no coincide
/// con las medidas que el selector mostró.
///
/// Por eso se guardan aparte los de las que **se estiraron**, y sólo ésos: en
/// una pantalla sola, o en varias de la misma escala, el lienzo ya tiene los
/// píxeles de verdad y quedarse con una segunda copia sería duplicar decenas de
/// megabytes para nada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Copia {
    pub salida: Monitor,
    /// Los píxeles tal como llegaron, sólo si la composición los estiró.
    pub nativos: Option<RgbaImage>,
}

impl Copia {
    /// Cuántos píxeles de verdad tiene esta pantalla por unidad del layout.
    ///
    /// La suya, no la del lienzo. En una pantalla que no se estiró son la
    /// misma; en una de menor escala, no — y ésa es toda la diferencia entre
    /// guardar los píxeles que el compositor entregó o una interpolación de
    /// ellos al doble de tamaño.
    ///
    /// `None` cuando no hay píxeles guardados, que quiere decir «los del lienzo
    /// son los suyos». Un cero de relleno sería una escala válida a la vista y
    /// una división por cero un rato después.
    pub fn escala(&self) -> Option<(f64, f64)> {
        let px = self.nativos.as_ref()?;
        Some((
            f64::from(px.width()) / f64::from(self.salida.ancho.max(1)),
            f64::from(px.height()) / f64::from(self.salida.alto.max(1)),
        ))
    }
}

/// Pide una copia de cada pantalla y las pega donde el layout las tiene.
///
/// Devuelve el error del compositor tal cual cuando lo hay: si el escritorio no
/// nos ofrece el protocolo —porque esta aplicación no está en la lista de
/// permitidos— lo que falta es el global, y eso se dice con todas las letras en
/// vez de «falló sin decir por qué».
pub fn capturar_pantallas() -> Result<Lienzo, String> {
    let conexion = Connection::connect_to_env()
        .map_err(|e| format!("no se pudo hablar con el compositor: {e}"))?;
    let (globales, mut cola) = registry_queue_init::<Estado>(&conexion)
        .map_err(|e| format!("no se pudo leer el registro de Wayland: {e}"))?;
    let qh = cola.handle();

    let shm: WlShm = globales
        .bind(&qh, 1..=1, ())
        .map_err(|_| "el compositor no ofrece memoria compartida (wl_shm)".to_string())?;
    let capturador: ZwlrScreencopyManagerV1 = globales.bind(&qh, 1..=3, ()).map_err(|_| {
        "el compositor no ofrece el protocolo de captura (zwlr_screencopy_manager_v1): \
         esta aplicación no está entre las que pueden capturar la pantalla"
            .to_string()
    })?;
    let gestor_xdg: ZxdgOutputManagerV1 = globales
        .bind(&qh, 1..=3, ())
        .map_err(|_| "el compositor no ofrece xdg_output_manager".to_string())?;

    let mut estado = Estado {
        shm,
        pantallas: HashMap::new(),
        de_xdg: HashMap::new(),
        de_frame: HashMap::new(),
    };

    // Las salidas del registro, cada una con su xdg_output y su copia pedida.
    let salidas: Vec<WlOutput> = globales
        .contents()
        .clone_list()
        .into_iter()
        .filter(|global| global.interface == "wl_output")
        .map(|global| {
            globales
                .registry()
                .bind::<WlOutput, _, _>(global.name, global.version.min(4), &qh, ())
        })
        .collect();

    if salidas.is_empty() {
        return Err("el compositor no informó ninguna pantalla".to_string());
    }

    for salida in &salidas {
        let id = salida.id().protocol_id();
        estado.pantallas.insert(id, Pantalla::default());

        let xdg = gestor_xdg.get_xdg_output(salida, &qh, ());
        estado.de_xdg.insert(xdg.id().protocol_id(), id);

        // `overlay_cursor = 0`: el puntero no va en la captura. Es lo que hace
        // `grim` sin `-c`, y lo que se espera de una captura de pantalla.
        let cuadro = capturador.capture_output(0, salida, &qh, ());
        estado.de_frame.insert(cuadro.id().protocol_id(), id);
    }

    // Se bombea hasta que cada pantalla haya contestado: con sus píxeles o con
    // un fallo. Sin tope de vueltas: quien corta es el compositor, que contesta
    // `ready` o `failed` por cada copia.
    //
    // Una que falle **no** corta a las demás. Dos pantallas y una que el
    // compositor no quiere copiar dan una captura de la otra, que es mejor que
    // ninguna; sólo si no queda ninguna usable esto devuelve error, y entonces
    // el mensaje lleva los motivos de las que fallaron.
    while !estado.terminado() {
        cola.blocking_dispatch(&mut estado)
            .map_err(|e| format!("se cortó la conversación con el compositor: {e}"))?;
    }

    componer(estado.pantallas)
}

/// Todas las pantallas contestaron: geometría y píxeles, o fallo.
///
/// La geometría pide **las dos** mitades. Con sólo una, lo que falta puede
/// estar por llegar en el evento siguiente.
///
/// Va suelta y no como método de `Estado` para poder probarla sin una conexión
/// de Wayland: `Estado` lleva adentro un `wl_shm`, que no existe fuera de una
/// sesión. Mismo criterio que `decidir` en el plugin de permisos.
fn todas_contestaron(pantallas: &HashMap<u32, Pantalla>) -> bool {
    pantallas
        .values()
        .all(|p| p.fallada || (p.posicion.is_some() && p.tamanio.is_some() && p.pixeles.is_some()))
}

/// Marca una pantalla como perdida, con su motivo. No toca a las demás.
fn dar_por_perdida(pantallas: &mut HashMap<u32, Pantalla>, id: u32, motivo: String) {
    if let Some(pantalla) = pantallas.get_mut(&id) {
        pantalla.fallada = true;
        pantalla.motivo.get_or_insert(motivo);
        // Lo copiado a medias no sirve y la memoria es grande.
        pantalla.mapa = None;
        pantalla.recursos = None;
    }
}

impl Estado {
    fn terminado(&self) -> bool {
        todas_contestaron(&self.pantallas)
    }

    fn anotar(&mut self, id: u32, motivo: String) {
        dar_por_perdida(&mut self.pantallas, id, motivo);
    }
}

/// Lo que hace falta de una pantalla para pegarla: cómo se llama, dónde va y
/// qué píxeles tiene.
type Puesta = (Option<String>, (i32, i32, i32, i32), RgbaImage);

/// Pega cada pantalla en su lugar del layout.
///
/// Toma el mapa **por valor** para poder quedarse con los píxeles en lugar de
/// copiarlos: son decenas de megabytes y el que llama no los usa después.
fn componer(pantallas: HashMap<u32, Pantalla>) -> Result<Lienzo, String> {
    // Los motivos de las que fallaron, antes de consumir el mapa. Sin esto el
    // error sería «ninguna pantalla se pudo copiar» a secas, que no deja
    // arreglar nada.
    let motivos: Vec<String> = pantallas
        .values()
        .filter_map(|p| p.motivo.clone())
        .collect();

    let mut utiles: Vec<Puesta> = pantallas
        .into_values()
        .filter_map(|p| match (p.posicion, p.tamanio, p.pixeles) {
            (Some((x, y)), Some((ancho, alto)), Some(pixeles)) if !p.fallada => {
                Some((p.nombre, (x, y, ancho, alto), pixeles))
            }
            _ => None,
        })
        .collect();

    // De arriba abajo y de izquierda a derecha. El orden no cambia la imagen
    // —cada pantalla se pega en su lugar— pero sí la lista que sale de acá, y
    // esa se muestra en el selector y se nombra en `--salida`. Sin ordenar
    // saldría en el orden de un `HashMap`, o sea distinto en cada ejecución.
    utiles.sort_by_key(|(_, l, _)| (l.1, l.0));

    if utiles.is_empty() {
        return Err(if motivos.is_empty() {
            "ninguna pantalla se pudo copiar".to_string()
        } else {
            format!("ninguna pantalla se pudo copiar: {}", motivos.join("; "))
        });
    }

    let x0 = utiles.iter().map(|(_, l, _)| l.0).min().unwrap_or(0);
    let y0 = utiles.iter().map(|(_, l, _)| l.1).min().unwrap_or(0);
    let x1 = utiles.iter().map(|(_, l, _)| l.0 + l.2).max().unwrap_or(0);
    let y1 = utiles.iter().map(|(_, l, _)| l.1 + l.3).max().unwrap_or(0);

    // La escala de cada pantalla sale de comparar sus píxeles con su tamaño
    // lógico, que es la única forma de enterarse de una escala fraccionaria: el
    // `scale` de `wl_output` es entero y miente cuando el compositor usa 1.25.
    let escala = utiles
        .iter()
        .map(|(_, l, img)| f64::from(img.width()) / f64::from(l.2.max(1)))
        .fold(1.0_f64, f64::max);

    // **Los bordes, no los tamaños.** Cada borde del layout se lleva a píxeles
    // por su cuenta y el tamaño sale de restar los dos ya redondeados. Con
    // escala fraccionaria, redondear el ancho aparte deja a dos pantallas
    // vecinas con un píxel de más o de menos entre ellas: una costura
    // transparente o una superposición, según para qué lado cayó el redondeo.
    // Es la misma regla que `Region::en_la_captura`, y por la misma razón.
    let borde = |valor: i32, origen: i32| (f64::from(valor - origen) * escala).round() as i64;

    let ancho = (borde(x1, x0).max(1)) as u32;
    let alto = (borde(y1, y0).max(1)) as u32;
    let mut lienzo = RgbaImage::new(ancho, alto);

    let mut copias = Vec::with_capacity(utiles.len());
    for (i, (nombre, logica, pixeles)) in utiles.into_iter().enumerate() {
        let destino_x = borde(logica.0, x0).max(0);
        let destino_y = borde(logica.1, y0).max(0);
        let ancho_destino = (borde(logica.0 + logica.2, x0) - destino_x).max(1) as u32;
        let alto_destino = (borde(logica.1 + logica.3, y0) - destino_y).max(1) as u32;

        // Los nombres que el compositor no dio se completan por posición:
        // `pantalla-1` es la de más arriba a la izquierda. No sirve para
        // reconocer el monitor, pero sí para poder nombrarlo en `--salida`.
        let salida = Monitor::nuevo(
            nombre.unwrap_or_else(|| format!("pantalla-{}", i + 1)),
            Salida {
                x: logica.0,
                y: logica.1,
                ancho: logica.2,
                alto: logica.3,
            },
        );

        if pixeles.width() == ancho_destino && pixeles.height() == alto_destino {
            image::imageops::overlay(&mut lienzo, &pixeles, destino_x, destino_y);
            // El lienzo ya tiene sus píxeles de verdad: guardarlos otra vez
            // serían decenas de megabytes duplicados. Es el caso normal.
            copias.push(Copia {
                salida,
                nativos: None,
            });
        } else {
            // Pantallas con escalas distintas: la de menor escala se estira para
            // ocupar lo que le toca del lienzo. `Triangle` y no `Nearest` porque
            // el resultado se mira, no se mide.
            let estirada = image::imageops::resize(
                &pixeles,
                ancho_destino,
                alto_destino,
                image::imageops::FilterType::Triangle,
            );
            image::imageops::overlay(&mut lienzo, &estirada, destino_x, destino_y);
            // Y los de verdad se guardan: recortar de lo estirado devuelve una
            // imagen más grande y borrosa que la que se eligió.
            copias.push(Copia {
                salida,
                nativos: Some(pixeles),
            });
        }
    }

    Ok(Lienzo {
        imagen: lienzo,
        copias,
    })
}

/// Los formatos que esta conversión sabe leer.
///
/// Los cuatro son de 32 bits por píxel, que es lo que `a_rgba` da por sentado
/// al avanzar de a cuatro bytes. Con uno de 24 —`Bgr888`— o de 16 leería
/// cualquier cosa y devolvería una imagen corrida en vez de un error, así que
/// el formato se comprueba **antes** de armar el buffer: si el compositor
/// ofrece otro, esa pantalla se da por perdida con su motivo.
fn formato_conocido(formato: Format) -> bool {
    matches!(
        formato,
        Format::Xrgb8888 | Format::Argb8888 | Format::Xbgr8888 | Format::Abgr8888
    )
}

/// Pasa el buffer del compositor a RGBA.
///
/// Los formatos que ofrece son little-endian y con los componentes al revés de
/// como los quiere `image`: `Xrgb8888` en memoria es B, G, R, X. Copiarlos sin
/// dar vuelta deja la captura con los azules y los rojos cambiados, que es un
/// error que se ve pero que nadie sabe nombrar.
///
/// El paso puede ser mayor que el ancho por cuatro —el compositor alinea las
/// filas— así que se recorre fila por fila y no de corrido.
fn a_rgba(datos: &[u8], formato: Format, ancho: u32, alto: u32, paso: u32) -> Option<RgbaImage> {
    let opaco = matches!(formato, Format::Xrgb8888 | Format::Xbgr8888);
    let invertido = matches!(formato, Format::Xrgb8888 | Format::Argb8888);

    let mut imagen = RgbaImage::new(ancho, alto);
    for y in 0..alto {
        let inicio = (y as usize) * (paso as usize);
        let fila = datos.get(inicio..inicio + (ancho as usize) * 4)?;
        for x in 0..ancho {
            let p = &fila[(x as usize) * 4..(x as usize) * 4 + 4];
            let (r, g, b) = if invertido {
                (p[2], p[1], p[0])
            } else {
                (p[0], p[1], p[2])
            };
            let a = if opaco { 255 } else { p[3] };
            imagen.put_pixel(x, y, image::Rgba([r, g, b, a]));
        }
    }
    Some(imagen)
}

impl Dispatch<WlRegistry, GlobalListContents> for Estado {
    fn event(
        _estado: &mut Self,
        _registro: &WlRegistry,
        _evento: <WlRegistry as wayland_client::Proxy>::Event,
        _datos: &GlobalListContents,
        _conexion: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // Las pantallas se enumeran una vez, al arrancar: una que aparezca
        // mientras se captura no entra en esta captura.
    }
}

macro_rules! sin_eventos {
    ($($tipo:ty),*) => {$(
        impl Dispatch<$tipo, ()> for Estado {
            fn event(
                _estado: &mut Self,
                _objeto: &$tipo,
                _evento: <$tipo as wayland_client::Proxy>::Event,
                _datos: &(),
                _conexion: &Connection,
                _qh: &QueueHandle<Self>,
            ) {
            }
        }
    )*};
}

sin_eventos!(
    WlShm,
    WlShmPool,
    WlBuffer,
    WlOutput,
    ZwlrScreencopyManagerV1,
    ZxdgOutputManagerV1
);

impl Dispatch<ZxdgOutputV1, ()> for Estado {
    fn event(
        estado: &mut Self,
        objeto: &ZxdgOutputV1,
        evento: zxdg_output_v1::Event,
        _datos: &(),
        _conexion: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let Some(&id) = estado.de_xdg.get(&objeto.id().protocol_id()) else {
            return;
        };
        let Some(pantalla) = estado.pantallas.get_mut(&id) else {
            return;
        };
        // Cada mitad por su lado: llegan en eventos distintos y no hay orden
        // garantizado entre ellos. El `done` de `xdg_output` no se usa —está
        // deprecado desde su versión 3, que manda usar el de `wl_output`— y no
        // hace falta: lo que dice que la geometría está completa es tener las
        // dos mitades, que es lo que mira `terminado`.
        match evento {
            zxdg_output_v1::Event::LogicalPosition { x, y } => {
                pantalla.posicion = Some((x, y));
            }
            zxdg_output_v1::Event::LogicalSize { width, height } => {
                pantalla.tamanio = Some((width, height));
            }
            zxdg_output_v1::Event::Name { name } => {
                pantalla.nombre = Some(name);
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwlrScreencopyFrameV1, ()> for Estado {
    fn event(
        estado: &mut Self,
        objeto: &ZwlrScreencopyFrameV1,
        evento: zwlr_screencopy_frame_v1::Event,
        _datos: &(),
        _conexion: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let Some(&id) = estado.de_frame.get(&objeto.id().protocol_id()) else {
            return;
        };

        match evento {
            // El compositor dice de qué tamaño y formato tiene que ser el
            // buffer. Se guarda y se espera a `buffer_done`: mandar la copia
            // antes deja al compositor esperando un buffer que todavía no
            // existe.
            zwlr_screencopy_frame_v1::Event::Buffer {
                format,
                width,
                height,
                stride,
            } => {
                let WEnum::Value(formato) = format else {
                    estado.anotar(id, "el compositor ofreció un formato desconocido".into());
                    return;
                };
                if !formato_conocido(formato) {
                    // Se sigue escuchando por si ofrece otro: el protocolo manda
                    // un `buffer` por cada formato que acepta, y alcanza con que
                    // uno sirva.
                    return;
                }
                if let Some(pantalla) = estado.pantallas.get_mut(&id) {
                    if pantalla.buffer.is_none() {
                        pantalla.buffer = Some((formato, width, height, stride));
                    }
                }

                // `buffer_done` existe recién en la versión 3. Con un compositor
                // más viejo no llega nunca, y esperarlo dejaba el bucle colgado
                // para siempre: ahí la copia se pide acá mismo. Lo marcó la
                // revisión.
                if objeto.version() < 3 {
                    armar_y_copiar(estado, id, objeto, qh);
                }
            }

            zwlr_screencopy_frame_v1::Event::BufferDone => {
                armar_y_copiar(estado, id, objeto, qh);
            }

            zwlr_screencopy_frame_v1::Event::Ready { .. } => {
                let Some((formato, ancho, alto, paso)) =
                    estado.pantallas.get(&id).and_then(|p| p.buffer)
                else {
                    return;
                };
                let convertida = estado
                    .pantallas
                    .get(&id)
                    .and_then(|p| p.mapa.as_ref())
                    .and_then(|mapa| a_rgba(mapa, formato, ancho, alto, paso));

                let invertida = estado.pantallas.get(&id).is_some_and(|p| p.invertida);

                match convertida.map(|imagen| {
                    if invertida {
                        image::imageops::flip_vertical(&imagen)
                    } else {
                        imagen
                    }
                }) {
                    Some(imagen) => {
                        if let Some(pantalla) = estado.pantallas.get_mut(&id) {
                            pantalla.pixeles = Some(imagen);
                            // Ya está copiado: se sueltan el mapa y el buffer.
                            pantalla.mapa = None;
                            pantalla.recursos = None;
                        }
                    }
                    None => estado.anotar(id, "el buffer vino más corto de lo que dice".into()),
                }
            }

            // El compositor puede entregar el contenido dado vuelta de arriba
            // abajo. Sin mirar esto, la captura sale espejada en vertical y no
            // hay nada que lo delate hasta que alguien la mira.
            zwlr_screencopy_frame_v1::Event::Flags { flags } => {
                let invertida = match flags {
                    WEnum::Value(valor) => valor.contains(zwlr_screencopy_frame_v1::Flags::YInvert),
                    WEnum::Unknown(_) => false,
                };
                if let Some(pantalla) = estado.pantallas.get_mut(&id) {
                    pantalla.invertida = invertida;
                }
            }

            zwlr_screencopy_frame_v1::Event::Failed => {
                estado.anotar(id, "el compositor rechazó copiar la pantalla".into());
            }

            _ => {}
        }
    }
}

/// Un archivo en memoria del tamaño pedido.
fn memoria_compartida(tamanio: usize) -> Result<File, String> {
    use rustix::fs::{memfd_create, MemfdFlags};
    let fd: OwnedFd = memfd_create("vasak-shot", MemfdFlags::CLOEXEC)
        .map_err(|e| format!("no se pudo reservar memoria compartida: {e}"))?;
    let archivo = File::from(fd);
    archivo
        .set_len(tamanio as u64)
        .map_err(|e| format!("no se pudo dimensionar la memoria compartida: {e}"))?;
    Ok(archivo)
}

/// Arma el buffer que el compositor pidió y le manda la copia.
///
/// Sale de `buffer_done` y también del propio `buffer` en las versiones 1 y 2
/// del protocolo, donde aquel evento no existe.
fn armar_y_copiar(
    estado: &mut Estado,
    id: u32,
    cuadro: &ZwlrScreencopyFrameV1,
    qh: &QueueHandle<Estado>,
) {
    // Si ya se pidió la copia de esta pantalla, no se pide otra: en las
    // versiones viejas el compositor manda un `buffer` por cada formato que
    // acepta, y el primero que sirva alcanza.
    if estado
        .pantallas
        .get(&id)
        .is_some_and(|p| p.recursos.is_some())
    {
        return;
    }

    let Some((formato, ancho, alto, paso)) = estado.pantallas.get(&id).and_then(|p| p.buffer)
    else {
        return estado.anotar(
            id,
            "el compositor no ofreció ningún formato que sepamos leer".into(),
        );
    };

    let tamanio = (paso as usize) * (alto as usize);
    let archivo = match memoria_compartida(tamanio) {
        Ok(archivo) => archivo,
        Err(e) => return estado.anotar(id, e),
    };
    let mapa = match unsafe { memmap2::MmapMut::map_mut(&archivo) } {
        Ok(mapa) => mapa,
        Err(e) => return estado.anotar(id, format!("no se pudo mapear la memoria: {e}")),
    };
    let pool = estado
        .shm
        .create_pool(archivo.as_fd(), tamanio as i32, qh, ());
    let buffer = pool.create_buffer(0, ancho as i32, alto as i32, paso as i32, formato, qh, ());
    cuadro.copy(&buffer);

    if let Some(pantalla) = estado.pantallas.get_mut(&id) {
        pantalla.mapa = Some(mapa);
        pantalla.recursos = Some((pool, buffer));
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Una pantalla ya copiada, para armar composiciones a mano.
    fn pantalla(logica: (i32, i32, i32, i32), imagen: RgbaImage) -> Pantalla {
        Pantalla {
            posicion: Some((logica.0, logica.1)),
            tamanio: Some((logica.2, logica.3)),
            pixeles: Some(imagen),
            ..Pantalla::default()
        }
    }

    /// Lo mismo, con el nombre que el compositor le habría dado.
    fn pantalla_llamada(nombre: &str, logica: (i32, i32, i32, i32), imagen: RgbaImage) -> Pantalla {
        Pantalla {
            nombre: Some(nombre.to_string()),
            ..pantalla(logica, imagen)
        }
    }

    /// Una imagen de un color, del tamaño pedido.
    fn lisa(ancho: u32, alto: u32, color: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(ancho, alto, image::Rgba(color))
    }

    #[test]
    fn el_buffer_llega_con_los_colores_al_reves() {
        // `Xrgb8888` en memoria es B, G, R, X. Copiarlo de corrido deja la
        // captura con los azules donde van los rojos: se ve, pero cuesta
        // nombrarlo, y es el error que más fácil se cuela.
        let azul_en_memoria = [0xFF, 0x00, 0x00, 0x00];
        let imagen = a_rgba(&azul_en_memoria, Format::Xrgb8888, 1, 1, 4).unwrap();

        assert_eq!(
            imagen.get_pixel(0, 0),
            &image::Rgba([0x00, 0x00, 0xFF, 0xFF])
        );
    }

    #[test]
    fn el_formato_sin_alfa_sale_opaco() {
        // La X de `Xrgb8888` no es transparencia: es relleno. Copiarla como
        // alfa deja la captura entera invisible si el compositor la dejó en
        // cero, que es lo que hace wlroots.
        let con_relleno_en_cero = [0x10, 0x20, 0x30, 0x00];
        let imagen = a_rgba(&con_relleno_en_cero, Format::Xrgb8888, 1, 1, 4).unwrap();

        assert_eq!(imagen.get_pixel(0, 0)[3], 0xFF);
    }

    #[test]
    fn el_alfa_de_verdad_se_respeta() {
        let medio_transparente = [0x10, 0x20, 0x30, 0x80];
        let imagen = a_rgba(&medio_transparente, Format::Argb8888, 1, 1, 4).unwrap();

        assert_eq!(imagen.get_pixel(0, 0)[3], 0x80);
    }

    #[test]
    fn las_filas_se_leen_por_el_paso_y_no_por_el_ancho() {
        // El compositor alinea las filas, así que el paso puede ser mayor que
        // el ancho por cuatro. Leyendo de corrido, cada fila arranca corrida
        // respecto de la anterior y la imagen sale inclinada.
        //
        // El relleno va de un color **distinto** a propósito. Con relleno en
        // ceros esta prueba pasaba igual leyendo mal: el corrimiento caía justo
        // sobre el primer píxel de la fila siguiente y daba el mismo color que
        // se esperaba. Se comprobó saboteando la función, que es como se
        // descubrió.
        let ancho = 2usize;
        let paso = 12usize; // 8 bytes de píxeles y 4 de relleno
        let verde_de_relleno = [0x00, 0xFF, 0x00, 0x00];
        let mut datos = vec![0u8; paso * 2];
        for fila in 0..2 {
            for x in 0..ancho {
                let color = if fila == 0 {
                    [0x00, 0x00, 0xFF, 0x00] // rojo, en orden B G R X
                } else {
                    [0xFF, 0x00, 0x00, 0x00] // azul
                };
                let desde = fila * paso + x * 4;
                datos[desde..desde + 4].copy_from_slice(&color);
            }
            let relleno = fila * paso + ancho * 4;
            datos[relleno..relleno + 4].copy_from_slice(&verde_de_relleno);
        }

        let imagen = a_rgba(&datos, Format::Xrgb8888, ancho as u32, 2, paso as u32).unwrap();

        assert_eq!(
            imagen.get_pixel(0, 0),
            &image::Rgba([0xFF, 0x00, 0x00, 0xFF])
        );
        // El primero de la segunda fila: leyendo de corrido, acá caería el
        // relleno verde.
        assert_eq!(
            imagen.get_pixel(0, 1),
            &image::Rgba([0x00, 0x00, 0xFF, 0xFF])
        );
    }

    #[test]
    fn un_buffer_mas_corto_de_lo_que_dice_no_revienta() {
        // Devuelve `None` en vez de paniquear: el compositor es de afuera.
        assert!(a_rgba(&[0, 0, 0, 0], Format::Xrgb8888, 100, 100, 400).is_none());
    }

    #[test]
    fn con_la_geometria_a_medias_todavia_no_se_termino() {
        // Los dos eventos de `xdg_output` llegan por separado y sin orden
        // garantizado. Dar la geometría por completa con el primero dejaba el
        // lienzo de cero píxeles: la posición puesta, el tamaño en nada, y esto
        // se daba por listo. Lo marcó la revisión.
        let mut pantallas = HashMap::new();
        pantallas.insert(
            1,
            Pantalla {
                posicion: Some((0, 0)),
                pixeles: Some(lisa(10, 10, [0, 0, 0, 255])),
                ..Pantalla::default()
            },
        );

        assert!(!todas_contestaron(&pantallas), "falta el tamaño");

        pantallas.get_mut(&1).unwrap().tamanio = Some((10, 10));
        assert!(todas_contestaron(&pantallas));
    }

    #[test]
    fn una_pantalla_perdida_no_corta_el_bucle() {
        // El bucle espera a que **todas** contesten, y una fallada ya contestó.
        // Con el error global de antes esto cortaba la captura entera, así que
        // la prueba de composición que decía cubrirlo no cubría nada: el flujo
        // no llegaba a componer.
        let mut pantallas = HashMap::new();
        pantallas.insert(1, pantalla((0, 0, 10, 10), lisa(10, 10, [1, 2, 3, 255])));
        pantallas.insert(2, Pantalla::default());

        assert!(
            !todas_contestaron(&pantallas),
            "la segunda no contestó todavía"
        );

        dar_por_perdida(&mut pantallas, 2, "el compositor la rechazó".into());

        assert!(todas_contestaron(&pantallas), "una fallada ya contestó");
        assert_eq!(
            pantallas[&2].motivo.as_deref(),
            Some("el compositor la rechazó")
        );
        // Y la que sí salió sigue entera, que es el punto.
        assert_eq!(
            componer(pantallas).ok().map(|l| l.imagen.dimensions()),
            Some((10, 10)),
            "la que sí salió sigue entera"
        );
    }

    #[test]
    fn sin_ninguna_usable_el_error_dice_por_que() {
        // «ninguna pantalla se pudo copiar» a secas no deja arreglar nada.
        let mut pantallas = HashMap::new();
        pantallas.insert(
            1,
            Pantalla {
                fallada: true,
                motivo: Some("el compositor la rechazó".into()),
                ..Pantalla::default()
            },
        );

        // Sin `unwrap_err`: pediría `Debug` sobre el lienzo, y derivarlo
        // volcaría la imagen entera en el mensaje de una prueba que falle.
        let Err(error) = componer(pantallas) else {
            panic!("con todas las pantallas perdidas no puede haber captura");
        };

        assert!(error.contains("el compositor la rechazó"), "{error}");
    }

    #[test]
    fn los_formatos_de_menos_de_cuatro_bytes_no_se_aceptan() {
        // `a_rgba` avanza de a cuatro bytes por píxel. Con uno de 24 bits
        // leería corrido y devolvería una imagen torcida en vez de un error.
        assert!(formato_conocido(Format::Xrgb8888));
        assert!(formato_conocido(Format::Argb8888));
        assert!(formato_conocido(Format::Xbgr8888));
        assert!(!formato_conocido(Format::Bgr888));
        assert!(!formato_conocido(Format::Rgb565));
    }

    #[test]
    fn el_contenido_dado_vuelta_se_endereza() {
        // El compositor puede entregar las filas de abajo hacia arriba y
        // avisarlo con `YInvert`. Sin mirar esa bandera la captura sale
        // espejada en vertical, y no hay nada que lo delate hasta que alguien
        // la mira. Acá se comprueba la operación que hace el manejador: dos
        // filas de colores distintos, dadas vuelta.
        let mut cruda = RgbaImage::new(1, 2);
        cruda.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        cruda.put_pixel(0, 1, image::Rgba([0, 0, 255, 255]));

        let enderezada = image::imageops::flip_vertical(&cruda);

        assert_eq!(enderezada.get_pixel(0, 0), &image::Rgba([0, 0, 255, 255]));
        assert_eq!(enderezada.get_pixel(0, 1), &image::Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn una_sola_pantalla_sale_tal_cual() {
        let mut pantallas = HashMap::new();
        pantallas.insert(
            1,
            pantalla((0, 1080, 1920, 1080), lisa(1920, 1080, [1, 2, 3, 255])),
        );

        let lienzo = componer(pantallas).unwrap();

        // 1920x1080 y no 1920x2160: el lienzo abarca lo que ocupan las
        // pantallas, que acá no empieza en el origen. Es lo que hace `grim`, y
        // lo que `Region::en_la_captura` da por sentado.
        assert_eq!(
            (lienzo.imagen.width(), lienzo.imagen.height()),
            (1920, 1080)
        );
        assert_eq!(lienzo.imagen.get_pixel(0, 0), &image::Rgba([1, 2, 3, 255]));
    }

    #[test]
    fn dos_pantallas_apiladas_van_una_debajo_de_la_otra() {
        let mut pantallas = HashMap::new();
        pantallas.insert(
            1,
            pantalla((0, 0, 100, 50), lisa(100, 50, [255, 0, 0, 255])),
        );
        pantallas.insert(
            2,
            pantalla((0, 50, 100, 50), lisa(100, 50, [0, 0, 255, 255])),
        );

        let lienzo = componer(pantallas).unwrap();

        assert_eq!((lienzo.imagen.width(), lienzo.imagen.height()), (100, 100));
        assert_eq!(lienzo.imagen.get_pixel(0, 0)[0], 255, "arriba la roja");
        assert_eq!(lienzo.imagen.get_pixel(0, 99)[2], 255, "abajo la azul");
    }

    #[test]
    fn la_pantalla_de_mas_escala_manda_el_tamanio_del_lienzo() {
        // Con una al 100% y otra al 200%, bajar todo a la menor tiraría la
        // mitad de los píxeles que la segunda sí tiene.
        let mut pantallas = HashMap::new();
        pantallas.insert(
            1,
            pantalla((0, 0, 100, 50), lisa(100, 50, [255, 0, 0, 255])),
        );
        pantallas.insert(
            2,
            pantalla((100, 0, 100, 50), lisa(200, 100, [0, 0, 255, 255])),
        );

        let lienzo = componer(pantallas).unwrap();

        assert_eq!((lienzo.imagen.width(), lienzo.imagen.height()), (400, 100));
        // La de escala 1 se estira para ocupar su mitad del lienzo.
        assert_eq!(lienzo.imagen.get_pixel(10, 10)[0], 255);
        assert_eq!(lienzo.imagen.get_pixel(300, 10)[2], 255);
    }

    #[test]
    fn una_pantalla_que_fallo_no_arrastra_a_las_demas() {
        // Si una copia falla, lo que se puede entregar es el resto. Una captura
        // parcial es mejor que ninguna.
        let mut pantallas = HashMap::new();
        pantallas.insert(
            1,
            pantalla((0, 0, 100, 50), lisa(100, 50, [255, 0, 0, 255])),
        );
        pantallas.insert(
            2,
            Pantalla {
                posicion: Some((100, 0)),
                tamanio: Some((100, 50)),
                fallada: true,
                motivo: Some("el compositor la rechazó".into()),
                ..Pantalla::default()
            },
        );

        let lienzo = componer(pantallas).unwrap();

        assert_eq!((lienzo.imagen.width(), lienzo.imagen.height()), (100, 50));
    }

    #[test]
    fn sin_ninguna_pantalla_copiada_se_dice() {
        let pantallas = HashMap::new();
        assert!(componer(pantallas).is_err());
    }

    #[test]
    fn las_pantallas_salen_nombradas_y_ubicadas() {
        // Es lo que `--salida` necesita para elegir una, y el selector para
        // decir cuál se está mirando.
        let mut pantallas = HashMap::new();
        pantallas.insert(
            1,
            pantalla_llamada(
                "HDMI-A-1",
                (0, 50, 100, 50),
                lisa(100, 50, [0, 0, 255, 255]),
            ),
        );
        pantallas.insert(
            2,
            pantalla_llamada("eDP-1", (0, 0, 100, 50), lisa(100, 50, [255, 0, 0, 255])),
        );

        let lienzo = componer(pantallas).unwrap();

        // Ordenadas de arriba abajo, no en el orden de un `HashMap`: la lista
        // se muestra en el selector, y una que cambia de orden en cada
        // ejecución obliga a leerla entera cada vez.
        let nombres: Vec<&str> = lienzo
            .copias
            .iter()
            .map(|c| c.salida.nombre.as_str())
            .collect();
        assert_eq!(nombres, vec!["eDP-1", "HDMI-A-1"]);
        assert_eq!(
            lienzo.copias[1].salida.area(),
            crate::captura::Salida {
                x: 0,
                y: 50,
                ancho: 100,
                alto: 50
            }
        );
    }

    #[test]
    fn sin_nombre_del_compositor_se_las_llama_por_su_lugar() {
        // Con `xdg_output` versión 1 no llega el evento `name`. Un nombre
        // inventado no sirve para reconocer el monitor, pero sí para poder
        // nombrarlo en `--salida`, que sin esto quedaría sin ninguna opción.
        let mut pantallas = HashMap::new();
        pantallas.insert(1, pantalla((0, 0, 100, 50), lisa(100, 50, [1, 2, 3, 255])));

        let lienzo = componer(pantallas).unwrap();
        assert_eq!(lienzo.copias[0].salida.nombre, "pantalla-1");
    }

    #[test]
    fn una_pantalla_que_fallo_no_se_ofrece() {
        // No está en la imagen, así que pedirla daría un recorte de píxeles
        // negros en vez de un error.
        let mut pantallas = HashMap::new();
        pantallas.insert(
            1,
            pantalla_llamada("eDP-1", (0, 0, 100, 50), lisa(100, 50, [255, 0, 0, 255])),
        );
        pantallas.insert(
            2,
            Pantalla {
                fallada: true,
                ..pantalla_llamada(
                    "HDMI-A-1",
                    (0, 50, 100, 50),
                    lisa(100, 50, [0, 0, 255, 255]),
                )
            },
        );

        let lienzo = componer(pantallas).unwrap();
        let nombres: Vec<&str> = lienzo
            .copias
            .iter()
            .map(|c| c.salida.nombre.as_str())
            .collect();
        assert_eq!(nombres, vec!["eDP-1"]);
    }

    #[test]
    fn la_razon_unica_del_lienzo_es_exacta_con_escalas_distintas() {
        // Acá decía lo contrario, y de ese comentario salió el informe de que el
        // recorte se corría. No se corre: `componer` lleva **todas** las
        // pantallas a la escala mayor, así que la imagen mide exactamente el
        // layout por esa razón y `escala_de` la calcula sin error. Esta prueba
        // está para que la afirmación quede fijada y no haya que volver a
        // deducirla leyendo el bucle.
        let mut pantallas = HashMap::new();
        // Una al 100 %: 100x50 lógicos, 100x50 píxeles.
        pantallas.insert(
            1,
            pantalla_llamada("normal", (0, 0, 100, 50), lisa(100, 50, [255, 0, 0, 255])),
        );
        // Otra al 200 %: 100x50 lógicos, 200x100 píxeles.
        pantallas.insert(
            2,
            pantalla_llamada("hidpi", (0, 50, 100, 50), lisa(200, 100, [0, 0, 255, 255])),
        );

        let lienzo = componer(pantallas).unwrap();
        let layout = crate::captura::layout_de(
            &lienzo
                .copias
                .iter()
                .map(|c| c.salida.clone())
                .collect::<Vec<_>>(),
        );

        // El layout mide 100x100 y la imagen 200x200: la razón es 2 en los dos
        // ejes, y es la misma para toda la imagen.
        assert_eq!((lienzo.imagen.width(), lienzo.imagen.height()), (200, 200));
        assert_eq!(
            crate::captura::escala_de(lienzo.imagen.dimensions(), layout),
            (2.0, 2.0)
        );
    }

    #[test]
    fn la_de_menor_escala_guarda_sus_pixeles_y_la_otra_no() {
        // Los de la estirada hacen falta: recortar del lienzo devuelve una
        // imagen del doble de tamaño e interpolada. Los de la otra ya están en
        // el lienzo tal cual, y guardarlos sería duplicar decenas de megabytes.
        let mut pantallas = HashMap::new();
        pantallas.insert(
            1,
            pantalla_llamada("normal", (0, 0, 100, 50), lisa(100, 50, [255, 0, 0, 255])),
        );
        pantallas.insert(
            2,
            pantalla_llamada("hidpi", (0, 50, 100, 50), lisa(200, 100, [0, 0, 255, 255])),
        );

        let lienzo = componer(pantallas).unwrap();

        let normal = &lienzo.copias[0];
        let hidpi = &lienzo.copias[1];
        assert_eq!(normal.salida.nombre, "normal");
        assert!(normal.nativos.is_some(), "la estirada guarda los suyos");
        assert_eq!(normal.escala(), Some((1.0, 1.0)));
        assert!(hidpi.nativos.is_none(), "la que manda la escala, no");
        assert_eq!(hidpi.escala(), None);
    }

    #[test]
    fn con_una_sola_pantalla_no_se_guarda_nada_aparte() {
        // El caso de todos los días: el lienzo **es** la pantalla, así que una
        // segunda copia serían ocho megabytes de nada.
        let mut pantallas = HashMap::new();
        pantallas.insert(1, pantalla((0, 0, 100, 50), lisa(100, 50, [1, 2, 3, 255])));

        let lienzo = componer(pantallas).unwrap();
        assert!(lienzo.copias[0].nativos.is_none());
    }

    #[test]
    fn dos_pantallas_de_escala_fraccionaria_no_dejan_costura() {
        // Con 1,25 los bordes caen entre píxeles. Redondeando cada ancho por su
        // cuenta, dos pantallas vecinas se pisan o dejan una columna
        // transparente en el medio; redondeando los bordes, encajan.
        let mut pantallas = HashMap::new();
        // 100 lógicos al 125 % son 125 píxeles.
        pantallas.insert(
            1,
            pantalla_llamada("izq", (0, 0, 100, 40), lisa(125, 50, [255, 0, 0, 255])),
        );
        pantallas.insert(
            2,
            pantalla_llamada("der", (100, 0, 100, 40), lisa(125, 50, [0, 0, 255, 255])),
        );

        let lienzo = componer(pantallas).unwrap();
        assert_eq!((lienzo.imagen.width(), lienzo.imagen.height()), (250, 50));

        // Ni una sola columna sin pintar, y cada mitad con su color.
        for x in 0..250 {
            let p = lienzo.imagen.get_pixel(x, 25);
            assert_eq!(p[3], 255, "columna {x} transparente");
            if x < 125 {
                assert_eq!(p[0], 255, "columna {x} tendría que ser la roja");
            } else {
                assert_eq!(p[2], 255, "columna {x} tendría que ser la azul");
            }
        }
    }
}
