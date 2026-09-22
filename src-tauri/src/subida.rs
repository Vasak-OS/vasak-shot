//! Subir la captura a algún lado y quedarse con el enlace.
//!
//! # Subir es publicar
//!
//! Una captura lleva encima lo que había en la pantalla, y eso es distinto de
//! copiarla al portapapeles o de dejarla en una carpeta: el enlace lo puede
//! abrir cualquiera que lo tenga, y quién lo guarda, por cuánto tiempo y quién
//! más lo mira ya no depende de esta máquina. De ahí salen las tres reglas que
//! este módulo hace cumplir:
//!
//! - **No hay destino por omisión.** Sin una dirección escrita a mano, el botón
//!   no existe. Un servicio por omisión convierte un botón mal apretado en una
//!   publicación.
//! - **Sólo `https`.** Subir por `http` es publicar en texto plano, y `curl`
//!   habla una docena de protocolos más —`file://` entre ellos— que con una
//!   dirección mal escrita harían cualquier otra cosa.
//! - **Sin metadatos.** Lo que el PNG lleve adentro viaja con él.
//!
//! Preguntar antes es cosa de la interfaz, que es la que puede mostrar a dónde
//! va.
//!
//! # Por qué `curl` y no un cliente de HTTP
//!
//! Esta aplicación no tiene ninguna pila de TLS: sumar una para un `POST` serían
//! cien dependencias nuevas y un segundo almacén de certificados que mantener al
//! día. `curl` ya sabe de certificados del sistema, de proxies y de redirecciones,
//! y es el mismo trato que el portapapeles —`wl-copy`— y el aviso —`notify-send`—.

use std::path::Path;
use std::process::{Command, Stdio};

/// El nombre del campo del formulario, cuando no se eligió otro.
///
/// El que usan casi todos los servicios de subida. Se puede cambiar porque
/// algunos piden `files[]`, y con el nombre equivocado la subida falla con un
/// error del servidor que no dice qué pasó.
pub const CAMPO_POR_OMISION: &str = "file";

/// Cuánto se espera a que termine la subida, en segundos.
const TECHO_DE_ESPERA: &str = "120";

/// Cuánto se acepta de respuesta. Lo que se espera es un enlace.
const TECHO_DE_RESPUESTA: &str = "64K";

/// Una dirección a la que se puede subir, ya comprobada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destino {
    pub url: String,
    pub campo: String,
}

impl Destino {
    /// El servidor, para poder decirlo antes de subir.
    ///
    /// Lo que hace falta mostrar es a dónde va, y eso es el anfitrión: la ruta
    /// completa es ruido en una pregunta que se lee en dos segundos.
    pub fn servidor(&self) -> &str {
        self.url
            .trim_start_matches("https://")
            .split(['/', '?', '#'])
            .next()
            .unwrap_or(&self.url)
    }
}

/// Comprueba una dirección escrita a mano.
///
/// Devuelve `None` si no hay ninguna configurada, que no es un error: es el
/// estado de siempre, y quiere decir que no se sube a ningún lado.
pub fn destino_de(url: Option<&str>, campo: Option<&str>) -> Result<Option<Destino>, String> {
    let Some(url) = url.map(str::trim).filter(|u| !u.is_empty()) else {
        return Ok(None);
    };

    // Sólo `https`, y comprobado acá y no con una comparación suelta en el
    // frontend: el que ejecuta `curl` es este lado.
    let Some(resto) = url.strip_prefix("https://") else {
        return Err(format!(
            "la dirección para subir tiene que empezar con «https://», y es «{url}»"
        ));
    };
    if resto.is_empty() || resto.starts_with('/') {
        return Err("la dirección para subir no tiene servidor".to_string());
    }
    if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("la dirección para subir tiene espacios o caracteres de control".to_string());
    }

    Ok(Some(Destino {
        url: url.to_string(),
        campo: campo_de(campo)?,
    }))
}

/// El nombre del campo, comprobado.
///
/// `curl` parte el argumento de `--form` por el primer `=`, y lo que viene
/// después de un `@` es un archivo: un nombre de campo con cualquiera de los dos
/// deja de nombrar un campo y pasa a decidir qué se manda.
pub fn campo_de(campo: Option<&str>) -> Result<String, String> {
    let Some(campo) = campo.map(str::trim).filter(|c| !c.is_empty()) else {
        return Ok(CAMPO_POR_OMISION.to_string());
    };
    if !campo
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "_-[]".contains(ch))
    {
        return Err(format!(
            "el campo del formulario sólo puede tener letras, números, «_», «-» y «[]», y es «{campo}»"
        ));
    }
    Ok(campo.to_string())
}

/// Los argumentos de `curl`, sin ejecutarlo.
///
/// Sueltos para poder probarlos: acá viven las tres cosas que impiden que una
/// dirección mal escrita haga algo que nadie pidió.
pub fn argumentos(destino: &Destino, archivo: &Path) -> Vec<String> {
    vec![
        "--silent".to_string(),
        "--show-error".to_string(),
        // Falla con los errores del servidor, pero **imprimiendo el cuerpo**:
        // es donde los servicios de subida dicen por qué no.
        "--fail-with-body".to_string(),
        // Aunque la dirección ya esté comprobada: `curl` habla `file://`,
        // `scp://` y una docena más, y esto lo deja en uno solo.
        "--proto".to_string(),
        "=https".to_string(),
        // **Sin seguir redirecciones.** La pregunta que se contestó decía a qué
        // servidor va, y `curl` reenvía un `POST` tal cual ante un 307 o un 308:
        // el destino de verdad podría terminar siendo otro anfitrión, y la
        // captura ya estaría allá cuando alguien se entere. Un servicio de
        // subida que conteste con una redirección no se usa desde acá.
        "--max-time".to_string(),
        TECHO_DE_ESPERA.to_string(),
        // La respuesta que se espera es un enlace. `output()` junta en memoria
        // todo lo que llegue, así que sin techo un servidor —o algo en el medio—
        // puede mandar un cuerpo sin fin durante los dos minutos de espera.
        "--max-filesize".to_string(),
        TECHO_DE_RESPUESTA.to_string(),
        "--form".to_string(),
        format!(
            "{}=@{};type=image/png",
            destino.campo,
            archivo.to_string_lossy()
        ),
        destino.url.clone(),
    ]
}

/// El enlace que contestó el servidor.
///
/// La respuesta se toma como el enlace y nada más: es lo que devuelven los
/// servicios de subida, y adivinar dentro de un JSON cuál de sus campos es el
/// enlace sería adivinar. Lo que no parece un enlace se informa tal cual, que es
/// la única forma de enterarse de qué contestó el servidor.
pub fn enlace_de(salida: &str) -> Result<String, String> {
    let enlace = salida.trim();
    if enlace.is_empty() {
        return Err("el servidor no contestó ningún enlace".to_string());
    }
    if !(enlace.starts_with("https://") || enlace.starts_with("http://")) {
        let primera = enlace.lines().next().unwrap_or_default();
        return Err(format!("el servidor no contestó un enlace: {primera}"));
    }
    if enlace.lines().count() > 1 {
        return Err(format!(
            "el servidor contestó más de una línea: {}",
            enlace.lines().next().unwrap_or_default()
        ));
    }
    Ok(enlace.to_string())
}

/// Reescribe el PNG dejando sólo los píxeles.
///
/// Decodificar y volver a codificar tira todo lo que no sea la imagen: el
/// codificador escribe la cabecera, los datos y el final, y nada más. Lo que el
/// original llevara —una fecha, un comentario, de qué programa salió— no viaja.
///
/// Un PNG de captura no suele llevar mucho, pero **lo que lleve se publica**, y
/// eso hay que decidirlo antes y no cuando ya está subido.
pub fn sin_metadatos(origen: &Path, destino: &Path) -> Result<(), String> {
    let imagen = image::open(origen)
        .map_err(|e| format!("no se pudo leer {}: {e}", origen.display()))?
        .into_rgba8();
    imagen
        .save(destino)
        .map_err(|e| format!("no se pudo escribir {}: {e}", destino.display()))
}

/// Sube el archivo y devuelve el enlace.
///
/// El archivo que se manda es una copia sin metadatos, no el original.
pub fn subir(archivo: &Path, destino: &Destino) -> Result<String, String> {
    let limpio = crate::anotada::temporal("subida")?;
    let resultado = subir_limpio(archivo, &limpio, destino);
    let _ = std::fs::remove_file(&limpio);
    resultado
}

/// Lo de arriba, con el temporal ya creado, para poder borrarlo pase lo que pase.
fn subir_limpio(archivo: &Path, limpio: &Path, destino: &Destino) -> Result<String, String> {
    sin_metadatos(archivo, limpio)?;

    let salida = Command::new("curl")
        .args(argumentos(destino, limpio))
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("no se pudo ejecutar curl: {e}"))?;

    let cuerpo = String::from_utf8_lossy(&salida.stdout).into_owned();
    if !salida.status.success() {
        let error = String::from_utf8_lossy(&salida.stderr);
        let detalle = [error.trim(), cuerpo.trim()]
            .into_iter()
            .find(|t| !t.is_empty())
            .unwrap_or("sin detalle");
        return Err(format!(
            "{} no aceptó la captura: {detalle}",
            destino.servidor()
        ));
    }

    enlace_de(&cuerpo)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::path::PathBuf;

    fn destino() -> Destino {
        destino_de(Some("https://ejemplo.invalido/subir"), None)
            .unwrap()
            .unwrap()
    }

    #[test]
    fn sin_direccion_no_se_sube_a_ningun_lado() {
        // Es el estado de siempre, y no es un error: un servicio por omisión
        // convierte un botón mal apretado en una publicación.
        assert_eq!(destino_de(None, None), Ok(None));
        assert_eq!(destino_de(Some(""), None), Ok(None));
        assert_eq!(destino_de(Some("   "), None), Ok(None));
    }

    #[test]
    fn una_direccion_que_no_es_https_no_se_acepta() {
        // `http` publica en texto plano, y `curl` habla una docena de protocolos
        // más que con una dirección mal escrita harían cualquier otra cosa.
        for mala in [
            "http://ejemplo.invalido/subir",
            "file:///etc/passwd",
            "scp://servidor/cosa",
            "ejemplo.invalido/subir",
            "https://",
            "https:///sin-servidor",
        ] {
            assert!(destino_de(Some(mala), None).is_err(), "con «{mala}»");
        }
    }

    #[test]
    fn una_direccion_con_espacios_no_se_acepta() {
        assert!(destino_de(Some("https://ejemplo.invalido/a b"), None).is_err());
        assert!(destino_de(Some("https://ejemplo.invalido/a\nb"), None).is_err());
    }

    #[test]
    fn el_campo_no_puede_cambiar_lo_que_se_manda() {
        // `curl` parte `--form` por el primer `=`, y lo que sigue a un `@` es un
        // archivo: un campo con cualquiera de los dos deja de nombrar un campo.
        for malo in ["archivo=@/etc/passwd", "a@b", "a b", "a;type=text/plain"] {
            assert!(
                destino_de(Some("https://ejemplo.invalido/s"), Some(malo)).is_err(),
                "con «{malo}»"
            );
        }
        // Y los que los servicios de verdad piden, sí.
        for bueno in ["file", "files[]", "imagen_1", "up-load"] {
            assert!(
                destino_de(Some("https://ejemplo.invalido/s"), Some(bueno)).is_ok(),
                "con «{bueno}»"
            );
        }
    }

    #[test]
    fn el_servidor_se_puede_decir_antes_de_subir() {
        // Es lo que la pregunta tiene que mostrar: a dónde va.
        assert_eq!(destino().servidor(), "ejemplo.invalido");
        let con_puerto = destino_de(Some("https://ejemplo.invalido:8443/a/b?c=d"), None)
            .unwrap()
            .unwrap();
        assert_eq!(con_puerto.servidor(), "ejemplo.invalido:8443");
    }

    #[test]
    fn los_argumentos_de_curl_no_dejan_salir_de_https() {
        let args = argumentos(&destino(), &PathBuf::from("/tmp/x.png"));
        let juntos = args.join(" ");

        assert!(juntos.contains("--proto =https"), "{juntos}");
        assert!(juntos.contains("--fail-with-body"), "{juntos}");
        // Sin seguir redirecciones: la pregunta que se contestó decía a qué
        // servidor va, y un 307 mandaría el `POST` entero a otro.
        assert!(!juntos.contains("--location"), "{juntos}");
        // Y con techo para la respuesta, que se junta entera en memoria.
        assert!(juntos.contains("--max-filesize 64K"), "{juntos}");
        assert!(
            juntos.contains("file=@/tmp/x.png;type=image/png"),
            "{juntos}"
        );
        // La dirección va última, y entera.
        assert_eq!(args.last().unwrap(), "https://ejemplo.invalido/subir");
    }

    #[test]
    fn la_respuesta_del_servidor_se_lee_como_enlace() {
        assert_eq!(
            enlace_de("https://ejemplo.invalido/abc.png\n"),
            Ok("https://ejemplo.invalido/abc.png".to_string())
        );
    }

    #[test]
    fn lo_que_no_es_un_enlace_se_informa_y_no_se_copia() {
        // Copiar al portapapeles la página de error de un servidor sería peor
        // que fallar: parece que funcionó.
        assert!(enlace_de("").is_err());
        assert!(enlace_de("   \n ").is_err());
        let error = enlace_de("<html><body>413 Payload Too Large</body></html>").unwrap_err();
        assert!(error.contains("413"), "{error}");
        // Varias líneas tampoco: un enlace es uno.
        assert!(enlace_de("https://a.invalido/x\nhttps://a.invalido/y").is_err());
    }

    #[test]
    fn reescribir_el_png_le_saca_lo_que_no_son_pixeles() {
        // Lo que el archivo lleve adentro viaja con él, y una vez subido ya no
        // hay nada que decidir.
        let dir = std::env::temp_dir().join(format!("vsk-shot-meta-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let con_texto = dir.join("con-texto.png");
        let limpio = dir.join("limpio.png");

        // Un PNG de verdad, y después un trozo `tEXt` metido a mano entre la
        // cabecera y los datos. Es como se ve un comentario adentro de un PNG.
        let original = image::RgbaImage::from_fn(4, 4, |x, y| {
            image::Rgba([x as u8 * 60, y as u8 * 60, 7, 255])
        });
        original.save(&con_texto).unwrap();

        let bytes = std::fs::read(&con_texto).unwrap();
        let corte = 8 + 25; // la firma, y después el trozo IHDR entero
        let mut trozo = Vec::new();
        let datos = b"Comment\0lo que habia en la pantalla";
        trozo.extend_from_slice(&(datos.len() as u32).to_be_bytes());
        trozo.extend_from_slice(b"tEXt");
        trozo.extend_from_slice(datos);
        let mut crc = crc32(b"tEXt");
        crc = crc32_con(crc, datos);
        trozo.extend_from_slice(&crc.to_be_bytes());

        let mut con_chunk = Vec::new();
        con_chunk.extend_from_slice(&bytes[..corte]);
        con_chunk.extend_from_slice(&trozo);
        con_chunk.extend_from_slice(&bytes[corte..]);
        std::fs::write(&con_texto, &con_chunk).unwrap();

        // El trozo está, y se lee: si esto fallara, la prueba de abajo no
        // probaría nada.
        assert!(
            contiene(&con_chunk, b"lo que habia en la pantalla"),
            "el PNG de prueba tendría que llevar el comentario"
        );
        assert_eq!(
            image::open(&con_texto).unwrap().into_rgba8(),
            original,
            "y tendría que seguir siendo la misma imagen"
        );

        sin_metadatos(&con_texto, &limpio).unwrap();

        let salida = std::fs::read(&limpio).unwrap();
        assert!(
            !contiene(&salida, b"lo que habia en la pantalla"),
            "el comentario viajó con la captura"
        );
        assert!(!contiene(&salida, b"tEXt"), "quedó el trozo de texto");
        // Y los píxeles son los mismos: limpiar no puede cambiar la captura.
        assert_eq!(image::open(&limpio).unwrap().into_rgba8(), original);

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn contiene(donde: &[u8], que: &[u8]) -> bool {
        donde.windows(que.len()).any(|v| v == que)
    }

    /// El CRC de los PNG, que es el de siempre. Veinte líneas contra una
    /// dependencia nueva para armar un archivo de prueba.
    fn crc32(datos: &[u8]) -> u32 {
        // Cero es el valor ya «finalizado» de un CRC vacío, así que encadenarlo
        // con `crc32_con` arranca el registro en todo unos, que es donde empieza.
        crc32_con(0, datos)
    }

    fn crc32_con(anterior: u32, datos: &[u8]) -> u32 {
        let mut c = anterior ^ 0xFFFF_FFFF;
        for &b in datos {
            c ^= u32::from(b);
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
        }
        c ^ 0xFFFF_FFFF
    }
}
