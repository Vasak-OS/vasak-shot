//! Guardia del afinado del WebKit de la ventana.
//!
//! No corre GTK —el afinado baja al `WebKitWebView` por el árbol de widgets y no
//! se puede montar sin un display ni un WebKit reales—: lee la fuente y comprueba
//! que las piezas sigan en su lugar. Que esto viva en `commands/webkit.rs` no es
//! un detalle: ese archivo lo excluye la cobertura, y mover el afinado a otro
//! lado volvería a bloquear el merge por cobertura de lo que ninguna prueba puede
//! ejercitar. Por eso también se comprueba que el comando esté registrado en
//! `lib.rs` y que el frontend lo invoque al montar.

use std::path::PathBuf;

fn fuente(rel: &str) -> String {
    let ruta = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&ruta)
        .unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", ruta.display()))
}

#[test]
fn el_afinado_vive_en_commands_webkit() {
    let webkit = fuente("src/commands/webkit.rs");
    assert!(
        webkit.contains("pub fn tune_webview"),
        "falta el comando `tune_webview` en commands/webkit.rs"
    );
    assert!(
        webkit.contains("set_enable_developer_extras(false)"),
        "el afinado tiene que apagar las extras de desarrollo"
    );
    assert!(
        webkit.contains("set_enable_back_forward_navigation_gestures(false)"),
        "el afinado tiene que apagar los gestos de navegación atrás/adelante"
    );
    assert!(
        webkit.contains("CacheModel::DocumentViewer"),
        "el afinado tiene que usar el caché de visor de documento"
    );
}

#[test]
fn el_comando_esta_registrado_en_lib() {
    let lib = fuente("src/lib.rs");
    assert!(
        lib.contains("commands::webkit::tune_webview"),
        "`tune_webview` tiene que estar en el generate_handler! de lib.rs"
    );
}

#[test]
fn el_frontend_lo_invoca_al_montar() {
    let app = fuente("../src/App.vue");
    assert!(
        app.contains("invoke('tune_webview')"),
        "App.vue tiene que invocar `tune_webview` al montar"
    );
}
