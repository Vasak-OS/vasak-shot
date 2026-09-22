/**
 * Las pantallas que entraron en la captura.
 *
 * Llegan de Rust ya traducidas a coordenadas de **esta** pantalla, igual que
 * las ventanas: la que el selector está tapando queda en (0, 0) y las demás
 * alrededor, con las coordenadas que les toquen —negativas las que estén arriba
 * o a la izquierda—. Eso es lo que deja pedir otra pantalla sin mover el
 * puntero hasta ella: la región viaja en el mismo espacio que una selección
 * hecha con el ratón.
 */

/** Una pantalla, con el nombre con el que el compositor la nombra. */
export interface Monitor {
	/** El del conector: `HDMI-A-1`, `eDP-1`. */
	nombre: string;
	x: number;
	y: number;
	ancho: number;
	alto: number;
}

/** Las pantallas, separadas en la que se está mirando y las demás. */
export interface Salidas {
	/** La que el selector tapa. Nula si no se pudo saber cuál es. */
	actual: Monitor | null;
	/** Las otras, si hay más de un monitor. */
	otras: Monitor[];
}

/**
 * Mientras el backend no conteste, y cuando no se pudo saber.
 *
 * No poder nombrar las pantallas no es un error que se muestre: queda el
 * arrastre, que es lo que funcionaba antes de que esto existiera.
 */
export const SIN_SALIDAS: Salidas = { actual: null, otras: [] };

/** Todas, con la de acá primero: es el orden en el que se ofrecen. */
export function todas(salidas: Salidas): Monitor[] {
	return salidas.actual ? [salidas.actual, ...salidas.otras] : salidas.otras;
}

/**
 * Cómo se muestra una pantalla en la barra.
 *
 * Con las medidas al lado del nombre porque el nombre solo —`DP-2`— no dice
 * cuál de los dos monitores es, y el tamaño sí cuando son distintos. Sin
 * traducir: es un nombre de conector y dos números.
 */
export function etiquetaDe(monitor: Monitor): string {
	return `${monitor.nombre} · ${monitor.ancho} × ${monitor.alto}`;
}
