/**
 * Deshacer y rehacer, sobre la lista de anotaciones.
 *
 * Sin nada de Vue adentro: son cuentas que se rompen solas —una pila que crece
 * sin techo, un rehacer que sobrevive a un dibujo nuevo— y se prueban solas.
 *
 * Cada función devuelve un historial **nuevo** en lugar de tocar el que recibe.
 * Es lo que hace que guardar una foto de la lista en la pila de atrás sirva de
 * algo: si se compartieran los arreglos, deshacer devolvería la misma lista que
 * se acaba de modificar.
 */

import type { Anotacion } from '@/tools/anotacion';

/**
 * Cuántos pasos atrás se guardan.
 *
 * Con techo porque cada entrada es una copia de la lista entera: sin él, una
 * sesión larga de dibujo se lleva la memoria de a poco y nadie lo relaciona.
 */
export const TOPE = 100;

export interface Historial {
	items: Anotacion[];
	atras: Anotacion[][];
	adelante: Anotacion[][];
}

export function crear(items: Anotacion[] = []): Historial {
	return { items, atras: [], adelante: [] };
}

/** Una copia de la lista, con las anotaciones copiadas también. */
function copiar(items: Anotacion[]): Anotacion[] {
	return items.map((a) => ({ ...a, puntos: a.puntos.map((p) => ({ ...p })) }));
}

/**
 * Suma una anotación.
 *
 * **Una entrada por gesto terminado**, no una por punto: un trazo a mano alzada
 * con doscientos puntos intermedios llenaría la pila y haría falta apretar
 * doscientas veces para deshacerlo. Quien llama acá suma el trazo entero, ya
 * cerrado.
 */
export function agregar(historial: Historial, anotacion: Anotacion): Historial {
	const atras = [...historial.atras, copiar(historial.items)].slice(-TOPE);
	return {
		items: [...historial.items, anotacion],
		atras,
		// Dibujar algo nuevo **borra** lo que había para rehacer: la rama que se
		// había deshecho ya no lleva a ningún lado.
		adelante: [],
	};
}

export function deshacer(historial: Historial): Historial {
	const anterior = historial.atras.at(-1);
	if (!anterior) return historial;
	return {
		items: anterior,
		atras: historial.atras.slice(0, -1),
		adelante: [...historial.adelante, copiar(historial.items)],
	};
}

export function rehacer(historial: Historial): Historial {
	const siguiente = historial.adelante.at(-1);
	if (!siguiente) return historial;
	return {
		items: siguiente,
		atras: [...historial.atras, copiar(historial.items)].slice(-TOPE),
		adelante: historial.adelante.slice(0, -1),
	};
}

export function puedeDeshacer(historial: Historial): boolean {
	return historial.atras.length > 0;
}

export function puedeRehacer(historial: Historial): boolean {
	return historial.adelante.length > 0;
}
