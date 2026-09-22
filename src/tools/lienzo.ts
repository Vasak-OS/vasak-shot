/**
 * Lo que el selector necesita saber de la captura congelada.
 *
 * Vive aparte del componente porque lo usan dos: la pantalla y la lupa, que
 * dibujan la **misma imagen** con distinta escala y distinto origen. Tener la
 * forma en un solo lugar es lo que evita que una de las dos se corra.
 */

/** Un rectángulo de pantalla, en unidades del layout. */
export interface Salida {
	x: number;
	y: number;
	ancho: number;
	alto: number;
}

export interface Lienzo {
	ruta: string;
	/** El tamaño de la captura entera, con todas las salidas. */
	ancho: number;
	alto: number;
	/** La salida que esta ventana está tapando, en unidades del layout. */
	salida: Salida;
	/** Píxeles de la captura por unidad del layout, por eje. */
	escalaX: number;
	escalaY: number;
}

/**
 * El tamaño de la imagen en píxeles CSS, o sea llevada a unidades del layout.
 *
 * Es el paso que hace que un píxel CSS sea una unidad del layout, y el que
 * faltaba cuando la composición entera se estiraba dentro de una sola salida.
 */
export function medidaEnCss(lienzo: Lienzo): { ancho: number; alto: number } {
	return { ancho: lienzo.ancho / lienzo.escalaX, alto: lienzo.alto / lienzo.escalaY };
}
