import { describe, expect, test } from 'bun:test';
import { etiquetaDe, type Monitor, type Salidas, SIN_SALIDAS, todas } from '@/tools/salidas';

/**
 * Dos monitores apilados, mirando el de abajo.
 *
 * Como los manda Rust: el que el selector tapa en (0, 0) y el otro con las
 * coordenadas que le tocan, que acá son negativas porque está arriba.
 */
const ACTUAL: Monitor = { nombre: 'HDMI-A-1', x: 0, y: 0, ancho: 1920, alto: 1080 };
const OTRA: Monitor = { nombre: 'DP-1', x: 0, y: -1080, ancho: 1920, alto: 1080 };
const DOS: Salidas = { actual: ACTUAL, otras: [OTRA] };

describe('todas', () => {
	test('la de acá va primero', () => {
		// Es el orden en el que se ofrecen, y la de acá es la que se está
		// mirando: buscarla en el medio de la lista sería raro.
		expect(todas(DOS)).toEqual([ACTUAL, OTRA]);
	});

	test('sin saber en cuál se está, quedan las que se sepan', () => {
		// Pasa cuando no se pudo averiguar qué pantalla tapa el selector. Es el
		// mismo caso que no tener ninguna: el resaltado se apaga y queda el
		// arrastre.
		expect(todas({ actual: null, otras: [OTRA] })).toEqual([OTRA]);
		expect(todas(SIN_SALIDAS)).toEqual([]);
	});
});

describe('etiquetaDe', () => {
	test('el nombre no alcanza para saber cuál es', () => {
		// «DP-2» no dice cuál de los dos monitores es; el tamaño sí, cuando son
		// distintos.
		expect(etiquetaDe(ACTUAL)).toBe('HDMI-A-1 · 1920 × 1080');
	});
});
