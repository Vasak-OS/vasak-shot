//! Cuánto cuesta cada paso después de que el instante ya está congelado.
//!
//! Sin compositor: arma una imagen del tamaño de una pantalla y mide lo que la
//! aplicación hace con ella. La conversación con el compositor **no** se puede
//! medir desde acá —el permiso de captura es por ejecutable y lo tiene
//! `/usr/bin/vasak-shot`, así que un binario compilado en un árbol de trabajo
//! falla antes de empezar— y por eso se mide lo demás, que es lo que decide si
//! la interfaz llega tarde.
fn main() {
    let ancho = 1920;
    let alto = 1080;

    // Ruido y no un color liso: un PNG de un solo color se comprime a nada y el
    // número que saldría no se parecería al de una pantalla de verdad.
    let mut imagen = image::RgbaImage::new(ancho, alto);
    let mut semilla: u32 = 0x1234_5678;
    for p in imagen.pixels_mut() {
        semilla = semilla.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let b = semilla.to_le_bytes();
        *p = image::Rgba([b[0], b[1], b[2], 255]);
    }

    let dir = std::env::temp_dir().join("vsk-shot-tiempos");
    let _ = std::fs::create_dir_all(&dir);
    let cruda = dir.join("cruda.png");
    let recorte = dir.join("recorte.png");

    let t0 = std::time::Instant::now();
    imagen.save(&cruda).expect("guardar el PNG");
    println!(
        "  escribir el PNG  {:>4} ms   {ancho}x{alto}",
        t0.elapsed().as_millis()
    );

    let region = vasak_shot_lib::captura::Region {
        x: 400,
        y: 300,
        ancho: 300,
        alto: 200,
    };
    let t1 = std::time::Instant::now();
    vasak_shot_lib::captura::recortar(&cruda, region, &recorte).expect("recortar");
    println!(
        "  recortar         {:>4} ms   300x200",
        t1.elapsed().as_millis()
    );

    let _ = std::fs::remove_dir_all(&dir);
}
