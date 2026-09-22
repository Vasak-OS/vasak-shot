//! Cómo se invocó el programa.
//!
//! Separado de `lib.rs` para poder probarlo entero: de esto depende que apretar
//! la tecla de captura abra el selector, guarde la pantalla o espere cinco
//! segundos, y confundirlos hace que la herramienta haga lo contrario de lo que
//! se le pidió — sin fallar, que es lo peor: simplemente abre una ventana
//! cuando se esperaba un archivo, o al revés.

use crate::retardo;

/// Qué se hace con la captura.
#[derive(Debug, PartialEq, Eq)]
pub enum Modo {
    /// Abrir el selector para elegir una región.
    Selector,
    /// Guardar la composición entera —todas las pantallas— y salir, sin interfaz.
    PantallaCompleta,
    /// Guardar una sola pantalla, la que se llame así, y salir.
    Salida(String),
    /// Mostrar el aviso de una captura **ya guardada**, y esperar la respuesta.
    ///
    /// No captura nada: es el proceso suelto que el selector deja atrás para
    /// que los botones del aviso tengan a alguien escuchando. Ver `aviso`.
    Aviso {
        ruta: std::path::PathBuf,
        /// Si la captura ya quedó en el portapapeles, para no ofrecer copiarla.
        copiada: bool,
        /// Los cuatro textos, una línea cada uno, como los tradujo el frontend.
        textos: String,
    },
}

/// Todo lo que se puede pedir por línea de órdenes.
#[derive(Debug, PartialEq, Eq)]
pub struct Opciones {
    pub modo: Modo,
    /// Segundos de espera antes de disparar. Cero es lo de siempre.
    pub retardo: u64,
}

/// Cómo se escribe, para cuando algo se escribió mal.
///
/// Va junto al error y no en un `--ayuda` aparte: quien se equivocó en un
/// argumento tiene el problema **ahora**, y mandarlo a ejecutar otra cosa para
/// enterarse de la forma correcta es un paso de más.
pub const USO: &str = "uso: vasak-shot [--pantalla] [--salida NOMBRE] [--retardo SEGUNDOS]";

/// Lee las opciones, o dice qué está mal.
///
/// Un argumento desconocido **no** es un error: se ignora y queda el selector.
/// Mejor abrir el selector que interpretar cualquier cosa como «guardá todo»;
/// lo primero se cancela con Esc, lo segundo ya escribió el archivo.
///
/// Lo que sí falla es un argumento conocido con un valor que no sirve. Un
/// `--retardo dos` que capturara al instante sería peor que uno que se queja:
/// la foto saldría mal y nadie sabría por qué.
pub fn leer(argumentos: &[String]) -> Result<Opciones, String> {
    let mut modo = Modo::Selector;
    let mut retardo = 0;
    let mut aviso = None;
    let mut copiada = false;
    let mut textos = String::new();
    let mut i = 0;

    while i < argumentos.len() {
        let argumento = argumentos[i].as_str();
        match bandera(argumento) {
            Some(("--aviso", pegado)) => {
                aviso = Some(
                    valor(pegado, argumentos.get(i + 1), &mut i)
                        .ok_or_else(|| "--aviso necesita la ruta de una captura".to_string())?,
                );
            }
            Some(("--ya-copiada", _)) => copiada = true,
            Some(("--textos", pegado)) => {
                // Sin queja si falta: los textos tienen reserva, y un aviso en
                // español es mejor que ningún aviso.
                textos = valor(pegado, argumentos.get(i + 1), &mut i).unwrap_or_default();
            }
            Some(("--pantalla", _)) | Some(("-p", _)) => modo = Modo::PantallaCompleta,
            Some(("--salida", pegado)) | Some(("-s", pegado)) => {
                let nombre = valor(pegado, argumentos.get(i + 1), &mut i)
                    .ok_or_else(|| format!("--salida necesita el nombre de una pantalla\n{USO}"))?;
                modo = Modo::Salida(nombre);
            }
            Some(("--retardo", pegado)) | Some(("-r", pegado)) => {
                let texto = valor(pegado, argumentos.get(i + 1), &mut i)
                    .ok_or_else(|| format!("--retardo necesita un número de segundos\n{USO}"))?;
                retardo = segundos(&texto)?;
            }
            _ => {}
        }
        i += 1;
    }

    // El aviso gana sobre cualquier modo de captura: es otro programa adentro
    // del mismo binario, y lo único que hace es mostrar una notificación de algo
    // que ya se guardó. Capturar además sería sacar una foto que nadie pidió.
    if let Some(ruta) = aviso {
        return Ok(Opciones {
            modo: Modo::Aviso {
                ruta: std::path::PathBuf::from(ruta),
                copiada,
                textos,
            },
            retardo: 0,
        });
    }

    Ok(Opciones { modo, retardo })
}

/// Parte `--clave=valor` en sus dos mitades, o devuelve la bandera sola.
///
/// Las dos formas porque las dos se escriben: `--retardo 5` sale más natural a
/// mano y `--retardo=5` es lo que pone quien la copia de un archivo `.desktop`.
/// Rechazar una de las dos sería una trampa sin ninguna ventaja.
fn bandera(argumento: &str) -> Option<(&str, Option<&str>)> {
    if !argumento.starts_with('-') {
        return None;
    }
    match argumento.split_once('=') {
        Some((clave, valor)) => Some((clave, Some(valor))),
        None => Some((argumento, None)),
    }
}

/// El valor de una bandera: el pegado con `=`, o el argumento siguiente.
///
/// Avanza el índice cuando se come el siguiente, para que no se lo vuelva a
/// leer como si fuera otra bandera.
///
/// Un valor que empieza con `-` no cuenta como valor: `--retardo --pantalla` es
/// un olvido, y tomar `--pantalla` como el número dejaría un error de parseo que
/// no dice lo que pasó de verdad.
fn valor(pegado: Option<&str>, siguiente: Option<&String>, i: &mut usize) -> Option<String> {
    if let Some(texto) = pegado {
        return (!texto.is_empty()).then(|| texto.to_string());
    }
    let texto = siguiente?;
    if texto.is_empty() || texto.starts_with('-') {
        return None;
    }
    *i += 1;
    Some(texto.clone())
}

/// Un número de segundos que tenga sentido.
///
/// Los negativos ni siquiera llegan a `parse::<u64>`, pero el mensaje que da
/// —«invalid digit found in string»— no ayuda a nadie, así que se dice acá con
/// palabras.
fn segundos(texto: &str) -> Result<u64, String> {
    let valor: u64 = texto
        .parse()
        .map_err(|_| format!("«{texto}» no es un número de segundos\n{USO}"))?;
    if valor > retardo::TECHO {
        return Err(format!(
            "un retardo de {valor} s es demasiado; el máximo es {} s",
            retardo::TECHO
        ));
    }
    Ok(valor)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Los argumentos como los recibe el programa, con su nombre adelante.
    fn args(resto: &[&str]) -> Vec<String> {
        std::iter::once("vasak-shot")
            .chain(resto.iter().copied())
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn sin_argumentos_se_abre_el_selector() {
        // El caso normal: apretar la tecla y elegir con el ratón.
        let o = leer(&args(&[])).expect("sin argumentos no hay nada que fallar");
        assert_eq!(o.modo, Modo::Selector);
        assert_eq!(o.retardo, 0);
    }

    #[test]
    fn con_pantalla_se_guarda_directo() {
        for bandera in ["--pantalla", "-p"] {
            assert_eq!(
                leer(&args(&[bandera])).map(|o| o.modo),
                Ok(Modo::PantallaCompleta),
                "con {bandera}"
            );
        }
    }

    #[test]
    fn un_argumento_desconocido_no_cambia_el_modo() {
        // Mejor abrir el selector que interpretar cualquier cosa como «guardá
        // todo»: lo primero se cancela con Esc, lo segundo ya escribió el archivo.
        assert_eq!(
            leer(&args(&["--que-se-yo"])).map(|o| o.modo),
            Ok(Modo::Selector)
        );
    }

    #[test]
    fn la_salida_se_puede_nombrar_de_las_dos_formas() {
        for escrito in [vec!["--salida", "HDMI-A-1"], vec!["--salida=HDMI-A-1"]] {
            assert_eq!(
                leer(&args(&escrito)).map(|o| o.modo),
                Ok(Modo::Salida("HDMI-A-1".to_string())),
                "con {escrito:?}"
            );
        }
    }

    #[test]
    fn una_salida_sin_nombre_no_captura_cualquier_cosa() {
        // Guardar la composición entera porque faltó el nombre sería entregar
        // algo que nadie pidió, y encima parecido a lo pedido.
        for escrito in [vec!["--salida"], vec!["--salida="], vec!["--salida", "-p"]] {
            let error = leer(&args(&escrito)).expect_err(&format!("con {escrito:?}"));
            assert!(error.contains("--salida"), "{error}");
        }
    }

    #[test]
    fn el_retardo_se_lee_de_las_dos_formas() {
        for escrito in [vec!["--retardo", "5"], vec!["--retardo=5"], vec!["-r", "5"]] {
            assert_eq!(
                leer(&args(&escrito)).map(|o| o.retardo),
                Ok(5),
                "con {escrito:?}"
            );
        }
    }

    #[test]
    fn un_retardo_que_no_es_un_numero_se_dice() {
        // Capturar al instante porque el número estaba mal escrito es la peor
        // salida: la foto sale mal y nadie se entera de por qué.
        for escrito in [
            vec!["--retardo"],
            vec!["--retardo", "dos"],
            vec!["--retardo", "-3"],
            vec!["--retardo="],
        ] {
            let error = leer(&args(&escrito)).expect_err(&format!("con {escrito:?}"));
            assert!(
                error.contains("retardo") || error.contains("número"),
                "{error}"
            );
        }
    }

    #[test]
    fn un_retardo_absurdo_se_rechaza() {
        let error = leer(&args(&["--retardo", "1000"])).expect_err("mil segundos no van");
        assert!(error.contains("máximo"), "{error}");
        // Y el tope justo sí entra: el límite es un tope, no un margen.
        assert_eq!(
            leer(&args(&["--retardo", "60"])).map(|o| o.retardo),
            Ok(retardo::TECHO)
        );
    }

    #[test]
    fn el_retardo_se_combina_con_los_demas_modos() {
        // Es la mitad de para qué sirve: `--pantalla --retardo 5` es la captura
        // de una pantalla entera con un menú abierto.
        let o = leer(&args(&["--pantalla", "--retardo", "3"])).expect("las dos juntas valen");
        assert_eq!(o.modo, Modo::PantallaCompleta);
        assert_eq!(o.retardo, 3);

        let o = leer(&args(&["--retardo", "3", "--salida", "DP-1"])).expect("y estas dos también");
        assert_eq!(o.modo, Modo::Salida("DP-1".to_string()));
        assert_eq!(o.retardo, 3);
    }

    #[test]
    fn el_valor_de_una_bandera_no_se_lee_como_otra() {
        // Sin avanzar el índice, el «5» de `--retardo 5` se volvería a mirar y
        // un `--salida DP-1 --pantalla` dejaría el modo equivocado.
        let o = leer(&args(&["--retardo", "5", "--pantalla"])).expect("las dos se leen");
        assert_eq!(o.modo, Modo::PantallaCompleta);
        assert_eq!(o.retardo, 5);
    }

    #[test]
    fn el_aviso_no_captura_nada() {
        // Es otro programa adentro del mismo binario: muestra la notificación de
        // algo que ya se guardó. Capturar además sería una foto que nadie pidió.
        let o = leer(&args(&["--aviso", "/tmp/x.png"])).expect("el aviso es válido");
        assert_eq!(
            o.modo,
            Modo::Aviso {
                ruta: std::path::PathBuf::from("/tmp/x.png"),
                copiada: false,
                textos: String::new(),
            }
        );
        assert_eq!(o.retardo, 0);
    }

    #[test]
    fn el_aviso_gana_sobre_los_modos_de_captura() {
        // Y también se lleva puesto el retardo: esperar cinco segundos para
        // mostrar un aviso de algo ya guardado no tiene sentido.
        let o = leer(&args(&[
            "--pantalla",
            "--retardo",
            "5",
            "--aviso",
            "/tmp/x.png",
            "--ya-copiada",
        ]))
        .expect("vale");
        assert!(matches!(o.modo, Modo::Aviso { copiada: true, .. }));
        assert_eq!(o.retardo, 0);
    }

    #[test]
    fn un_aviso_sin_ruta_no_muestra_nada() {
        let error = leer(&args(&["--aviso"])).expect_err("sin ruta no hay aviso");
        assert!(error.contains("--aviso"), "{error}");
    }

    #[test]
    fn los_textos_del_aviso_llegan_enteros() {
        let o = leer(&args(&[
            "--aviso",
            "/tmp/x.png",
            "--textos",
            "Guardada\nAbrir\nCarpeta\nCopiar",
        ]))
        .expect("vale");
        let Modo::Aviso { textos, .. } = o.modo else {
            panic!("tendría que ser un aviso");
        };
        assert_eq!(textos.lines().count(), 4);
    }
}
