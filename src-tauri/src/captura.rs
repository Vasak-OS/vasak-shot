//! Sacar la captura, y recortarla.
//!
//! # Por qué se captura antes de mostrar nada
//!
//! Crear una ventana de Tauri bajo demanda tarda **entre uno y dos segundos** —lo
//! medimos en las ventanas del escritorio y del selector de acentos—. Una
//! herramienta de capturas que abre ventana y *después* captura pierde justo el
//! momento que se quería guardar: el menú que estaba abierto se cerró, el cursor
//! se movió, la notificación desapareció.
//!
//! Así que el orden es al revés. Primero la captura, que tarda unos 100 ms, y
//! recién entonces la ventana, que muestra ese cuadro **congelado**. La lentitud
//! de la interfaz deja de importar porque el instante ya está guardado, y de paso
//! la selección se hace sobre una imagen quieta en lugar de sobre una pantalla
//! que sigue cambiando debajo.
//!
//! # Por qué `grim` y no el protocolo de Wayland a mano
//!
//! `grim` ya habla `zwlr_screencopy` correctamente, maneja varias salidas y sus
//! escalas, y viene instalado en la ISO. Reimplementarlo sería reescribir la parte
//! difícil para llegar al mismo lugar. Se recorta acá y no con `grim -g` por dos
//! razones: una sola captura en lugar de dos, y porque la geometría de `grim` está
//! en coordenadas del layout de salidas —que en esta máquina no coinciden con las
//! de la pantalla, así que `-g "0,0 400x300"` contesta «did not intersect with any
//! outputs"—.

use std::path::{Path, PathBuf};

/// El programa que toma los píxeles.
const GRIM: &str = "grim";

/// Una captura ya tomada, esperando en disco.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Captura {
    pub ruta: PathBuf,
    pub ancho: u32,
    pub alto: u32,
}

/// Una región elegida con el ratón.
///
/// Los campos son `i32` y no `u32` a propósito: arrastrando hacia arriba o hacia
/// la izquierda, el ancho y el alto salen **negativos**, y quien los recibe tiene
/// que poder representarlos antes de normalizarlos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub ancho: i32,
    pub alto: i32,
}

impl Region {
    /// Deja la región con origen arriba a la izquierda y medidas positivas.
    ///
    /// Arrastrar hacia arriba o hacia la izquierda es tan válido como hacia abajo
    /// y a la derecha, y produce medidas negativas. Sin normalizar, el recorte
    /// recibe un ancho negativo y devuelve una imagen vacía o falla — según la
    /// biblioteca, sin decir por qué.
    pub fn normalizada(self) -> Self {
        let (x, ancho) = if self.ancho < 0 {
            (self.x + self.ancho, -self.ancho)
        } else {
            (self.x, self.ancho)
        };
        let (y, alto) = if self.alto < 0 {
            (self.y + self.alto, -self.alto)
        } else {
            (self.y, self.alto)
        };
        Self { x, y, ancho, alto }
    }

    /// Recorta la región a lo que de verdad existe en la imagen.
    ///
    /// Se puede arrastrar más allá del borde de la pantalla —el puntero llega, la
    /// imagen no—, y también empezar fuera. Sin este ajuste, el recorte pide
    /// píxeles que no están: `image` paniquea con un `sub_image` fuera de rango.
    ///
    /// Devuelve `None` si no queda nada: una selección enteramente fuera de la
    /// imagen, o de cero píxeles porque fue un clic sin arrastrar.
    pub fn recortada_a(self, ancho_imagen: u32, alto_imagen: u32) -> Option<Self> {
        let n = self.normalizada();

        let x0 = n.x.max(0);
        let y0 = n.y.max(0);
        let x1 = (n.x + n.ancho).min(ancho_imagen as i32);
        let y1 = (n.y + n.alto).min(alto_imagen as i32);

        if x1 <= x0 || y1 <= y0 {
            return None;
        }

        Some(Self {
            x: x0,
            y: y0,
            ancho: x1 - x0,
            alto: y1 - y0,
        })
    }
}

/// Alto y ancho de un PNG, leídos de su cabecera.
///
/// Se leen los 24 primeros bytes en lugar de decodificar la imagen entera: para
/// saber el tamaño de una captura de 1920x1080 no hace falta traer ocho megabytes
/// de píxeles a memoria.
///
/// El formato lo fija la especificación: 8 bytes de firma, 4 de longitud, `IHDR`,
/// y ahí el ancho y el alto como enteros de 32 bits big-endian.
pub fn dimensiones_png(bytes: &[u8]) -> Option<(u32, u32)> {
    const FIRMA: &[u8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24 || !bytes.starts_with(FIRMA) || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let ancho = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let alto = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    // Un PNG de cero píxeles no es válido, y aceptarlo haría que el recorte
    // devolviera `None` sin poder distinguirlo de una selección vacía.
    if ancho == 0 || alto == 0 {
        return None;
    }
    Some((ancho, alto))
}

/// El nombre del archivo para una captura, a partir de la fecha.
///
/// Con segundos, porque dos capturas seguidas dentro del mismo minuto son lo
/// normal —se prueba un encuadre, se corrige, se vuelve a sacar— y sin ellos la
/// segunda pisaría a la primera.
pub fn nombre_de_archivo(anio: i32, mes: u32, dia: u32, hora: u32, minuto: u32, segundo: u32) -> String {
    format!("Captura {anio:04}-{mes:02}-{dia:02} {hora:02}.{minuto:02}.{segundo:02}.png")
}

/// Toma la captura de todo lo que se ve.
pub fn capturar(destino: &Path) -> Result<Captura, String> {
    let salida = std::process::Command::new(GRIM)
        .arg(destino)
        .output()
        .map_err(|e| format!("no se pudo ejecutar {GRIM}: {e}"))?;

    if !salida.status.success() {
        let motivo = String::from_utf8_lossy(&salida.stderr).trim().to_string();
        return Err(if motivo.is_empty() {
            format!("{GRIM} falló sin decir por qué")
        } else {
            motivo
        });
    }

    // Se leen sólo los bytes de la cabecera, no el archivo entero.
    let cabecera = leer_cabecera(destino)?;
    let (ancho, alto) = dimensiones_png(&cabecera)
        .ok_or_else(|| format!("{} no parece un PNG válido", destino.display()))?;

    Ok(Captura {
        ruta: destino.to_path_buf(),
        ancho,
        alto,
    })
}

/// Los primeros bytes de un archivo, los que alcanzan para el `IHDR`.
fn leer_cabecera(ruta: &Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut archivo =
        std::fs::File::open(ruta).map_err(|e| format!("no se pudo abrir {}: {e}", ruta.display()))?;
    let mut cabecera = [0u8; 24];
    let leidos = archivo
        .read(&mut cabecera)
        .map_err(|e| format!("no se pudo leer {}: {e}", ruta.display()))?;
    Ok(cabecera[..leidos].to_vec())
}

// ── Recorte y entrega ──────────────────────────────────────

/// Recorta la captura a la región elegida y la guarda en `destino`.
///
/// El recorte se hace acá y no con `grim -g` por dos razones: una sola captura en
/// lugar de dos —así lo que se guarda es exactamente el instante que se vio, no
/// uno posterior— y porque la geometría de `grim` está en coordenadas del layout
/// de salidas, que no son las de la pantalla.
pub fn recortar(origen: &Path, region: Region, destino: &Path) -> Result<(), String> {
    let imagen = image::open(origen)
        .map_err(|e| format!("no se pudo leer {}: {e}", origen.display()))?;

    let (ancho, alto) = (imagen.width(), imagen.height());
    let region = region
        .recortada_a(ancho, alto)
        .ok_or_else(|| "la selección quedó fuera de la imagen".to_string())?;

    // `to_image` copia sólo la región, no la imagen entera.
    let recorte = image::imageops::crop_imm(
        &imagen,
        region.x as u32,
        region.y as u32,
        region.ancho as u32,
        region.alto as u32,
    )
    .to_image();

    if let Some(padre) = destino.parent() {
        std::fs::create_dir_all(padre)
            .map_err(|e| format!("no se pudo crear {}: {e}", padre.display()))?;
    }

    recorte
        .save(destino)
        .map_err(|e| format!("no se pudo guardar {}: {e}", destino.display()))
}

/// Copia un PNG al portapapeles.
///
/// Por `wl-copy` y no por la API de Tauri: el portapapeles de Tauri maneja texto,
/// y lo que hace útil una captura es poder pegarla como imagen en un chat o un
/// documento. `wl-copy` es el que sabe declarar el tipo `image/png` en Wayland.
pub fn copiar_al_portapapeles(ruta: &Path) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let bytes = std::fs::read(ruta)
        .map_err(|e| format!("no se pudo leer {}: {e}", ruta.display()))?;

    let mut hijo = Command::new("wl-copy")
        .args(["--type", "image/png"])
        .stdin(Stdio::piped())
        // La salida va a `null`, y no es cosmético.
        //
        // `wl-copy` se demoniza para seguir sirviendo el portapapeles después de
        // que su padre termina, y ese proceso hereda nuestra salida estándar. Con
        // los descriptores heredados, **quien nos llamó se queda esperando que el
        // pipe se cierre** — y no se cierra mientras el portapapeles tenga la
        // imagen. Desde una terminal se ve como si la herramienta se hubiera
        // colgado; desde un atajo de teclado, como si nunca hubiera terminado.
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("no se pudo ejecutar wl-copy: {e}"))?;

    // El `take` es necesario: dejando la tubería abierta, `wl-copy` sigue
    // esperando más datos y `wait` no vuelve nunca.
    hijo.stdin
        .take()
        .ok_or_else(|| "wl-copy no aceptó la entrada".to_string())?
        .write_all(&bytes)
        .map_err(|e| format!("no se pudo escribir al portapapeles: {e}"))?;

    let estado = hijo
        .wait()
        .map_err(|e| format!("wl-copy no terminó: {e}"))?;

    if estado.success() {
        Ok(())
    } else {
        Err("wl-copy falló".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrastrar_en_cualquier_direccion_da_la_misma_region() {
        // Las cuatro formas de seleccionar el mismo rectángulo: desde cada
        // esquina. Sin normalizar, tres de las cuatro llegan con medidas
        // negativas al recorte.
        let esperada = Region { x: 10, y: 20, ancho: 30, alto: 40 };

        let desde_arriba_izquierda = Region { x: 10, y: 20, ancho: 30, alto: 40 };
        let desde_arriba_derecha = Region { x: 40, y: 20, ancho: -30, alto: 40 };
        let desde_abajo_izquierda = Region { x: 10, y: 60, ancho: 30, alto: -40 };
        let desde_abajo_derecha = Region { x: 40, y: 60, ancho: -30, alto: -40 };

        for region in [
            desde_arriba_izquierda,
            desde_arriba_derecha,
            desde_abajo_izquierda,
            desde_abajo_derecha,
        ] {
            assert_eq!(region.normalizada(), esperada, "arrastrando desde {region:?}");
        }
    }

    #[test]
    fn una_seleccion_que_se_pasa_del_borde_se_recorta() {
        // El puntero llega más allá de la imagen; la imagen no.
        let region = Region { x: 1800, y: 1000, ancho: 400, alto: 400 };
        assert_eq!(
            region.recortada_a(1920, 1080),
            Some(Region { x: 1800, y: 1000, ancho: 120, alto: 80 })
        );
    }

    #[test]
    fn una_seleccion_que_empieza_afuera_tambien() {
        let region = Region { x: -50, y: -50, ancho: 200, alto: 200 };
        assert_eq!(
            region.recortada_a(1920, 1080),
            Some(Region { x: 0, y: 0, ancho: 150, alto: 150 })
        );
    }

    #[test]
    fn una_seleccion_vacia_no_es_una_region() {
        // Un clic sin arrastrar, y una selección enteramente fuera de la imagen.
        // Devolver una región de cero píxeles haría que el recorte paniqueara o
        // guardara un archivo vacío.
        assert_eq!(Region { x: 10, y: 10, ancho: 0, alto: 0 }.recortada_a(1920, 1080), None);
        assert_eq!(Region { x: 10, y: 10, ancho: 50, alto: 0 }.recortada_a(1920, 1080), None);
        assert_eq!(Region { x: 5000, y: 5000, ancho: 10, alto: 10 }.recortada_a(1920, 1080), None);
        assert_eq!(Region { x: -100, y: -100, ancho: 50, alto: 50 }.recortada_a(1920, 1080), None);
    }

    #[test]
    fn la_region_completa_sobrevive_intacta() {
        let region = Region { x: 0, y: 0, ancho: 1920, alto: 1080 };
        assert_eq!(region.recortada_a(1920, 1080), Some(region));
    }

    /// La cabecera de un PNG armada a mano, que es lo que lee `dimensiones_png`.
    fn cabecera_png(ancho: u32, alto: u32) -> Vec<u8> {
        let mut b = b"\x89PNG\r\n\x1a\n".to_vec();
        b.extend_from_slice(&13u32.to_be_bytes());
        b.extend_from_slice(b"IHDR");
        b.extend_from_slice(&ancho.to_be_bytes());
        b.extend_from_slice(&alto.to_be_bytes());
        b
    }

    #[test]
    fn las_dimensiones_salen_de_la_cabecera() {
        assert_eq!(dimensiones_png(&cabecera_png(1920, 1080)), Some((1920, 1080)));
        assert_eq!(dimensiones_png(&cabecera_png(3840, 2160)), Some((3840, 2160)));
    }

    #[test]
    fn lo_que_no_es_un_png_se_rechaza() {
        // Si `grim` fallara dejando un archivo a medias, o escribiera otra cosa,
        // hay que decirlo en lugar de seguir con dimensiones inventadas.
        assert_eq!(dimensiones_png(b""), None);
        assert_eq!(dimensiones_png(b"no soy un png en absoluto..."), None);
        assert_eq!(dimensiones_png(&cabecera_png(1920, 1080)[..20]), None, "cabecera cortada");
        assert_eq!(dimensiones_png(&cabecera_png(0, 1080)), None, "cero de ancho");
        assert_eq!(dimensiones_png(&cabecera_png(1920, 0)), None, "cero de alto");
    }

    /// El recorte de verdad, sobre una imagen conocida.
    ///
    /// Se arma un PNG con cuatro cuadrantes de colores distintos y se recorta uno:
    /// así el test comprueba no sólo el tamaño sino **que se recortó el pedazo
    /// correcto**. Con un solo color, invertir dos coordenadas pasaría inadvertido.
    #[test]
    fn se_recorta_el_pedazo_correcto() {
        let dir = std::env::temp_dir().join(format!("vsk-shot-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("crear el directorio");
        let origen = dir.join("origen.png");
        let destino = dir.join("recorte.png");

        // 100x100: arriba-izq rojo, arriba-der verde, abajo-izq azul, abajo-der blanco.
        let mut imagen = image::RgbImage::new(100, 100);
        for (x, y, pixel) in imagen.enumerate_pixels_mut() {
            *pixel = match (x < 50, y < 50) {
                (true, true) => image::Rgb([255, 0, 0]),
                (false, true) => image::Rgb([0, 255, 0]),
                (true, false) => image::Rgb([0, 0, 255]),
                (false, false) => image::Rgb([255, 255, 255]),
            };
        }
        imagen.save(&origen).expect("guardar el origen");

        // Un rectángulo **asimétrico y no cuadrado**, dentro del cuadrante verde,
        // y seleccionado desde la esquina opuesta para que pase por la
        // normalización.
        //
        // Las dos cosas importan: con un cuadrado en posición simétrica —lo que
        // este test tenía al principio— invertir `x` e `y` daba exactamente el
        // mismo recorte y el test pasaba igual. Así, invertirlas cae en el
        // cuadrante azul y con las medidas al revés.
        let region = Region { x: 90, y: 50, ancho: -30, alto: -40 };
        recortar(&origen, region, &destino).expect("recortar");

        let recorte = image::open(&destino).expect("leer el recorte").to_rgb8();
        assert_eq!(recorte.dimensions(), (30, 40), "el ancho y el alto no son intercambiables");
        assert_eq!(
            *recorte.get_pixel(15, 20),
            image::Rgb([0, 255, 0]),
            "se recortó otro pedazo de la imagen"
        );

        // Y una selección fuera de la imagen no escribe nada.
        let afuera = Region { x: 500, y: 500, ancho: 10, alto: 10 };
        assert!(recortar(&origen, afuera, &dir.join("no.png")).is_err());
        assert!(!dir.join("no.png").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_nombre_lleva_la_hora_con_segundos() {
        // Sin segundos, dos capturas del mismo minuto se pisan — y probar un
        // encuadre, corregirlo y volver a sacar es el caso normal.
        assert_eq!(
            nombre_de_archivo(2026, 8, 26, 9, 5, 3),
            "Captura 2026-08-26 09.05.03.png"
        );
        assert_ne!(
            nombre_de_archivo(2026, 8, 26, 9, 5, 3),
            nombre_de_archivo(2026, 8, 26, 9, 5, 4)
        );
    }
}
