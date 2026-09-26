//! Que la lista de dependencias del `.deb` sea de Debian y de esta aplicación.
//!
//! La lista venía copiada de la plantilla `vapp` y no tenía relación con esta
//! aplicación: declaraba `libsoup2.4-1` y `libsoup-3.0-0` a la vez —las dos
//! generaciones, y el binario sólo enlaza la 3—, `libpango-1.0-0` sin
//! enlazarla, y le faltaban `libdbus-1-3` y `libgtk-layer-shell0`, que el
//! binario sí enlaza según `readelf -d`.
//!
//! `libgtk-layer-shell0` era la que de verdad rompía la instalación: nada la
//! arrastra desde `libgtk-3-0t64`, así que un `.deb` compilado con la lista
//! vieja instalaba bien y después no arrancaba, porque el enlazador dinámico no
//! encontraba `libgtk-layer-shell.so.0`. `libdbus-1-3` sí llega con `gtk3`, pero
//! el binario la enlaza directo y lo que se enlaza se declara.
//!
//! Lo que se declara se audita con `readelf -d … | grep NEEDED`, nunca con
//! `ldd`. Estas pruebas no reemplazan esa auditoría: cuidan que no vuelva a
//! entrar lo que ya se sacó y que no falte lo que se sabe que se enlaza.

use std::path::PathBuf;

fn deb_depends() -> Vec<String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", path.display()));
    let config: serde_json::Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("{} no es JSON válido: {e}", path.display()));
    config["bundle"]["linux"]["deb"]["depends"]
        .as_array()
        .expect("bundle.linux.deb.depends tiene que existir")
        .iter()
        .map(|v| {
            v.as_str()
                .expect("cada dependencia es un texto")
                .to_string()
        })
        .collect()
}

#[test]
fn las_dependencias_del_deb_tienen_nombre_de_debian() {
    for name in deb_depends() {
        // Los nombres de paquete de Debian: minúsculas, dígitos y `+-.`.
        let valid = name.len() >= 2
            && name
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphanumeric())
            && name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "+-.".contains(c));
        assert!(valid, "«{name}» no es un nombre de paquete de Debian");
        // `-devel` y `gstreamer1-*` son nombres de Fedora: en Debian no existen
        // y el paquete no se podría instalar.
        assert!(
            !name.ends_with("-devel") && !name.starts_with("gstreamer1-"),
            "«{name}» es un nombre de Fedora, no de Debian"
        );
        assert!(
            !name.ends_with("-dev"),
            "«{name}» es de compilación: el paquete instalado no lo usa"
        );
    }
}

#[test]
fn las_dependencias_del_deb_no_se_repiten() {
    let depends = deb_depends();
    let mut seen = std::collections::BTreeSet::new();
    for name in &depends {
        assert!(seen.insert(name), "«{name}» está dos veces");
    }
}

/// El binario enlaza libsoup 3 y nada más: la 2.4 viene de la plantilla.
#[test]
fn no_viajan_las_dos_generaciones_de_libsoup() {
    let depends = deb_depends();
    assert!(
        !depends.iter().any(|n| n == "libsoup2.4-1"),
        "libsoup2.4-1 no la enlaza nadie: el binario usa libsoup-3.0"
    );
}

/// pango no aparece en ningún `NEEDED`: lo arrastra `libgtk-3-0t64`, que ya
/// está declarado. Declararlo también ataba el `.deb` a un paquete que no hace
/// falta para que la aplicación arranque.
#[test]
fn no_se_declara_pango_que_no_se_enlaza() {
    let depends = deb_depends();
    assert!(
        !depends.iter().any(|n| n == "libpango-1.0-0"),
        "nada NEEDs pango directamente: lo trae libgtk-3-0t64"
    );
}

/// Lo que `readelf -d` muestra enlazado, con su paquete de Debian. Si una de
/// éstas deja de enlazarse, se saca de la lista y de acá a la vez.
#[test]
fn estan_las_bibliotecas_que_el_binario_enlaza() {
    let depends = deb_depends();
    for (soname, package) in [
        ("libcairo.so.2", "libcairo2"),
        ("libdbus-1.so.3", "libdbus-1-3"),
        ("libgdk-3.so.0", "libgtk-3-0t64"),
        ("libgdk_pixbuf-2.0.so.0", "libgdk-pixbuf-2.0-0"),
        ("libgio-2.0.so.0", "libglib2.0-0t64"),
        ("libglib-2.0.so.0", "libglib2.0-0t64"),
        ("libgtk-3.so.0", "libgtk-3-0t64"),
        ("libgtk-layer-shell.so.0", "libgtk-layer-shell0"),
        (
            "libjavascriptcoregtk-4.1.so.0",
            "libjavascriptcoregtk-4.1-0",
        ),
        ("libsoup-3.0.so.0", "libsoup-3.0-0"),
        ("libwebkit2gtk-4.1.so.0", "libwebkit2gtk-4.1-0"),
    ] {
        assert!(
            depends.iter().any(|n| n == package),
            "el binario enlaza {soname} y el .deb no declara {package}"
        );
    }
}

/// La capa de superposición del compositor, que es lo que hace que esta
/// aplicación no sea una ventana normal. Es el motivo de que exista el paquete
/// `gtk-layer-shell` en `Cargo.toml`, y lo que la lista vieja no declaraba.
#[test]
fn esta_la_capa_de_superposicion_del_compositor() {
    let depends = deb_depends();
    assert!(
        depends.iter().any(|n| n == "libgtk-layer-shell0"),
        "la superficie de selección va en la capa del compositor: sin esto el \
         .deb instala bien y la aplicación no arranca"
    );
}
