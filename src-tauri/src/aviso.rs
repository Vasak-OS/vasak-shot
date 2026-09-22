//! El aviso de que la captura quedó guardada, con qué hacer con ella.
//!
//! Sin esto el aviso es un callejón sin salida: dice el nombre del archivo,
//! muestra la miniatura, y para hacer algo con la captura hay que abrir el
//! gestor de archivos y navegar hasta la carpeta.
//!
//! # Por qué lo muestra otro proceso
//!
//! `notify-send --action` declara los botones **e implica `--wait`**: se queda
//! esperando a que la persona elija, y recién ahí imprime cuál eligió. Alguien
//! tiene que seguir vivo para escuchar esa respuesta, y no puede ser la ventana
//! del selector — que se cierra sola en cuanto la captura está entregada, que es
//! lo que hace que el gesto dure lo que dura.
//!
//! Así que el aviso lo muestra un hijo suelto: esta misma aplicación con
//! `--aviso`, lanzada sin esperarla y con sus descriptores cerrados. El selector
//! se va, el hijo se queda con la notificación, y cuando llega la respuesta la
//! ejecuta. Es la misma razón por la que el aviso nunca lo emitió la ventana:
//! una notificación emitida por un proceso que está terminando puede irse con
//! él.

use std::path::Path;
use std::process::{Command, Stdio};

/// Los textos del aviso, que salen del catálogo de idioma del frontend.
///
/// Vienen de allá y no se leen acá porque el catálogo lo sirve un plugin de
/// Tauri, y esto corre —a propósito— en un proceso donde Tauri no existe.
/// Parsear los `.yml` de nuevo de este lado sería una segunda implementación de
/// lo que el plugin ya hace, y con una biblioteca de YAML sin mantenimiento.
///
/// Por omisión quedan en español, que es el idioma de reserva de la aplicación:
/// es lo que ve el camino de la línea de órdenes, donde no hay ninguna ventana
/// que los haya traducido.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Textos {
    pub guardada: String,
    pub abrir: String,
    pub carpeta: String,
    pub copiar: String,
}

impl Default for Textos {
    fn default() -> Self {
        Self {
            guardada: "Captura guardada".to_string(),
            abrir: "Abrir".to_string(),
            carpeta: "Abrir la carpeta".to_string(),
            copiar: "Copiar".to_string(),
        }
    }
}

/// Qué se eligió en el aviso.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accion {
    Abrir,
    Carpeta,
    Copiar,
}

impl Accion {
    /// El identificador que viaja por el protocolo de notificaciones.
    ///
    /// Corto y en minúsculas: no se muestra, lo imprime `notify-send` para
    /// decir cuál se apretó.
    fn clave(self) -> &'static str {
        match self {
            Accion::Abrir => "abrir",
            Accion::Carpeta => "carpeta",
            Accion::Copiar => "copiar",
        }
    }
}

/// Qué acción nombra la respuesta de `notify-send`.
///
/// `None` no es un error: es lo que devuelve una notificación que se cerró sola
/// —caducó, o alguien la descartó— y ahí no hay que hacer nada. Un nombre
/// desconocido va por el mismo camino, porque abrir algo «por las dudas» es
/// abrir algo que nadie pidió.
pub fn accion_de(salida: &str) -> Option<Accion> {
    match salida.trim() {
        "abrir" => Some(Accion::Abrir),
        "carpeta" => Some(Accion::Carpeta),
        "copiar" => Some(Accion::Copiar),
        _ => None,
    }
}

/// Los argumentos de `notify-send`, sin ejecutarlo.
///
/// Sueltos para poder probarlos: lo que decide si el aviso lleva botones, y
/// cuáles, es esta lista — y un error acá no falla, sólo deja el aviso como
/// estaba.
///
/// `copiar` sólo aparece cuando la captura **no** se copió ya. Ofrecer copiar
/// algo que acaba de quedar en el portapapeles es un botón que no hace nada.
pub fn argumentos(ruta: &Path, copiada: bool, textos: &Textos) -> Vec<String> {
    let mut args = vec![
        "--app-name=vasak-shot".to_string(),
        "--icon".to_string(),
        ruta.to_string_lossy().into_owned(),
    ];

    for accion in [Accion::Abrir, Accion::Carpeta, Accion::Copiar] {
        if accion == Accion::Copiar && copiada {
            continue;
        }
        let texto = match accion {
            Accion::Abrir => &textos.abrir,
            Accion::Carpeta => &textos.carpeta,
            Accion::Copiar => &textos.copiar,
        };
        args.push(format!("--action={}={}", accion.clave(), texto));
    }

    args.push(textos.guardada.clone());
    args.push(
        ruta.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
    );
    args
}

/// Lanza el proceso que va a mostrar el aviso, y no lo espera.
///
/// Si falla no se dice nada más: la captura ya está guardada y el aviso es un
/// lujo, no el resultado.
pub fn lanzar(ruta: &Path, copiada: bool, textos: &Textos) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };

    let mut orden = Command::new(exe);
    orden.arg("--aviso").arg(ruta);
    if copiada {
        orden.arg("--ya-copiada");
    }
    // Los textos viajan como argumentos porque el hijo no tiene de dónde
    // sacarlos: es un proceso sin ventana y sin plugin de idioma.
    orden
        .arg("--textos")
        .arg(format!(
            "{}\n{}\n{}\n{}",
            textos.guardada, textos.abrir, textos.carpeta, textos.copiar
        ))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let _ = orden.spawn();
}

/// Los cuatro textos como los pasa `lanzar`, de vuelta a su estructura.
///
/// Una línea por texto y en orden fijo. Con menos líneas de las que hace falta
/// quedan los de reserva: es preferible un aviso en español a uno con botones
/// vacíos.
pub fn textos_de(crudo: &str) -> Textos {
    let mut lineas = crudo.split('\n');
    let por_omision = Textos::default();
    let mut siguiente = |reserva: String| {
        lineas
            .next()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map_or(reserva, str::to_string)
    };

    Textos {
        guardada: siguiente(por_omision.guardada),
        abrir: siguiente(por_omision.abrir),
        carpeta: siguiente(por_omision.carpeta),
        copiar: siguiente(por_omision.copiar),
    }
}

/// Muestra el aviso, espera la respuesta y hace lo que se haya elegido.
///
/// Bloquea: es todo lo que este proceso tiene para hacer.
pub fn mostrar(ruta: &Path, copiada: bool, textos: &Textos) {
    let salida = Command::new("notify-send")
        .args(argumentos(ruta, copiada, textos))
        .stderr(Stdio::null())
        .output();

    let Ok(salida) = salida else {
        return;
    };
    let Ok(elegida) = String::from_utf8(salida.stdout) else {
        return;
    };
    let Some(accion) = accion_de(&elegida) else {
        return;
    };

    ejecutar(accion, ruta);
}

/// Hace lo elegido.
///
/// Abrir va por `xdg-open`, que respeta lo que la persona eligió como visor de
/// imágenes y como gestor de archivos. Elegir uno acá sería imponer el nuestro.
fn ejecutar(accion: Accion, ruta: &Path) {
    match accion {
        Accion::Abrir => abrir(ruta),
        Accion::Carpeta => {
            if let Some(carpeta) = ruta.parent() {
                abrir(carpeta);
            }
        }
        Accion::Copiar => {
            if let Err(e) = crate::captura::copiar_al_portapapeles(ruta) {
                eprintln!("vasak-shot: no se pudo copiar: {e}");
            }
        }
    }
}

/// Abre una ruta con lo que el escritorio tenga puesto.
fn abrir(ruta: &Path) {
    // Esperar a que termine, y no soltarlo: `xdg-open` puede delegar en un
    // proceso que vive lo que dure la ventana que abre, y este proceso no tiene
    // nada más que hacer. Soltarlo y salir mataría el grupo en algunos
    // escritorios.
    let _ = Command::new("xdg-open")
        .arg(ruta)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::path::PathBuf;

    fn ruta() -> PathBuf {
        PathBuf::from("/home/pato/Imágenes/ScreenShots/Captura 2026-09-22 10.11.12.png")
    }

    #[test]
    fn el_aviso_lleva_las_tres_acciones() {
        let args = argumentos(&ruta(), false, &Textos::default());
        let juntos = args.join(" ");

        assert!(juntos.contains("--action=abrir=Abrir"), "{juntos}");
        assert!(
            juntos.contains("--action=carpeta=Abrir la carpeta"),
            "{juntos}"
        );
        assert!(juntos.contains("--action=copiar=Copiar"), "{juntos}");
    }

    #[test]
    fn copiar_no_aparece_cuando_ya_se_copio() {
        // Sería un botón que no hace nada: la captura ya está en el
        // portapapeles.
        let args = argumentos(&ruta(), true, &Textos::default());
        let juntos = args.join(" ");

        assert!(!juntos.contains("--action=copiar"), "{juntos}");
        assert!(juntos.contains("--action=abrir"), "{juntos}");
    }

    #[test]
    fn el_aviso_lleva_la_miniatura_y_el_nombre() {
        // Es lo que ya hacía, y no puede perderse al sumarle los botones: el
        // aviso sin miniatura no deja reconocer cuál de dos capturas es.
        let args = argumentos(&ruta(), false, &Textos::default());

        assert!(args.contains(&"--icon".to_string()));
        assert!(args.contains(&ruta().to_string_lossy().into_owned()));
        assert_eq!(args.last().unwrap(), "Captura 2026-09-22 10.11.12.png");
    }

    #[test]
    fn los_textos_traducidos_van_a_los_botones() {
        // El catálogo lo tiene el frontend; acá llegan ya traducidos.
        let textos = Textos {
            guardada: "Screenshot saved".to_string(),
            abrir: "Open".to_string(),
            carpeta: "Open the folder".to_string(),
            copiar: "Copy".to_string(),
        };
        let juntos = argumentos(&ruta(), false, &textos).join(" ");

        assert!(juntos.contains("--action=abrir=Open"), "{juntos}");
        assert!(juntos.contains("Screenshot saved"), "{juntos}");
    }

    #[test]
    fn la_respuesta_se_convierte_en_la_accion() {
        // `notify-send` imprime el nombre con un salto de línea al final.
        assert_eq!(accion_de("abrir\n"), Some(Accion::Abrir));
        assert_eq!(accion_de("carpeta\n"), Some(Accion::Carpeta));
        assert_eq!(accion_de("copiar"), Some(Accion::Copiar));
    }

    #[test]
    fn una_notificacion_que_se_cerro_sola_no_hace_nada() {
        // Es el caso normal: nadie aprieta nada y la notificación caduca.
        // Abrir algo acá sería abrir una ventana que nadie pidió, encima un rato
        // después de haber capturado.
        assert_eq!(accion_de(""), None);
        assert_eq!(accion_de("\n"), None);
        assert_eq!(accion_de("cualquier-cosa"), None);
    }

    #[test]
    fn los_textos_viajan_y_vuelven() {
        let textos = Textos {
            guardada: "Screenshot saved".to_string(),
            abrir: "Open".to_string(),
            carpeta: "Open the folder".to_string(),
            copiar: "Copy".to_string(),
        };
        let crudo = format!(
            "{}\n{}\n{}\n{}",
            textos.guardada, textos.abrir, textos.carpeta, textos.copiar
        );

        assert_eq!(textos_de(&crudo), textos);
    }

    #[test]
    fn unos_textos_a_medias_quedan_en_los_de_reserva() {
        // Un aviso en español es mejor que uno con botones vacíos.
        let a_medias = textos_de("Guardada\n\nAbrir la carpeta");
        assert_eq!(a_medias.guardada, "Guardada");
        assert_eq!(a_medias.abrir, "Abrir");
        assert_eq!(a_medias.carpeta, "Abrir la carpeta");
        assert_eq!(a_medias.copiar, "Copiar");
        assert_eq!(textos_de(""), Textos::default());
    }
}
