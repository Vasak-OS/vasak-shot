/**
 * La región que se está eligiendo con el ratón.
 *
 * Vive aparte del componente para poder probarla: son cuentas chicas donde un
 * error no se ve —un rectángulo que no sigue al puntero, una selección de un
 * píxel que igual se guarda— y las tres se rompen por separado.
 *
 * La normalización está también en Rust, y no es duplicación: la de Rust protege
 * el recorte de medidas negativas, esta protege lo que se **dibuja**. Con ancho
 * negativo el CSS no dibuja nada, así que el rectángulo desaparecería al arrastrar
 * hacia arriba o hacia la izquierda.
 */

export interface Punto {
	x: number;
	y: number;
}

export interface Region {
	x: number;
	y: number;
	ancho: number;
	alto: number;
}

/**
 * Cuántos píxeles de lado tiene que tener una selección para contar.
 *
 * Un clic sin arrastrar produce un rectángulo de cero o un píxel. Guardar eso
 * deja un archivo que no sirve, y peor: parece que la herramienta funcionó.
 */
export const MINIMO = 2;

/**
 * La región entre dos puntos, o `null` si no alcanza el mínimo.
 *
 * Arrastrar desde cualquier esquina da el mismo rectángulo: se toma el mínimo de
 * cada eje como origen y la distancia absoluta como medida.
 */
export function regionEntre(desde: Punto | null, hasta: Punto | null): Region | null {
	if (!desde || !hasta) return null;

	const ancho = Math.abs(hasta.x - desde.x);
	const alto = Math.abs(hasta.y - desde.y);
	if (ancho < MINIMO || alto < MINIMO) return null;

	return {
		x: Math.min(desde.x, hasta.x),
		y: Math.min(desde.y, hasta.y),
		ancho,
		alto,
	};
}

/**
 * Qué se va a entregar: lo elegido, o toda la pantalla si no se eligió nada.
 *
 * Que sin selección se guarde la pantalla entera es deliberado: apretar Intro sin
 * arrastrar es la forma más rápida de capturar todo, y no tener que elegir «toda
 * la pantalla» primero ahorra el paso más común.
 */
export function aEntregar(
	elegida: Region | null,
	pantalla: { ancho: number; alto: number } | null
): Region | null {
	if (elegida) return elegida;
	if (!pantalla) return null;
	return { x: 0, y: 0, ancho: pantalla.ancho, alto: pantalla.alto };
}

/** Las medidas para mostrar, como las lee una persona. */
export function medidasDe(region: Region | null): string {
	return region ? `${region.ancho} × ${region.alto}` : '';
}
