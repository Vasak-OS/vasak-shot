//! Punto de entrada de vasak-shot.
//!
//! # El orden importa, y es al revés de lo que parece
//!
//! Primero los píxeles, después la ventana. Crear una ventana de Tauri bajo
//! demanda tarda entre uno y dos segundos —medido en el escritorio y en el
//! selector de acentos—, y una herramienta de capturas que abre ventana y
//! *después* captura pierde justo el momento que se quería guardar: el menú que
//! estaba abierto se cerró, el cursor se movió, la notificación desapareció.
//!
//! Así que se captura al arrancar, en unos 100 ms, y la ventana muestra ese
//! cuadro **congelado**. La lentitud de la interfaz deja de importar porque el
//! instante ya está en disco, y la selección se hace sobre una imagen quieta en
//! lugar de sobre una pantalla que sigue cambiando debajo.

pub mod captura;
pub mod comandos;
pub mod destino;
mod locales;

use gtk_layer_shell::LayerShell;
use tauri::Manager;

/// Cómo se invocó el programa.
#[derive(Debug, PartialEq, Eq)]
pub enum Modo {
    /// Abrir el selector para elegir una región.
    Selector,
    /// Guardar toda la pantalla y salir, sin interfaz.
    PantallaCompleta,
}

/// Lee el modo de los argumentos.
///
/// Separado para poder probarlo: de esto depende que apretar la tecla de captura
/// abra el selector o guarde directo, y confundirlos hace que la herramienta
/// haga lo contrario de lo que se le pidió.
pub fn modo_de(argumentos: &[String]) -> Modo {
    if argumentos
        .iter()
        .any(|a| a == "--pantalla" || a == "-p")
    {
        Modo::PantallaCompleta
    } else {
        Modo::Selector
    }
}

/// Deja la ventana del selector tapando todo, panel incluido.
///
/// En la capa de superposición y anclada a los cuatro bordes: una ventana normal
/// quedaría debajo del panel y por encima nada más, así que la selección no
/// podría llegar a lo que el panel tapa. Y con teclado, que es lo que permite
/// confirmar con Intro y salir con Esc.
/// La ventana arranca oculta y se muestra desde acá, después del layer-shell.
///
/// El orden no es negociable: `init_layer_shell` sobre una ventana ya mapeada
/// aborta con «assertion '!gtk_widget_get_mapped' failed», y a partir de ahí cada
/// llamada siguiente avisa «GtkWindow is not a layer surface». El resultado es una
/// ventana de 800x600 con decoración en medio de la pantalla en lugar de una
/// superficie que tapa todo — que es exactamente lo que pasó la primera vez.
fn tapar_todo(ventana: &tauri::WebviewWindow) {
    let Ok(gtk) = ventana.gtk_window() else {
        eprintln!("vasak-shot: la ventana no expone su GtkWindow; el selector va a quedar debajo del panel");
        return;
    };
    gtk.init_layer_shell();
    gtk.set_layer(gtk_layer_shell::Layer::Overlay);
    gtk.set_keyboard_interactivity(true);
    for borde in [
        gtk_layer_shell::Edge::Top,
        gtk_layer_shell::Edge::Bottom,
        gtk_layer_shell::Edge::Left,
        gtk_layer_shell::Edge::Right,
    ] {
        gtk.set_anchor(borde, true);
    }
    // Con los cuatro anclajes puestos, el compositor le da el tamaño de la
    // salida entera; el margen a cero evita que un tema le reste bordes.
    gtk.set_exclusive_zone(-1);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let argumentos: Vec<String> = std::env::args().collect();

    // La captura, **antes** de armar nada de Tauri. Es lo que hace que el
    // instante guardado sea el que la persona vio al apretar la tecla.
    let cruda = std::env::temp_dir().join(format!("vasak-shot-{}.png", std::process::id()));
    let tomada = match captura::capturar(&cruda) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("vasak-shot: no se pudo capturar la pantalla: {e}");
            std::process::exit(1);
        }
    };

    // Con `--pantalla` no hace falta interfaz: se guarda y se sale. Así la tecla
    // de captura puede hacer lo obvio sin abrir nada, que es lo que se espera
    // cuando se quiere la pantalla entera.
    if modo_de(&argumentos) == Modo::PantallaCompleta {
        let todo = captura::Region {
            x: 0,
            y: 0,
            ancho: tomada.ancho as i32,
            alto: tomada.alto as i32,
        };
        comandos::recordar(tomada);
        match comandos::guardar_y_copiar(todo) {
            Ok(ruta) => println!("{ruta}"),
            Err(e) => {
                eprintln!("vasak-shot: no se pudo guardar: {e}");
                std::process::exit(1);
            }
        }
        let _ = std::fs::remove_file(&cruda);
        return;
    }

    comandos::recordar(tomada);

    tauri::Builder::default()
        // El idioma de la sesión. **Con la ruta explícita de los catálogos**:
        // el plugin sólo prueba rutas relativas al ejecutable y al directorio
        // de trabajo, y ninguna existe cuando el binario está en /usr/bin. Sin
        // esto, un paquete instalado muestra las claves crudas
        // («views.home.title») en lugar de los textos. Ver `locales.rs`.
        .plugin(tauri_plugin_i18n_vsk::init_with_path(
            Some(locales::idioma_del_sistema()),
            locales::directorio(),
        ))
        // El clic derecho abre el menú de VasakOS y no el del motor del
        // navegador, que ofrece «Recargar» e «Inspeccionar elemento».
        .plugin(tauri_plugin_vsk_contextual_menu::init())
        .plugin(tauri_plugin_config_manager::init())
        .plugin(tauri_plugin_vicons::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            comandos::lienzo,
            comandos::guardar,
            comandos::copiar,
            comandos::guardar_y_copiar,
        ])
        .setup(|app| {
            // El layer-shell tiene que correr en el hilo principal —GTK aborta
            // con «GTK may only be used from the main thread»— y `setup` ya está
            // ahí, así que no hace falta despachar nada.
            if let Some(ventana) = app.get_webview_window("main") {
                tapar_todo(&ventana);
                // Recién ahora: mapearla antes haría fallar el layer-shell.
                let _ = ventana.show();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error al ejecutar la aplicación");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_argumentos_se_abre_el_selector() {
        // El caso normal: apretar la tecla y elegir con el ratón.
        assert_eq!(modo_de(&["vasak-shot".to_string()]), Modo::Selector);
    }

    #[test]
    fn con_pantalla_se_guarda_directo() {
        // Confundir los dos modos hace que la herramienta haga lo contrario de
        // lo que se le pidió, y no falla: simplemente abre una ventana cuando se
        // esperaba un archivo, o al revés.
        for bandera in ["--pantalla", "-p"] {
            assert_eq!(
                modo_de(&["vasak-shot".to_string(), bandera.to_string()]),
                Modo::PantallaCompleta,
                "con {bandera}"
            );
        }
    }

    #[test]
    fn un_argumento_desconocido_no_cambia_el_modo() {
        // Mejor abrir el selector que interpretar cualquier cosa como «guardá
        // todo»: lo primero se cancela con Esc, lo segundo ya escribió el archivo.
        assert_eq!(
            modo_de(&["vasak-shot".to_string(), "--que-se-yo".to_string()]),
            Modo::Selector
        );
    }
}
