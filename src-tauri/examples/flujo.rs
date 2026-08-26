//! Prueba del flujo: capturar y recortar. El portapapeles se prueba a mano
//! porque reemplaza lo que la persona tenga copiado.
fn main() {
    let dir = std::env::temp_dir().join("vsk-shot-flujo");
    let _ = std::fs::create_dir_all(&dir);
    let cruda = dir.join("cruda.png");
    let recorte = dir.join("recorte.png");

    let t0 = std::time::Instant::now();
    let captura = match vasak_shot_lib::captura::capturar(&cruda) {
        Ok(c) => c,
        Err(e) => { eprintln!("captura falló: {e}"); std::process::exit(1) }
    };
    println!("  capturar: {:>4} ms   {}x{}", t0.elapsed().as_millis(), captura.ancho, captura.alto);

    let region = vasak_shot_lib::captura::Region { x: 400, y: 300, ancho: 300, alto: 200 };
    let t1 = std::time::Instant::now();
    if let Err(e) = vasak_shot_lib::captura::recortar(&cruda, region, &recorte) {
        eprintln!("recorte falló: {e}"); std::process::exit(1);
    }
    let bytes = std::fs::metadata(&recorte).map(|m| m.len()).unwrap_or(0);
    println!("  recortar: {:>4} ms   {} bytes", t1.elapsed().as_millis(), bytes);

    // Y que el recorte tenga el tamaño pedido, no el de la pantalla.
    match image::open(&recorte) {
        Ok(i) => println!("  el recorte mide {}x{} (pedido 300x200)", i.width(), i.height()),
        Err(e) => println!("  no se pudo releer: {e}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}
