/**
 * Las ventanas que había cuando se tomó la captura.
 *
 * Llegan de Rust ya traducidas a coordenadas de esta pantalla y ordenadas de la
 * de más adelante a la de más atrás, así que acá sólo queda decidir cuál está
 * bajo el puntero.
 */

import type { Punto } from '@/tools/region';

export interface Ventana {
	x: number;
	y: number;
	ancho: number;
	alto: number;
}

/**
 * La ventana que está bajo el punto, o `null`.
 *
 * La **primera** que lo contiene, que es la de más adelante: la lista viene
 * ordenada por el último foco. Sin ese orden, dos ventanas superpuestas darían
 * la que el compositor haya puesto primero en su lista, que no es la que se ve.
 */
export function ventanaEn(ventanas: Ventana[], punto: Punto | null): Ventana | null {
	if (!punto) return null;
	return (
		ventanas.find(
			(v) => punto.x >= v.x && punto.x < v.x + v.ancho && punto.y >= v.y && punto.y < v.y + v.alto
		) ?? null
	);
}
