//! Las preferencias, de ida y de vuelta por el disco.
//!
//! Los tests unitarios de `preferencias` miran el JSON; éste mira el archivo:
//! que la carpeta se cree, que la escritura por temporal y `rename` termine en
//! la ruta buena, y que lo que se lee después sea lo que se guardó.
//!
//! En su propio binario de test y con una sola función a propósito: toca
//! `XDG_CONFIG_HOME`, que es del proceso entero, y `cargo test` corre los tests
//! de un mismo binario en hilos que lo comparten. Con dos funciones acá, una le
//! cambiaría el mundo a la otra.

use vasak_shot_lib::preferencias::{self, AlSoltar, Preferencias};

#[test]
fn lo_guardado_se_vuelve_a_leer_del_disco() {
    let base = std::env::temp_dir().join(format!("vasak-shot-prefs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::env::set_var("XDG_CONFIG_HOME", &base);

    // Sin archivo, las de por omisión. Es el primer arranque de cualquiera.
    assert_eq!(preferencias::leer(), Preferencias::default());

    let elegidas = Preferencias {
        al_soltar: AlSoltar::Esperar,
        carpeta: Some(base.join("capturas")),
    };
    preferencias::escribir(&elegidas).expect("no se pudo escribir");

    // La carpeta de configuración se crea sola: en una instalación nueva no
    // existe, y sin esto la escritura fallaba en silencio.
    let ruta = preferencias::ruta().expect("no hay ruta de preferencias");
    assert!(ruta.exists(), "{} no quedó escrito", ruta.display());
    assert_eq!(preferencias::leer(), elegidas);

    // Y no queda el temporal tirado al lado del bueno.
    assert!(!ruta.with_extension("json.nuevo").exists());

    // Guardar otra vez pisa, no acumula.
    let segundas = Preferencias {
        al_soltar: AlSoltar::Copiar,
        carpeta: None,
    };
    preferencias::escribir(&segundas).expect("no se pudo reescribir");
    assert_eq!(preferencias::leer(), segundas);

    let _ = std::fs::remove_dir_all(&base);
}
