//! Dónde va a parar una captura.
//!
//! En `~/Imágenes/Capturas`, o el equivalente en el idioma de la instalación: el
//! nombre de la carpeta de imágenes lo elige `user-dirs.dirs` y no es «Pictures»
//! en todas las máquinas. Guardar en una ruta en inglés cuando el resto del
//! sistema está en español deja las capturas en una carpeta que la persona no
//! reconoce.

use std::path::PathBuf;

/// El nombre de la subcarpeta, dentro de la de imágenes.
const SUBCARPETA: &str = "Capturas";

/// La carpeta de imágenes según `user-dirs.dirs`, dado su contenido.
///
/// Separado de la lectura del archivo para poder probarlo: el formato admite
/// comentarios, comillas y `$HOME`, y una carpeta mal resuelta manda las capturas
/// a un lugar que nadie va a buscar.
pub fn imagenes_en_user_dirs(contenido: &str, home: &str) -> Option<PathBuf> {
    for linea in contenido.lines() {
        let linea = linea.trim();
        if linea.starts_with('#') {
            continue;
        }
        let Some(valor) = linea.strip_prefix("XDG_PICTURES_DIR=") else {
            continue;
        };
        let valor = valor.trim().trim_matches('"');
        if valor.is_empty() {
            continue;
        }
        return Some(PathBuf::from(valor.replace("$HOME", home)));
    }
    None
}

/// La carpeta donde se guardan las capturas, creándola si hace falta.
pub fn carpeta() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "no hay HOME".to_string())?;

    let config = PathBuf::from(&home).join(".config").join("user-dirs.dirs");
    let imagenes = std::fs::read_to_string(&config)
        .ok()
        .and_then(|c| imagenes_en_user_dirs(&c, &home))
        // Sin configuración, `Pictures` es lo que la especificación de XDG dice
        // por omisión.
        .unwrap_or_else(|| PathBuf::from(&home).join("Pictures"));

    let destino = imagenes.join(SUBCARPETA);
    std::fs::create_dir_all(&destino)
        .map_err(|e| format!("no se pudo crear {}: {e}", destino.display()))?;
    Ok(destino)
}

/// El nombre de archivo para ahora mismo.
///
/// La hora se saca de `SystemTime` y se convierte a mano en lugar de sumar una
/// dependencia de fechas para formatear seis números. Es hora local: una captura
/// llamada con la hora UTC no coincide con la que muestra el reloj del panel, y
/// la persona la busca por esa.
pub fn nombre_de_ahora() -> String {
    let ahora = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let (anio, mes, dia, hora, minuto, segundo) = civil_desde_epoch(ahora + desplazamiento_local());
    crate::captura::nombre_de_archivo(anio, mes, dia, hora, minuto, segundo)
}

/// Segundos que hay que sumarle a UTC para llegar a la hora local.
///
/// Se lo pregunta a `date`, que ya conoce la zona y el horario de verano. Hacerlo
/// a mano significaría leer la base de datos de zonas horarias, que es mucho para
/// nombrar un archivo.
fn desplazamiento_local() -> i64 {
    std::process::Command::new("date")
        .arg("+%z")
        .output()
        .ok()
        .and_then(|s| String::from_utf8(s.stdout).ok())
        .and_then(|t| desplazamiento_desde_z(t.trim()))
        .unwrap_or(0)
}

/// Convierte un `+HHMM` como el de `date +%z` en segundos.
pub fn desplazamiento_desde_z(texto: &str) -> Option<i64> {
    if texto.len() != 5 {
        return None;
    }
    let signo = match texto.as_bytes()[0] {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let horas: i64 = texto[1..3].parse().ok()?;
    let minutos: i64 = texto[3..5].parse().ok()?;
    Some(signo * (horas * 3600 + minutos * 60))
}

/// De segundos desde la época al calendario, en el algoritmo de Howard Hinnant.
///
/// Vale la pena tenerlo acá y no traer una dependencia: son veinte líneas y no
/// cambian nunca. Y con test, porque un error de un día en el nombre del archivo
/// no se nota hasta que alguien busca la captura de ayer.
pub fn civil_desde_epoch(segundos: i64) -> (i32, u32, u32, u32, u32, u32) {
    let dias = segundos.div_euclid(86_400);
    let resto = segundos.rem_euclid(86_400);

    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let anio = if m <= 2 { y + 1 } else { y };

    (
        anio as i32,
        m as u32,
        d as u32,
        (resto / 3600) as u32,
        ((resto % 3600) / 60) as u32,
        (resto % 60) as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_carpeta_de_imagenes_sale_de_user_dirs() {
        // En una instalación en español no es «Pictures», y guardar ahí deja las
        // capturas donde nadie las busca.
        assert_eq!(
            imagenes_en_user_dirs("XDG_PICTURES_DIR=\"$HOME/Imágenes\"", "/home/pato"),
            Some(PathBuf::from("/home/pato/Imágenes"))
        );
        assert_eq!(
            imagenes_en_user_dirs("XDG_PICTURES_DIR=\"/mnt/fotos\"", "/home/pato"),
            Some(PathBuf::from("/mnt/fotos"))
        );
    }

    #[test]
    fn los_comentarios_de_user_dirs_no_cuentan() {
        let contenido = "# XDG_PICTURES_DIR=\"$HOME/Viejo\"\nXDG_PICTURES_DIR=\"$HOME/Imágenes\"\n";
        assert_eq!(
            imagenes_en_user_dirs(contenido, "/home/pato"),
            Some(PathBuf::from("/home/pato/Imágenes"))
        );
    }

    #[test]
    fn sin_la_clave_no_se_inventa_una_carpeta() {
        assert_eq!(imagenes_en_user_dirs("XDG_MUSIC_DIR=\"$HOME/Musica\"", "/home/pato"), None);
        assert_eq!(imagenes_en_user_dirs("XDG_PICTURES_DIR=\"\"", "/home/pato"), None);
    }

    #[test]
    fn el_desplazamiento_de_zona_se_lee_bien() {
        assert_eq!(desplazamiento_desde_z("+0000"), Some(0));
        assert_eq!(desplazamiento_desde_z("-0300"), Some(-10_800));
        assert_eq!(desplazamiento_desde_z("+0530"), Some(19_800));
        // Y lo que no tiene esa forma no se interpreta como cero por las dudas:
        // un desplazamiento inventado corre la hora del nombre.
        assert_eq!(desplazamiento_desde_z(""), None);
        assert_eq!(desplazamiento_desde_z("0300"), None);
        assert_eq!(desplazamiento_desde_z("+03:00"), None);
    }

    #[test]
    fn la_fecha_se_calcula_bien() {
        // La época.
        assert_eq!(civil_desde_epoch(0), (1970, 1, 1, 0, 0, 0));
        // Un día bisiesto, que es donde este cálculo se rompe si está mal.
        assert_eq!(civil_desde_epoch(1_709_164_800), (2024, 2, 29, 0, 0, 0));
        // Y el fin de un siglo que no es bisiesto.
        assert_eq!(civil_desde_epoch(4_107_542_400), (2100, 3, 1, 0, 0, 0));
        // Con hora.
        assert_eq!(civil_desde_epoch(1_724_680_245), (2024, 8, 26, 13, 50, 45));
    }

    #[test]
    fn una_fecha_antes_de_la_epoca_no_rompe() {
        // `SystemTime` puede quedar antes de 1970 con una pila RTC muerta, y un
        // panic acá dejaría la captura tomada pero sin guardar.
        let (anio, _, _, _, _, _) = civil_desde_epoch(-86_400);
        assert_eq!(anio, 1969);
    }
}
