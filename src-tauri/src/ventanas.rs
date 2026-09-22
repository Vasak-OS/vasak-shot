//! Dónde está cada ventana, para poder señalar una y capturarla entera.
//!
//! Se lo pregunta al IPC de wayfire, que es el único que lo sabe.
//! `zwlr_foreign_toplevel_manager` —que es lo que el README suponía que hacía
//! falta— informa títulos y estados, **no rectángulos**: sirve para listar
//! ventanas, no para saber dónde están.
//!
//! Y no pasa por `permisos-globales`: el socket de wayfire es un socket Unix
//! anunciado por una variable de entorno, no un global de Wayland, así que no
//! lo toca ese filtro. Esta aplicación ya lo usa para saber dónde está el
//! puntero.

use crate::captura::Salida;
use serde_json::Value;

/// Una ventana y el rectángulo que ocupa.
///
/// En qué espacio está el rectángulo depende de quién lo devuelve, y está
/// dicho en cada función: `de_las_respuestas` las da en coordenadas del layout,
/// `en_la_pantalla` en las de la ventana del selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ventana {
    pub x: i32,
    pub y: i32,
    pub ancho: i32,
    pub alto: i32,
}

impl Ventana {
    fn derecha(&self) -> i32 {
        self.x + self.ancho
    }

    fn abajo(&self) -> i32 {
        self.y + self.alto
    }
}

/// Un rectángulo de la respuesta, si es uno con medidas.
///
/// Viene en flotantes porque wayfire lleva la geometría en subpíxeles, y se
/// redondea **hacia afuera**: el origen con `floor` y el borde opuesto con
/// `ceil`, sacando las medidas de la diferencia. Redondear el ancho por su
/// cuenta pierde un píxel del lado derecho cada vez que la parte decimal de la
/// posición y la del ancho suman más de uno — y un píxel de menos en una
/// captura de ventana es una franja del escritorio de atrás.
///
/// `floor` y no `as i32`, que trunca hacia cero: con una salida a la izquierda
/// del origen las coordenadas son negativas.
fn rectangulo(valor: &Value) -> Option<(i32, i32, i32, i32)> {
    let x = valor.get("x")?.as_f64()?;
    let y = valor.get("y")?.as_f64()?;
    let ancho = valor.get("width")?.as_f64()?;
    let alto = valor.get("height")?.as_f64()?;

    let izquierda = x.floor() as i32;
    let arriba = y.floor() as i32;
    let derecha = (x + ancho).ceil() as i32;
    let abajo = (y + alto).ceil() as i32;

    (derecha > izquierda && abajo > arriba).then_some((
        izquierda,
        arriba,
        derecha - izquierda,
        abajo - arriba,
    ))
}

/// Dónde empieza cada salida dentro del layout, por nombre.
fn origenes(salidas: &Value) -> std::collections::HashMap<String, (i32, i32)> {
    let mut mapa = std::collections::HashMap::new();
    let Some(lista) = salidas.as_array() else {
        return mapa;
    };
    for salida in lista {
        let Some(nombre) = salida.get("name").and_then(|n| n.as_str()) else {
            continue;
        };
        let Some((x, y, _, _)) = salida.get("geometry").and_then(rectangulo) else {
            continue;
        };
        mapa.insert(nombre.to_string(), (x, y));
    }
    mapa
}

/// Las ventanas de las dos respuestas, en coordenadas del **layout**.
///
/// Hacen falta las dos —`list-views` y `list-outputs`— y ésa es la parte que no
/// se ve venir: **la geometría de una vista es relativa a su salida**, no al
/// layout. Medido en una sesión de dos monitores apilados, una ventana a
/// pantalla completa en la de abajo —que empieza en `y = 1080`— contesta
/// `y = 0`. Sin sumarle el origen de su salida, señalar una ventana en el
/// monitor de abajo recortaría el de arriba.
///
/// Se quedan sólo las que son ventanas de verdad: `role` igual a `toplevel`
/// —así quedan afuera el fondo, el panel y cualquier otra superficie de capa,
/// que no son ventanas que alguien quiera capturar— y mapeadas.
///
/// **Ordenadas de la más adelante a la más atrás**, por la marca del último
/// foco. Es lo que decide cuál gana cuando dos se superponen bajo el puntero.
/// No es el orden de apilado de verdad —una ventana se puede levantar sin
/// recibir el foco— pero es lo único que la respuesta trae, y coincide con el
/// apilado en el caso normal.
pub fn de_las_respuestas(vistas: &Value, salidas: &Value) -> Vec<Ventana> {
    let origenes = origenes(salidas);
    let Some(lista) = vistas.as_array() else {
        return Vec::new();
    };

    let mut encontradas: Vec<(i64, Ventana)> = Vec::new();
    for vista in lista {
        if vista.get("role").and_then(|r| r.as_str()) != Some("toplevel") {
            continue;
        }
        if vista.get("mapped").and_then(|m| m.as_bool()) != Some(true) {
            continue;
        }
        let Some((x, y, ancho, alto)) = vista.get("geometry").and_then(rectangulo) else {
            continue;
        };
        // Sin saber dónde empieza su salida, la ventana **no entra**.
        //
        // Suponer el origen en cero parece más servicial y es peor: en un
        // layout de varias pantallas amontona las ventanas de las otras sobre
        // ésta, y ahí un clic elige un rectángulo que no tiene nada que ver
        // con lo que se señaló. Una falla que se disfraza de funcionamiento es
        // peor que una que se ve — y acá lo que se ve es que no se resalta
        // nada y queda el arrastre, que es lo que había antes.
        let Some(&(ox, oy)) = vista
            .get("output-name")
            .and_then(|n| n.as_str())
            .and_then(|nombre| origenes.get(nombre))
        else {
            continue;
        };

        let foco = vista
            .get("last-focus-timestamp")
            .and_then(|f| f.as_i64())
            .unwrap_or(0);

        encontradas.push((
            foco,
            Ventana {
                x: x + ox,
                y: y + oy,
                ancho,
                alto,
            },
        ));
    }

    // Descendente: la marca más alta es la que se enfocó último, o sea la que
    // está más adelante.
    encontradas.sort_by_key(|(foco, _)| std::cmp::Reverse(*foco));
    encontradas.into_iter().map(|(_, v)| v).collect()
}

/// Las que se ven en esta pantalla, en coordenadas de la ventana del selector.
///
/// Recorta contra la salida en lugar de descartar lo que sobresale: una ventana
/// que cruza el borde entre dos monitores sigue siendo señalable en los dos, y
/// lo que se captura es el pedazo que se ve — que es el único que hay en la
/// imagen de esta pantalla.
///
/// Y esto es también lo que deja afuera los otros escritorios: wayfire pone las
/// ventanas de los demás espacios de trabajo corridas un ancho de pantalla, así
/// que no tocan este rectángulo.
pub fn en_la_pantalla(ventanas: &[Ventana], salida: Salida) -> Vec<Ventana> {
    let (sx, sy) = (salida.x, salida.y);
    let (sd, sa) = (salida.x + salida.ancho, salida.y + salida.alto);

    ventanas
        .iter()
        .filter_map(|v| {
            let x = v.x.max(sx);
            let y = v.y.max(sy);
            let derecha = v.derecha().min(sd);
            let abajo = v.abajo().min(sa);
            (derecha > x && abajo > y).then_some(Ventana {
                x: x - sx,
                y: y - sy,
                ancho: derecha - x,
                alto: abajo - y,
            })
        })
        .collect()
}

/// Le pregunta a wayfire, o devuelve una lista vacía.
///
/// Vacía y no un error: sin wayfire —otro compositor, el IPC apagado— lo que
/// corresponde es que no se pueda señalar una ventana y quede el arrastre, que
/// es lo que había antes.
pub fn consultar() -> Vec<Ventana> {
    let Some(ruta) = crate::wayfire::socket() else {
        return Vec::new();
    };
    let (Some(vistas), Some(salidas)) = (
        crate::wayfire::pedir(&ruta, "window-rules/list-views"),
        crate::wayfire::pedir(&ruta, "window-rules/list-outputs"),
    ) else {
        return Vec::new();
    };
    de_las_respuestas(&vistas, &salidas)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Una respuesta como la que contesta wayfire de verdad, recortada.
    ///
    /// Copiada de una sesión real de dos monitores apilados: es donde se vio
    /// que la geometría de una vista es relativa a su salida.
    fn respuestas() -> (Value, Value) {
        let salidas = serde_json::json!([
            {"name": "eDP-1", "geometry": {"x": 0.0, "y": 1080.0, "width": 1920.0, "height": 1080.0}},
            {"name": "HDMI-A-2", "geometry": {"x": 0.0, "y": 0.0, "width": 1920.0, "height": 1080.0}}
        ]);
        let vistas = serde_json::json!([
            {"role": "unmanaged", "mapped": false, "output-name": "null",
             "geometry": {"x": 0.0, "y": 0.0, "width": 0.0, "height": 0.0}, "last-focus-timestamp": 0},
            {"role": "desktop-environment", "mapped": true, "output-name": "HDMI-A-2",
             "geometry": {"x": 0.0, "y": 0.0, "width": 1920.0, "height": 38.0}, "last-focus-timestamp": 0},
            {"role": "toplevel", "mapped": true, "output-name": "eDP-1",
             "geometry": {"x": 0.0, "y": 0.0, "width": 1920.0, "height": 1080.0},
             "last-focus-timestamp": 25553584651173i64},
            {"role": "toplevel", "mapped": true, "output-name": "HDMI-A-2",
             "geometry": {"x": 0.0, "y": 38.0, "width": 1920.0, "height": 1042.0},
             "last-focus-timestamp": 25542980794288i64},
            {"role": "toplevel", "mapped": true, "output-name": "eDP-1",
             "geometry": {"x": 100.0, "y": 200.0, "width": 800.0, "height": 600.0},
             "last-focus-timestamp": 21769912610400i64}
        ]);
        (vistas, salidas)
    }

    #[test]
    fn la_geometria_se_lleva_al_layout() {
        // Lo que no se ve venir: la vista de la pantalla de abajo contesta
        // `y = 0`, y esa pantalla empieza en 1080. Sin sumar el origen de su
        // salida, señalar una ventana de abajo recortaría la de arriba.
        let (vistas, salidas) = respuestas();
        let ventanas = de_las_respuestas(&vistas, &salidas);

        assert_eq!(ventanas.len(), 3, "sólo las tres `toplevel`");
        assert_eq!(
            ventanas[0],
            Ventana {
                x: 0,
                y: 1080,
                ancho: 1920,
                alto: 1080
            }
        );
        assert_eq!(
            ventanas[2],
            Ventana {
                x: 100,
                y: 1280,
                ancho: 800,
                alto: 600
            }
        );
    }

    #[test]
    fn el_fondo_y_el_panel_no_son_ventanas() {
        // Son superficies de capa: `role` es `desktop-environment`. Nadie
        // quiere «capturar el panel» señalándolo, y si estuvieran en la lista
        // taparían a las ventanas de verdad en cada punto de la pantalla.
        let (vistas, salidas) = respuestas();
        let ventanas = de_las_respuestas(&vistas, &salidas);
        assert!(!ventanas.iter().any(|v| v.alto == 38));
    }

    #[test]
    fn una_vista_sin_mapear_o_de_medida_cero_no_cuenta() {
        let (vistas, salidas) = respuestas();
        assert!(!de_las_respuestas(&vistas, &salidas)
            .iter()
            .any(|v| v.ancho == 0 || v.alto == 0));
    }

    #[test]
    fn gana_la_que_se_enfoco_ultimo() {
        // Es lo que decide cuál se señala cuando dos se superponen.
        let (vistas, salidas) = respuestas();
        let ventanas = de_las_respuestas(&vistas, &salidas);
        assert_eq!(ventanas[0].y, 1080, "la de marca más alta va primero");
        assert_eq!(ventanas[1].y, 38);
        assert_eq!(ventanas[2].y, 1280, "la más vieja, última");
    }

    #[test]
    fn una_respuesta_que_no_es_la_esperada_da_una_lista_vacia() {
        // No hay wayfire, contesta otra cosa, contesta un error: para quien
        // llama significan todos lo mismo, que es «no se puede señalar».
        let vacio = Value::Null;
        assert!(de_las_respuestas(&vacio, &vacio).is_empty());
        assert!(
            de_las_respuestas(&serde_json::json!({"error": "no such method"}), &vacio).is_empty()
        );
    }

    #[test]
    fn sin_saber_donde_empieza_su_salida_la_ventana_no_entra() {
        // Suponer el origen en cero amontonaría las ventanas de las otras
        // pantallas sobre ésta, y un clic elegiría un rectángulo que no tiene
        // nada que ver con lo que se señaló.
        let (vistas, _) = respuestas();
        assert!(de_las_respuestas(&vistas, &Value::Null).is_empty());

        // Y con una sola salida conocida pasan sólo las suyas.
        let una = serde_json::json!([
            {"name": "HDMI-A-2", "geometry": {"x": 0.0, "y": 0.0, "width": 1920.0, "height": 1080.0}}
        ]);
        assert_eq!(de_las_respuestas(&vistas, &una).len(), 1);
    }

    #[test]
    fn el_subpixel_se_redondea_hacia_afuera() {
        // Redondear el ancho por su cuenta pierde un píxel del lado derecho, y
        // un píxel de menos en una captura de ventana es una franja del
        // escritorio de atrás.
        let salidas = serde_json::json!([
            {"name": "eDP-1", "geometry": {"x": 0.0, "y": 0.0, "width": 1920.0, "height": 1080.0}}
        ]);
        let vistas = serde_json::json!([
            {"role": "toplevel", "mapped": true, "output-name": "eDP-1",
             "geometry": {"x": 10.5, "y": 20.5, "width": 100.5, "height": 200.5},
             "last-focus-timestamp": 1}
        ]);
        let ventanas = de_las_respuestas(&vistas, &salidas);
        assert_eq!(ventanas[0].x, 10);
        assert_eq!(ventanas[0].y, 20);
        // 10,5 + 100,5 = 111, y el origen quedó en 10: el ancho es 101.
        assert_eq!(ventanas[0].ancho, 101);
        assert_eq!(ventanas[0].alto, 201);
    }

    /// La pantalla de abajo del layout de las pruebas.
    fn la_de_abajo() -> Salida {
        Salida {
            x: 0,
            y: 1080,
            ancho: 1920,
            alto: 1080,
        }
    }

    #[test]
    fn se_traducen_a_coordenadas_del_selector() {
        let (vistas, salidas) = respuestas();
        let ventanas = de_las_respuestas(&vistas, &salidas);
        let acá = en_la_pantalla(&ventanas, la_de_abajo());

        // Las dos de eDP-1, ya sin el origen de la salida.
        assert_eq!(acá.len(), 2);
        assert_eq!(
            acá[0],
            Ventana {
                x: 0,
                y: 0,
                ancho: 1920,
                alto: 1080
            }
        );
        assert_eq!(
            acá[1],
            Ventana {
                x: 100,
                y: 200,
                ancho: 800,
                alto: 600
            }
        );
    }

    #[test]
    fn una_ventana_que_cruza_el_borde_se_recorta() {
        // Sigue siendo señalable, y lo que se captura es el pedazo que está en
        // esta imagen — el otro no está.
        let ventanas = [Ventana {
            x: 0,
            y: 980,
            ancho: 400,
            alto: 300,
        }];
        let acá = en_la_pantalla(&ventanas, la_de_abajo());
        assert_eq!(
            acá,
            vec![Ventana {
                x: 0,
                y: 0,
                ancho: 400,
                alto: 200
            }]
        );
    }

    #[test]
    fn otro_escritorio_no_aparece() {
        // Wayfire corre las ventanas de los otros espacios de trabajo un ancho
        // de pantalla, así que no tocan este rectángulo.
        let ventanas = [
            Ventana {
                x: 1920,
                y: 1080,
                ancho: 800,
                alto: 600,
            },
            Ventana {
                x: -1920,
                y: 1080,
                ancho: 800,
                alto: 600,
            },
        ];
        assert!(en_la_pantalla(&ventanas, la_de_abajo()).is_empty());
    }
}
