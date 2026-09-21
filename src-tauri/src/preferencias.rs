//! Qué hace soltar el botón, y dónde va la captura.
//!
//! Un archivo propio y no `vasak.conf`: el esquema de `config-manager` es cerrado
//! —estilo, fuentes, iconos y escritorio— y lo que no conoce lo descarta. Las
//! preferencias de una aplicación no van ahí.
//!
//! **Nada de acá es un error que se propague.** Un archivo que no existe, que no
//! se puede leer o que tiene basura adentro da los valores por omisión y la
//! captura sigue su camino. Perder una captura por un JSON roto sería cambiar un
//! problema chico por uno grande.

use std::path::{Path, PathBuf};

/// Qué se entrega al soltar el botón del ratón.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AlSoltar {
    /// Lo que la herramienta hizo siempre, y lo que se quiere casi siempre.
    #[default]
    GuardarYCopiar,
    Guardar,
    Copiar,
    /// Congelar la selección y dejar decidir con los botones.
    ///
    /// Es el valor que van a necesitar anotar y ajustar la selección: las dos
    /// cosas pasan **después** de soltar, y no existen si soltar ya entregó.
    Esperar,
}

impl AlSoltar {
    /// El valor que nombra ese texto, o `None` si no nombra ninguno.
    pub fn desde_texto(texto: &str) -> Option<Self> {
        match texto {
            "guardar-y-copiar" => Some(Self::GuardarYCopiar),
            "guardar" => Some(Self::Guardar),
            "copiar" => Some(Self::Copiar),
            "esperar" => Some(Self::Esperar),
            _ => None,
        }
    }
}

/// Las preferencias, como se guardan.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preferencias {
    pub al_soltar: AlSoltar,
    /// La carpeta elegida a mano, o `None` para la de por omisión.
    ///
    /// Nula y no la ruta ya resuelta: guardar el resultado congelaría el `$HOME`
    /// y el idioma que había el día que se guardó, y los dos cambian.
    pub carpeta: Option<PathBuf>,
}

/// El archivo donde viven, o `None` si no hay dónde.
///
/// Por `dirs` y no a mano: la regla de que una ruta XDG relativa se ignora ya
/// está escrita ahí, y es dependencia directa en media docena de repositorios del
/// taller. Escribirla por cuarta vez es garantizar que las cuatro se separen.
pub fn ruta() -> Option<PathBuf> {
    dirs::config_dir().map(|c| c.join("vasak-shot").join("preferencias.json"))
}

/// Las preferencias guardadas, o las de por omisión.
pub fn leer() -> Preferencias {
    ruta()
        .and_then(|r| std::fs::read_to_string(r).ok())
        .map(|c| desde_json(&c))
        .unwrap_or_default()
}

/// Lee un JSON de preferencias **campo por campo**.
///
/// Campo por campo y no con `serde` derivado a secas: así una clave rota se lleva
/// puesta sólo su propio valor. Con la derivación, un `alSoltar` que ya no existe
/// —porque el archivo lo escribió una versión más nueva— tiraría abajo el archivo
/// entero y se perdería también la carpeta, que estaba bien.
pub fn desde_json(texto: &str) -> Preferencias {
    let Ok(valor) = serde_json::from_str::<serde_json::Value>(texto) else {
        return Preferencias::default();
    };

    Preferencias {
        al_soltar: valor
            .get("alSoltar")
            .and_then(|v| v.as_str())
            .and_then(AlSoltar::desde_texto)
            .unwrap_or_default(),
        carpeta: valor
            .get("carpeta")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .map(PathBuf::from),
    }
}

/// Guarda las preferencias, creando la carpeta de configuración si hace falta.
///
/// Por archivo temporal y `rename`: una escritura cortada a la mitad —la sesión
/// que se cierra en el momento justo— dejaría un JSON truncado, y lo que hay que
/// evitar no es perder las preferencias sino tener que recuperarlas a mano.
pub fn escribir(preferencias: &Preferencias) -> Result<(), String> {
    let ruta = ruta().ok_or_else(|| "no hay carpeta de configuración".to_string())?;
    let padre = ruta
        .parent()
        .ok_or_else(|| "la ruta de configuración no tiene carpeta".to_string())?;
    std::fs::create_dir_all(padre)
        .map_err(|e| format!("no se pudo crear {}: {e}", padre.display()))?;

    let texto = serde_json::to_string_pretty(preferencias)
        .map_err(|e| format!("no se pudieron serializar las preferencias: {e}"))?;

    let temporal = ruta.with_extension("json.nuevo");
    std::fs::write(&temporal, texto)
        .map_err(|e| format!("no se pudo escribir {}: {e}", temporal.display()))?;
    std::fs::rename(&temporal, &ruta)
        .map_err(|e| format!("no se pudo guardar {}: {e}", ruta.display()))
}

/// Interpreta lo que alguien escribió en el campo de la carpeta.
///
/// Vacío significa «la de por omisión», que es distinto de una ruta inválida: hay
/// que poder volver atrás borrando el campo.
///
/// Relativa se rechaza en lugar de resolverse contra el directorio de trabajo. El
/// de esta aplicación es el de quien la lanzó —un atajo del compositor, una
/// terminal cualquiera—, así que `capturas` significaría un lugar distinto cada
/// vez y ninguno de ellos el que se quiso escribir.
pub fn carpeta_escrita(bruta: &str, home: &str) -> Result<Option<PathBuf>, String> {
    let bruta = bruta.trim();
    if bruta.is_empty() {
        return Ok(None);
    }

    let expandida = if bruta == "~" {
        PathBuf::from(home)
    } else if let Some(resto) = bruta.strip_prefix("~/") {
        Path::new(home).join(resto)
    } else {
        PathBuf::from(bruta)
    };

    if !expandida.is_absolute() {
        return Err(format!("{} no es una ruta absoluta", expandida.display()));
    }
    Ok(Some(expandida))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_archivo_roto_da_los_valores_por_omision() {
        // Lo importante no es qué devuelve sino que devuelva: una captura no se
        // pierde porque el JSON esté cortado.
        for texto in ["", "{", "no es json", "[]", "null", "42"] {
            assert_eq!(desde_json(texto), Preferencias::default());
        }
    }

    #[test]
    fn se_leen_las_cuatro_acciones() {
        for (texto, esperada) in [
            ("guardar-y-copiar", AlSoltar::GuardarYCopiar),
            ("guardar", AlSoltar::Guardar),
            ("copiar", AlSoltar::Copiar),
            ("esperar", AlSoltar::Esperar),
        ] {
            let json = format!(r#"{{"alSoltar":"{texto}"}}"#);
            assert_eq!(desde_json(&json).al_soltar, esperada);
        }
    }

    #[test]
    fn una_accion_inventada_no_rompe_el_resto() {
        // El caso de un archivo escrito por una versión más nueva: la acción que
        // acá no existe se ignora, y la carpeta —que se entiende igual— se
        // conserva.
        let leidas = desde_json(r#"{"alSoltar":"mandar-por-paloma","carpeta":"/mnt/fotos"}"#);
        assert_eq!(leidas.al_soltar, AlSoltar::GuardarYCopiar);
        assert_eq!(leidas.carpeta, Some(PathBuf::from("/mnt/fotos")));
    }

    #[test]
    fn una_accion_que_no_es_texto_tampoco() {
        assert_eq!(
            desde_json(r#"{"alSoltar":7,"carpeta":"/mnt/fotos"}"#).carpeta,
            Some(PathBuf::from("/mnt/fotos"))
        );
    }

    #[test]
    fn una_carpeta_vacia_es_la_de_por_omision() {
        // Que la clave esté con la cadena vacía no puede significar «guardá en la
        // raíz» ni «no guardes»: significa que nadie eligió nada.
        assert_eq!(desde_json(r#"{"carpeta":""}"#).carpeta, None);
        assert_eq!(desde_json(r#"{"carpeta":"   "}"#).carpeta, None);
        assert_eq!(desde_json(r#"{"carpeta":null}"#).carpeta, None);
    }

    #[test]
    fn lo_que_se_escribe_se_vuelve_a_leer() {
        let originales = Preferencias {
            al_soltar: AlSoltar::Copiar,
            carpeta: Some(PathBuf::from("/mnt/fotos")),
        };
        let texto = serde_json::to_string(&originales).unwrap();
        assert_eq!(desde_json(&texto), originales);
    }

    #[test]
    fn el_campo_de_la_carpeta_se_interpreta() {
        assert_eq!(carpeta_escrita("", "/home/pato"), Ok(None));
        assert_eq!(carpeta_escrita("   ", "/home/pato"), Ok(None));
        assert_eq!(
            carpeta_escrita("~/Fotos", "/home/pato"),
            Ok(Some(PathBuf::from("/home/pato/Fotos")))
        );
        assert_eq!(
            carpeta_escrita("~", "/home/pato"),
            Ok(Some(PathBuf::from("/home/pato")))
        );
        assert_eq!(
            carpeta_escrita("  /mnt/fotos  ", "/home/pato"),
            Ok(Some(PathBuf::from("/mnt/fotos")))
        );
    }

    #[test]
    fn una_ruta_relativa_se_rechaza() {
        // Resolverla contra el directorio de trabajo daría un lugar distinto
        // según desde dónde se lanzó la herramienta.
        assert!(carpeta_escrita("capturas", "/home/pato").is_err());
        assert!(carpeta_escrita("../otra", "/home/pato").is_err());
        // Y `~otro` no es el `~` de nadie: es un nombre relativo que empieza con
        // esa letra.
        assert!(carpeta_escrita("~otro/Fotos", "/home/pato").is_err());
    }
}
