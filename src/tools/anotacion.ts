/**
 * Lo que se dibuja encima de la captura, **como datos**.
 *
 * Nada se aplica sobre la imagen hasta el momento de entregar: una anotación es
 * un tipo, unos puntos, un color y un grosor. De ahí salen gratis tres cosas —
 * deshacer, mover lo ya dibujado, y que el archivo final se componga una sola
 * vez.
 */

import type { Punto } from '@/tools/region';

/** Las diez herramientas. Las dos últimas tapan, no dibujan. */
export type Herramienta =
	| 'recuadro'
	| 'elipse'
	| 'linea'
	| 'flecha'
	| 'lapiz'
	| 'resaltador'
	| 'texto'
	| 'paso'
	| 'difuminar'
	| 'pixelar';

export const HERRAMIENTAS: Herramienta[] = [
	'recuadro',
	'elipse',
	'linea',
	'flecha',
	'lapiz',
	'resaltador',
	'texto',
	'paso',
	'difuminar',
	'pixelar',
];

export interface Anotacion {
	tipo: Herramienta;
	/**
	 * En coordenadas del selector, las mismas que la selección.
	 *
	 * Dos puntos para las formas, uno para el texto y los pasos, y todos los que
	 * haga falta para un trazo a mano alzada.
	 */
	puntos: Punto[];
	color: string;
	/**
	 * El grosor del trazo — y en las dos que tapan, **cuánto** tapan: el lado
	 * del mosaico o el radio del difuminado.
	 *
	 * Un solo campo y no dos porque es el mismo control: la rueda de la barra
	 * mueve «cuánto» en las diez, y qué significa lo dice la herramienta.
	 */
	grosor: number;
	/** Recuadro y elipse, rellenos o al aire. */
	relleno: boolean;
	texto?: string;
	numero?: number;
}

/** El estilo con el que dibuja una herramienta. Se recuerda por separado en cada una. */
export interface Estilo {
	color: string;
	grosor: number;
	relleno: boolean;
}

/** Las que se dibujan arrastrando de una esquina a la otra. */
export function esRectangular(tipo: Herramienta): boolean {
	return (
		tipo === 'recuadro' ||
		tipo === 'elipse' ||
		tipo === 'linea' ||
		tipo === 'flecha' ||
		tipo === 'difuminar' ||
		tipo === 'pixelar'
	);
}

/** Las que siguen la mano punto por punto. */
export function esTrazo(tipo: Herramienta): boolean {
	return tipo === 'lapiz' || tipo === 'resaltador';
}

/** Las que se ponen con un solo clic. */
export function esDeUnPunto(tipo: Herramienta): boolean {
	return tipo === 'texto' || tipo === 'paso';
}

/** Las que pueden ir rellenas. */
export function admiteRelleno(tipo: Herramienta): boolean {
	return tipo === 'recuadro' || tipo === 'elipse';
}

/**
 * Las que **tapan** en vez de dibujar.
 *
 * No es una distinción estética: transforman los píxeles de abajo, así que el
 * orden de la lista les importa —una flecha dibujada antes de un difuminado que
 * la cruza queda difuminada— y no tienen color.
 */
export function tapa(tipo: Herramienta): boolean {
	return tipo === 'difuminar' || tipo === 'pixelar';
}

/**
 * Cuánto tapa cada una por omisión.
 *
 * Elegidos mirando **un renglón de terminal**, que es lo que de verdad se
 * comparte, y no una cara en una foto: un difuminado flojo sobre texto grande se
 * lee igual, y un mosaico chico sobre texto monoespaciado se puede revertir.
 */
export const TAPADO_POR_OMISION: Record<'difuminar' | 'pixelar', number> = {
	difuminar: 10,
	pixelar: 12,
};

/** El estilo con el que arranca cada herramienta. */
export function estiloPorOmision(tipo: Herramienta, color: string): Estilo {
	if (tapa(tipo)) {
		return { color, grosor: TAPADO_POR_OMISION[tipo as 'difuminar' | 'pixelar'], relleno: false };
	}
	return { color, grosor: tipo === 'resaltador' ? 12 : 3, relleno: false };
}

/**
 * La clave del catálogo que nombra una herramienta.
 *
 * Llana y no anidada, como el resto: el catálogo tiene **dos** niveles, y un
 * tercero parsea igual y deja el texto vacío — un hueco en la barra en lugar de
 * un error.
 */
export function claveDe(tipo: Herramienta): string {
	return `shot.herramienta${tipo.charAt(0).toUpperCase()}${tipo.slice(1)}`;
}

/** El número que le toca al próximo paso. */
export function proximoNumero(anotaciones: Anotacion[]): number {
	return anotaciones.filter((a) => a.tipo === 'paso').length + 1;
}
