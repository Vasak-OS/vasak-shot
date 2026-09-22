//! La captura que ya viene compuesta desde el selector.
//!
//! # Por qué los píxeles los compone el frontend y no esto
//!
//! El plan escrito en el issue era al revés: que el selector mandara la lista de
//! anotaciones y que Rust las pintara sobre el recorte. La razón era no cruzar
//! megabytes por el IPC, y sigue siendo cierta — pero pintarlas acá obliga a que
//! **dos** dibujantes distintos den el mismo resultado, y no lo dan:
//!
//! - El `<canvas>` suaviza los bordes de todo lo que dibuja, y un dibujante de
//!   píxeles a mano no. Cada recuadro y cada flecha saldrían distintos de como
//!   se vieron.
//! - El texto necesita una fuente y sus métricas. Hacer coincidir la tipografía
//!   del navegador con la de un rasterizador de Rust es una pelea que no se
//!   gana, y la diferencia se ve en cada letra.
//!
//! Y el issue de tapar lo privado pide explícitamente que la vista previa y el
//! archivo coincidan. La única manera de que coincidan **siempre** es que sean
//! la misma composición: el selector dibuja una vez, en el tamaño real de la
//! captura, y lo que se guarda es exactamente ese mapa de bits.
//!
//! El costo es el que se quería evitar: un PNG que cruza el IPC. Va por el canal
//! **crudo** de Tauri —el cuerpo entero del pedido son los bytes— y no adentro
//! de un JSON, que los convertiría en una lista de números y sí sería caro.

use std::path::{Path, PathBuf};

/// Los ocho bytes con los que empieza todo PNG.
const FIRMA_PNG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Si eso es un PNG.
///
/// Se comprueba antes de escribir: lo que llega es el cuerpo crudo de un pedido,
/// y escribir con extensión `.png` algo que no lo es deja en la carpeta de
/// capturas un archivo que ningún visor abre y que nadie sabe por qué está.
pub fn es_png(bytes: &[u8]) -> bool {
    bytes.len() > FIRMA_PNG.len() && bytes[..FIRMA_PNG.len()] == FIRMA_PNG
}

/// Escribe los bytes en `destino`, creando la carpeta si hace falta.
pub fn escribir(bytes: &[u8], destino: &Path) -> Result<(), String> {
    if !es_png(bytes) {
        return Err("lo que llegó no es un PNG".to_string());
    }
    if let Some(padre) = destino.parent() {
        std::fs::create_dir_all(padre)
            .map_err(|e| format!("no se pudo crear {}: {e}", padre.display()))?;
    }
    std::fs::write(destino, bytes)
        .map_err(|e| format!("no se pudo guardar {}: {e}", destino.display()))
}

/// Crea el archivo **vacío, privado y en exclusiva**.
///
/// Para los temporales, que van a `/tmp` y ahí escriben todos. Sin esto hay dos
/// agujeros, y los dos importan justamente en una herramienta de capturas:
///
/// - `std::fs::write` crea con `0o666 & !umask`, o sea `0o644` con el umask
///   habitual: una captura que alguien pidió **sólo copiar** queda un rato en el
///   disco legible por cualquier otra cuenta de la máquina.
/// - Si la ruta ya existe, escribir la sigue. Un enlace simbólico puesto antes
///   por otro convierte «guardar la captura» en «escribirle su archivo».
///
/// `create_new` falla si ya hay algo, así que el enlace deja de servir, y el
/// modo `0o600` cierra lo otro. Quien escriba después —`image::save`, o
/// `escribir`— abre el archivo que ya existe y **no** le cambia los permisos.
pub fn crear_privado(ruta: &Path) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(ruta)
        .map(|_| ())
        .map_err(|e| format!("no se pudo crear {}: {e}", ruta.display()))
}

/// Un archivo temporal para lo que sólo se va a copiar, ya creado y privado.
///
/// Quien copia quiere pegar, no acumular archivos que después hay que borrar a
/// mano.
///
/// El nombre lleva el pid **y el reloj**: con el pid solo, dos capturas seguidas
/// del mismo proceso piden el mismo nombre y la segunda choca contra el archivo
/// de la primera, que `create_new` rechaza — y con razón.
pub fn temporal(prefijo: &str) -> Result<PathBuf, String> {
    let marca = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let ruta = std::env::temp_dir().join(format!(
        "vasak-shot-{prefijo}-{}-{marca}.png",
        std::process::id()
    ));
    crear_privado(&ruta)?;
    Ok(ruta)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// El PNG más chico que existe: firma y un IHDR de 1x1.
    fn un_png() -> Vec<u8> {
        let mut bytes = FIRMA_PNG.to_vec();
        bytes.extend_from_slice(b"\x00\x00\x00\x0dIHDR");
        bytes
    }

    #[test]
    fn se_reconoce_un_png() {
        assert!(es_png(&un_png()));
    }

    #[test]
    fn lo_que_no_es_png_no_pasa() {
        // El cuerpo crudo de un pedido puede ser cualquier cosa. Escribirlo con
        // extensión `.png` dejaría en la carpeta de capturas un archivo que
        // ningún visor abre.
        assert!(!es_png(b""));
        assert!(!es_png(b"no soy un png"));
        // Ni la firma sola, sin nada atrás.
        assert!(!es_png(&FIRMA_PNG));
        // Ni un JPEG.
        assert!(!es_png(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 0, 0, 0, 0]));
    }

    #[test]
    fn el_temporal_no_lo_puede_leer_nadie_mas() {
        // Una captura que alguien pidió **sólo copiar** no puede quedar en
        // `/tmp` legible por las otras cuentas de la máquina.
        use std::os::unix::fs::PermissionsExt;
        let ruta = temporal("prueba").expect("tendría que haberse creado");
        let modo = std::fs::metadata(&ruta).unwrap().permissions().mode() & 0o777;
        assert_eq!(modo, 0o600, "quedó en {modo:o}");

        // Y escribir encima no le cambia los permisos.
        escribir(&un_png(), &ruta).unwrap();
        let despues = std::fs::metadata(&ruta).unwrap().permissions().mode() & 0o777;
        assert_eq!(despues, 0o600);

        let _ = std::fs::remove_file(&ruta);
    }

    #[test]
    fn el_temporal_no_pisa_lo_que_ya_esta() {
        // Un enlace simbólico puesto antes por otro convertiría «guardar la
        // captura» en «escribirle su archivo».
        let ruta = temporal("ocupado").expect("tendría que haberse creado");
        assert!(
            crear_privado(&ruta).is_err(),
            "tendría que haber fallado sobre un archivo que ya existe"
        );
        let _ = std::fs::remove_file(&ruta);
    }

    #[test]
    fn dos_temporales_seguidos_no_chocan() {
        // Con el pid solo, la segunda captura del mismo proceso chocaba contra
        // el archivo de la primera.
        let uno = temporal("seguidos").expect("uno");
        let otro = temporal("seguidos").expect("otro");
        assert_ne!(uno, otro);
        let _ = std::fs::remove_file(&uno);
        let _ = std::fs::remove_file(&otro);
    }

    #[test]
    fn escribir_deja_el_archivo_y_crea_la_carpeta() {
        let base = std::env::temp_dir().join(format!("vasak-shot-anotada-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let destino = base.join("adentro").join("captura.png");

        escribir(&un_png(), &destino).expect("tendría que haber escrito");
        assert_eq!(std::fs::read(&destino).unwrap(), un_png());

        // Y lo que no es PNG no llega a tocar el disco.
        let malo = base.join("malo.png");
        assert!(escribir(b"cualquier cosa", &malo).is_err());
        assert!(!malo.exists());

        let _ = std::fs::remove_dir_all(&base);
    }
}
