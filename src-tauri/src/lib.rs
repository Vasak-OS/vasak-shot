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
pub mod pantalla;
pub mod preferencias;
pub mod ventanas;
pub mod wayfire;

use captura::Salida;
use gtk::prelude::*;
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
    if argumentos.iter().any(|a| a == "--pantalla" || a == "-p") {
        Modo::PantallaCompleta
    } else {
        Modo::Selector
    }
}

/// Dónde está el puntero, preguntándoselo al compositor.
///
/// # Por qué no alcanza con GDK
///
/// `GdkDevice::position()` **no funciona en Wayland**: el protocolo no deja que
/// un cliente pregunte dónde está el puntero fuera de sus propias superficies,
/// y GDK devuelve `(0, 0)` sin avisar de nada. Como `(0, 0)` cae siempre en la
/// salida que está en el origen del layout, el selector aparecía siempre en la
/// misma pantalla por más que el ratón estuviera en la otra.
///
/// Comprobado en una sesión de dos monitores apilados: el cursor en
/// `y = 2109` —la pantalla de abajo, que empieza en 1080— y GDK contestando
/// `(0, 0)`, o sea la de arriba.
///
/// # De dónde sale entonces
///
/// De wayfire, por su socket de IPC: `window-rules/get_cursor_position`
/// devuelve la posición en coordenadas del layout, que es el mismo espacio en
/// el que GDK ubica las salidas.
///
/// Si no hay socket —otro compositor, X11, una prueba— se cae a GDK, que ahí sí
/// contesta. Preguntar primero y caer después mantiene andando lo que ya
/// andaba en lugar de cambiar una limitación por otra.
fn posicion_del_puntero(display: &gtk::gdk::Display) -> Option<(i32, i32)> {
    if let Some(pos) = cursor_de_wayfire() {
        return Some(pos);
    }

    let asiento = display.default_seat()?;
    let puntero = asiento.pointer()?;
    let (_, x, y) = puntero.position();
    Some((x, y))
}

/// El cursor según wayfire, o `None` si no se lo puede preguntar.
///
/// El protocolo es el de su IPC: cuatro bytes con el largo, después el JSON.
/// Ningún error se informa hacia arriba a propósito —no hay socket, no contesta,
/// contesta otra cosa—: todos significan lo mismo para quien llama, que es
/// «preguntale a GDK».
fn cursor_de_wayfire() -> Option<(i32, i32)> {
    cursor_de_wayfire_en(wayfire::socket()?)
}

/// Lo mismo, contra un socket concreto.
///
/// La ruta entra por argumento y no se lee acá para poder probarlo sin tocar el
/// entorno del proceso: `cargo test` corre en hilos que lo comparten, así que
/// una prueba que lo modifica le cambia el mundo a las otras — y la que
/// necesitaba el socket se salteaba sola.
fn cursor_de_wayfire_en(ruta: impl AsRef<std::path::Path>) -> Option<(i32, i32)> {
    let respuesta = wayfire::pedir(ruta, "window-rules/get_cursor_position")?;
    let pos = respuesta.get("pos")?;
    // Vienen como flotantes: el compositor las lleva en subpíxeles. `floor` y no
    // `as i32` a secas, que trunca hacia cero: con un monitor a la izquierda del
    // primario las coordenadas son negativas, y `-0.5` se volvería `0` — o sea
    // el monitor del otro lado del borde.
    Some((
        pos.get("x")?.as_f64()?.floor() as i32,
        pos.get("y")?.as_f64()?.floor() as i32,
    ))
}

/// La salida donde está el puntero, y el rectángulo que ocupa en el layout.
///
/// El puntero y no la salida primaria: quien aprieta la tecla de captura está
/// mirando la pantalla donde tiene el ratón, y esperar que el selector aparezca
/// en la otra es de las cosas que hacen sentir que la herramienta está rota.
fn salida_del_puntero(display: &gtk::gdk::Display) -> Option<(gtk::gdk::Monitor, Salida)> {
    let (x, y) = posicion_del_puntero(display)?;
    let monitor = display.monitor_at_point(x, y)?;
    let g = monitor.geometry();
    Some((
        monitor,
        Salida {
            x: g.x(),
            y: g.y(),
            ancho: g.width(),
            alto: g.height(),
        },
    ))
}

/// El rectángulo que abarcan todas las salidas juntas, **con su origen**.
///
/// Es el mismo espacio en el que `grim` compone, así que sirve para dos cosas:
/// saber cuántos píxeles de imagen hay por unidad de layout —ver
/// `captura::escala_de`— y saber desde dónde compone.
///
/// El origen importa y no siempre es (0, 0): un monitor puesto a la izquierda o
/// arriba del primario tiene coordenadas negativas, y el píxel (0, 0) de la
/// imagen es la esquina mínima del rectángulo, no el cero del layout. Quedarse
/// sólo con los máximos —lo que hacía esta función— daba un ancho equivocado y
/// un desplazamiento equivocado en cuanto había una salida en negativo.
fn layout_de(display: &gtk::gdk::Display) -> Salida {
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;

    for i in 0..display.n_monitors() {
        if let Some(m) = display.monitor(i) {
            let g = m.geometry();
            min_x = min_x.min(g.x());
            min_y = min_y.min(g.y());
            max_x = max_x.max(g.x() + g.width());
            max_y = max_y.max(g.y() + g.height());
        }
    }

    // Sin ninguna salida no hay rectángulo que devolver, y un cero es más honesto
    // que los centinelas: `escala_de` lo trata como «no sé» y responde escala uno.
    if min_x > max_x || min_y > max_y {
        return Salida {
            x: 0,
            y: 0,
            ancho: 0,
            alto: 0,
        };
    }

    Salida {
        x: min_x,
        y: min_y,
        ancho: max_x - min_x,
        alto: max_y - min_y,
    }
}

/// Deja la ventana del selector tapando una salida, panel incluido, y dice cuál.
///
/// En la capa de superposición y anclada a los cuatro bordes: una ventana normal
/// quedaría debajo del panel y por encima nada más, así que la selección no
/// podría llegar a lo que el panel tapa. Y con teclado, que es lo que permite
/// confirmar con Intro y salir con Esc. La ventana arranca oculta y se muestra
/// desde afuera, después del layer-shell.
///
/// El orden no es negociable: `init_layer_shell` sobre una ventana ya mapeada
/// aborta con «assertion '!gtk_widget_get_mapped' failed», y a partir de ahí cada
/// llamada siguiente avisa «GtkWindow is not a layer surface». El resultado es una
/// ventana de 800x600 con decoración en medio de la pantalla en lugar de una
/// superficie que tapa todo — que es exactamente lo que pasó la primera vez.
///
/// Devuelve la geometría de la salida elegida y el tamaño del layout entero. Sin
/// eso el recorte no puede traducir la selección: la ventana cubre **una**
/// pantalla y la captura contiene **todas**.
fn tapar_todo(ventana: &tauri::WebviewWindow) -> Option<(Salida, Salida)> {
    let Ok(gtk) = ventana.gtk_window() else {
        eprintln!("vasak-shot: la ventana no expone su GtkWindow; el selector va a quedar debajo del panel");
        return None;
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

    let display = gtk.display();
    let layout = layout_de(&display);
    let (monitor, salida) = salida_del_puntero(&display)?;

    // **La salida se fija explícitamente.** Sin esto el compositor elige, y no hay
    // forma de saber cuál eligió: la traducción del recorte quedaría adivinando
    // sobre qué pantalla se está seleccionando. Es la mitad del bug de los dos
    // monitores; la otra mitad era no sumar el origen.
    gtk.set_monitor(&monitor);

    Some((salida, layout))
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

    // Las ventanas, **en el mismo momento que los píxeles** y antes de que
    // exista la del selector. Preguntarlo después mostraría el mapa de un
    // escritorio que ya cambió —y con la superficie propia adentro de la
    // lista—, mientras que lo que se señala tiene que coincidir con lo que la
    // imagen congelada muestra.
    comandos::recordar_ventanas(ventanas::consultar());

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
        // El diario del sistema, con el nombre de esta aplicación. Va **primero**
        // de todos los plugins: instala el gancho de pánico, y un pánico mientras
        // arranca otro plugin es de los más probables y de los que menos rastro
        // dejan — sin esto sólo queda un volcado de núcleo sin símbolos.
        .plugin(tauri_plugin_vsk_journal::init())
        .plugin(tauri_plugin_vsk_contextual_menu::init())
        .plugin(tauri_plugin_config_manager::init())
        .plugin(tauri_plugin_vicons::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            comandos::lienzo,
            comandos::guardar,
            comandos::copiar,
            comandos::guardar_y_copiar,
            comandos::ajustes,
            comandos::guardar_ajustes,
            comandos::ventanas,
        ])
        .setup(|app| {
            // El layer-shell tiene que correr en el hilo principal —GTK aborta
            // con «GTK may only be used from the main thread»— y `setup` ya está
            // ahí, así que no hace falta despachar nada.
            if let Some(ventana) = app.get_webview_window("main") {
                match tapar_todo(&ventana) {
                    Some((salida, layout)) => comandos::recordar_salida(salida, layout),
                    // Sin geometría se sigue con una sola pantalla: es lo que
                    // había antes y funciona en ese caso. Peor sería no abrir.
                    None => eprintln!(
                        "vasak-shot: no se pudo determinar en qué salida está el puntero; \
                         con varios monitores el recorte puede salir de la pantalla equivocada"
                    ),
                }
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

    /// Un wayfire de mentira, para ejercitar el protocolo en cualquier máquina.
    ///
    /// Sirve una sola respuesta con el enmarcado real —cuatro bytes de largo y
    /// después el JSON— y devuelve la ruta del socket. Sin esto, el camino que
    /// arma el pedido y parsea la respuesta no lo recorría nadie en CI: la única
    /// prueba que lo tocaba se salteaba sola donde no hay sesión.
    fn wayfire_de_mentira(
        cuerpo: &'static str,
    ) -> (std::path::PathBuf, std::thread::JoinHandle<()>) {
        use std::io::{Read, Write};

        let ruta = std::env::temp_dir().join(format!(
            "vasak-shot-prueba-{}-{:?}.sock",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_file(&ruta);
        let escucha = std::os::unix::net::UnixListener::bind(&ruta).expect("socket de prueba");

        let hilo = std::thread::spawn(move || {
            if let Ok((mut sock, _)) = escucha.accept() {
                // Se lee el pedido entero antes de contestar: si no, el cliente
                // puede ver un socket cerrado a mitad de su `write_all`.
                let mut largo = [0u8; 4];
                if sock.read_exact(&mut largo).is_ok() {
                    let mut pedido = vec![0u8; u32::from_ne_bytes(largo) as usize];
                    let _ = sock.read_exact(&mut pedido);
                }
                let _ = sock.write_all(&(cuerpo.len() as u32).to_ne_bytes());
                let _ = sock.write_all(cuerpo.as_bytes());
            }
        });

        (ruta, hilo)
    }

    /// El enmarcado y el parseo, contra un servidor de mentira.
    #[test]
    fn la_respuesta_enmarcada_se_lee_entera() {
        let (ruta, hilo) = wayfire_de_mentira(r#"{"result":"ok","pos":{"x":828.34,"y":2109.18}}"#);
        let pos = cursor_de_wayfire_en(&ruta);
        hilo.join().unwrap();
        let _ = std::fs::remove_file(&ruta);

        assert_eq!(pos, Some((828, 2109)));
    }

    /// Las negativas se redondean hacia abajo, no hacia cero.
    ///
    /// `as i32` trunca hacia cero, así que `-0.5` daría `0`: con un monitor a la
    /// izquierda del primario eso elige el del otro lado del borde, que es el
    /// mismo error que este arreglo vino a corregir.
    #[test]
    fn una_coordenada_negativa_no_se_va_al_otro_monitor() {
        let (ruta, hilo) = wayfire_de_mentira(r#"{"result":"ok","pos":{"x":-0.5,"y":-1920.5}}"#);
        let pos = cursor_de_wayfire_en(&ruta);
        hilo.join().unwrap();
        let _ = std::fs::remove_file(&ruta);

        assert_eq!(pos, Some((-1, -1921)));
    }

    /// Un marco enorme se rechaza en vez de reservar la memoria que anuncia.
    ///
    /// El largo lo dice el otro extremo y la ruta sale de una variable de
    /// entorno: sin techo, quien la controle hace que esto intente reservar
    /// gigabytes en lugar de caer a GDK.
    #[test]
    fn un_marco_gigante_no_se_reserva() {
        use std::io::{Read, Write};

        let ruta =
            std::env::temp_dir().join(format!("vasak-shot-gigante-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&ruta);
        let escucha = std::os::unix::net::UnixListener::bind(&ruta).expect("socket de prueba");

        let hilo = std::thread::spawn(move || {
            if let Ok((mut sock, _)) = escucha.accept() {
                let mut largo = [0u8; 4];
                if sock.read_exact(&mut largo).is_ok() {
                    let mut pedido = vec![0u8; u32::from_ne_bytes(largo) as usize];
                    let _ = sock.read_exact(&mut pedido);
                }
                // Anuncia casi cuatro gigabytes y no manda nada.
                let _ = sock.write_all(&u32::MAX.to_ne_bytes());
            }
        });

        let pos = cursor_de_wayfire_en(&ruta);
        hilo.join().unwrap();
        let _ = std::fs::remove_file(&ruta);

        assert_eq!(
            pos, None,
            "un largo imposible tiene que caer al camino de GDK"
        );
    }

    /// Sin compositor al que preguntarle, se cae a GDK en vez de romper.
    ///
    /// Es el camino de X11, de otro compositor y de una máquina de integración.
    /// Que devuelva `None` es lo que deja que `posicion_del_puntero` siga.
    #[test]
    fn sin_socket_no_hay_cursor_por_ipc() {
        assert_eq!(cursor_de_wayfire_en("/no/existe/este/socket"), None);
    }

    /// Con wayfire andando, la posición sale de él y no es `(0, 0)` de mentira.
    ///
    /// El bug era ése: en Wayland `GdkDevice::position()` contesta `(0, 0)` sin
    /// avisar, y `(0, 0)` cae siempre en la salida del origen del layout, así
    /// que el selector aparecía siempre en la misma pantalla.
    ///
    /// Se saltea sin socket —una máquina de integración no tiene sesión— porque
    /// lo único que puede comprobar acá es que el compositor conteste.
    #[test]
    fn con_wayfire_la_posicion_sale_del_compositor() {
        if std::env::var_os("WAYFIRE_SOCKET").is_none() {
            return;
        }

        let ruta = std::env::var_os("WAYFIRE_SOCKET").expect("recién se comprobó");
        let (x, y) = cursor_de_wayfire_en(ruta).expect("wayfire tiene que contestar la posición");
        // Nada de rangos inventados: sólo que sea una coordenada de layout
        // plausible. Lo que importa es que venga del compositor.
        assert!(x > i32::MIN && y > i32::MIN, "({x}, {y})");
    }

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
