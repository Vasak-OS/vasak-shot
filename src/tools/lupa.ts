/**
 * La lupa: el mismo fondo, ampliado alrededor del puntero.
 *
 * No lee píxeles ni pasa por Rust. La captura ya está en el DOM como
 * `background-image`, así que ampliarla es la misma imagen con otra escala y
 * otro origen — y eso vale también para que no se pueda desincronizar de lo que
 * se ve debajo.
 *
 * Las cuentas viven acá y no en el componente porque son las que se rompen sin
 * hacer ruido: una lupa corrida un poco muestra un píxel que no es el que la
 * cruz señala, y eso es peor que no tener lupa.
 */

import type { Lienzo, Salida } from '@/tools/lienzo';
import { medidaEnCss } from '@/tools/lienzo';
import type { Punto } from '@/tools/region';

/** Cuántas veces se amplía. */
export const AUMENTO = 8;

/** El lado de la caja, en píxeles CSS. Impar sobre el aumento, para que la cruz caiga en un píxel. */
export const TAMANIO = 136;

/** A qué distancia del puntero se pone la caja. */
export const MARGEN = 24;

/**
 * El fondo de la lupa para que `punto` quede en el centro de la caja.
 *
 * El punto está en coordenadas del selector, o sea de **esta** pantalla. La
 * imagen contiene todas, así que primero hay que llevarlo al espacio de la
 * imagen sumándole el origen de esta salida — el mismo desplazamiento que el
 * fondo de la pantalla aplica en negativo.
 */
export function fondoDeLupa(
	punto: Punto,
	lienzo: Lienzo,
	aumento = AUMENTO,
	tamanio = TAMANIO
): { size: string; position: string } {
	const medida = medidaEnCss(lienzo);
	const enLaImagen: Punto = { x: lienzo.salida.x + punto.x, y: lienzo.salida.y + punto.y };
	return {
		size: `${medida.ancho * aumento}px ${medida.alto * aumento}px`,
		position: `${tamanio / 2 - enLaImagen.x * aumento}px ${tamanio / 2 - enLaImagen.y * aumento}px`,
	};
}

/**
 * Dónde poner la caja para que no tape el punto ni se salga de la pantalla.
 *
 * Abajo y a la derecha del puntero, y se da vuelta contra el borde. Sin eso, la
 * lupa se corta justo cuando más se la necesita, que es eligiendo el borde de
 * algo — y elegir el borde de la pantalla es de los casos más comunes.
 */
export function posicionDeLupa(
	punto: Punto,
	pantalla: Salida | { ancho: number; alto: number },
	tamanio = TAMANIO,
	margen = MARGEN
): Punto {
	const eje = (valor: number, limite: number) =>
		valor + margen + tamanio > limite ? valor - margen - tamanio : valor + margen;
	return {
		x: Math.max(0, eje(punto.x, pantalla.ancho)),
		y: Math.max(0, eje(punto.y, pantalla.alto)),
	};
}
