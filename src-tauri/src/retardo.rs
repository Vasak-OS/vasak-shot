//! La espera entre el pedido y el disparo.
//!
//! Existe para poder fotografiar lo que se cierra al perder el foco: un menú
//! abierto, un desplegable, un globo de ayuda. Que la captura se tome **antes**
//! de abrir la ventana —el orden que esta aplicación eligió a propósito— no
//! alcanza, porque el instante que se congela sigue siendo el de apretar la
//! tecla, y la mano que aprieta la tecla es la misma que tendría que estar
//! sosteniendo el menú.
//!
//! Durante la espera **no hay nada de esta aplicación en pantalla**. Es el
//! punto: cualquier interfaz visible arruinaría justo lo que se está por
//! capturar.

use std::process::{Command, Stdio};
use std::time::Duration;

/// El retardo más largo que se acepta, en segundos.
///
/// No hay ninguna razón técnica para un tope, pero sí una práctica: un
/// `--retardo 1000` es casi siempre un dedo que erró la tecla, y el resultado
/// sería un proceso invisible esperando diecisiete minutos para capturar algo
/// que nadie está mirando. Un minuto alcanza de sobra para abrir un menú.
pub const TECHO: u64 = 60;

/// En qué segundos se muestra la cuenta regresiva.
///
/// **El último segundo no lleva aviso, y por eso esto no es `(1..=n).rev()`.**
/// La notificación se ve en la pantalla, así que sale en la captura: contar
/// hasta uno dejaría el propio aviso adentro de la foto. El que se muestra en
/// el «2» se apaga solo un segundo después —`EXPIRA`—, o sea justo cuando
/// empieza el segundo que se va a fotografiar.
///
/// Con un retardo de un segundo no se anuncia nada: no hay dónde poner la
/// cuenta sin que salga en la foto, y un segundo tampoco alcanza para leerla.
pub fn cuenta(segundos: u64) -> Vec<u64> {
    (2..=segundos).rev().collect()
}

/// Cuánto vive cada aviso. Un segundo: hasta que lo reemplace el siguiente.
const EXPIRA: &str = "1000";

/// Espera los segundos pedidos, contándolos en una notificación.
///
/// La cuenta va en una notificación y no en una ventana porque es lo único que
/// puede aparecer sin robarle el foco a lo que se está por capturar — que es
/// exactamente lo que se rompería si esta aplicación mostrara algo.
pub fn esperar(segundos: u64) {
    if segundos == 0 {
        return;
    }

    let mut id = None;
    for restante in cuenta(segundos) {
        id = anunciar(restante, id);
        std::thread::sleep(Duration::from_secs(1));
    }

    // El último segundo, con la pantalla ya limpia.
    std::thread::sleep(Duration::from_secs(1));
}

/// Muestra —o reemplaza— el aviso de la cuenta, y devuelve su identificador.
///
/// Reemplazar y no apilar: cinco notificaciones una debajo de la otra tapan
/// media pantalla, que es la que se está por fotografiar. El identificador sale
/// de `--print-id` en el primer aviso y vuelve en `--replace-id` en los
/// siguientes.
///
/// El texto es sólo el número y la unidad, sin traducir. No es por descuido:
/// esto corre **antes** que Tauri y sus plugins, así que el catálogo de idioma
/// todavía no está cargado, y cargarlo acá a mano sería una segunda
/// implementación del que ya tiene el plugin. «5 s» junto al icono de una
/// cámara y el nombre de la aplicación se entiende en los dos idiomas.
fn anunciar(restante: u64, id: Option<u32>) -> Option<u32> {
    let mut orden = Command::new("notify-send");
    orden.args([
        "--app-name=vasak-shot",
        "--icon=camera-photo",
        "--expire-time",
        EXPIRA,
        "--print-id",
    ]);
    if let Some(anterior) = id {
        orden.args(["--replace-id", &anterior.to_string()]);
    }
    orden.arg(format!("{restante} s"));

    let salida = orden.stderr(Stdio::null()).output().ok()?;
    String::from_utf8(salida.stdout).ok()?.trim().parse().ok()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_cuenta_termina_antes_del_disparo() {
        // Lo que se cuenta no llega a uno: ese segundo es el que se fotografía,
        // y un aviso ahí saldría adentro de la captura.
        assert_eq!(cuenta(5), vec![5, 4, 3, 2]);
        assert_eq!(cuenta(3), vec![3, 2]);
        assert_eq!(cuenta(2), vec![2]);
    }

    #[test]
    fn un_retardo_muy_corto_no_anuncia_nada() {
        // Con uno no hay dónde poner la cuenta, y con cero no hay espera.
        assert!(cuenta(1).is_empty());
        assert!(cuenta(0).is_empty());
    }

    #[test]
    fn la_espera_dura_lo_que_se_pidio() {
        // El aviso del último paso se apaga justo cuando arranca el segundo sin
        // aviso: la cuenta más ese segundo tienen que dar el retardo pedido.
        for segundos in [2_u64, 3, 5, 10] {
            assert_eq!(
                cuenta(segundos).len() as u64 + 1,
                segundos,
                "con {segundos} s"
            );
        }
    }

    #[test]
    fn sin_espera_no_se_duerme() {
        // Cero tiene que ser gratis: es el valor de siempre, el de apretar la
        // tecla y capturar.
        let antes = std::time::Instant::now();
        esperar(0);
        assert!(antes.elapsed() < Duration::from_millis(100));
    }
}
