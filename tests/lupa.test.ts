import { describe, expect, test } from 'bun:test';
import type { Lienzo } from '@/tools/lienzo';
import { fondoDeLupa, posicionDeLupa } from '@/tools/lupa';

/** Dos monitores apilados: el selector está en el de abajo. */
const LIENZO: Lienzo = {
	ruta: '/tmp/captura.png',
	ancho: 1920,
	alto: 2160,
	salida: { x: 0, y: 1080, ancho: 1920, alto: 1080 },
	escalaX: 1,
	escalaY: 1,
	escalaPropiaX: 1,
	escalaPropiaY: 1,
};

describe('fondoDeLupa', () => {
	test('centra el punto del selector, no el de la imagen', () => {
		// La imagen tiene las dos pantallas; el punto está en la de abajo. Sin
		// sumarle el origen de la salida, la lupa mostraría el mismo lugar del
		// monitor de arriba — y eso se ve verosímil, que es lo peor.
		const { position } = fondoDeLupa({ x: 100, y: 50 }, LIENZO, 8, 136);
		expect(position).toBe(`${68 - 100 * 8}px ${68 - (1080 + 50) * 8}px`);
	});

	test('el fondo se amplía por el aumento, en unidades del layout', () => {
		const { size } = fondoDeLupa({ x: 0, y: 0 }, LIENZO, 8, 136);
		expect(size).toBe('15360px 17280px');
	});

	test('con una pantalla HiDPI la imagen sigue midiendo lo del layout', () => {
		// La captura sale al doble, y la lupa no puede ampliar el doble de más:
		// el punto que la cruz señala dejaría de ser el que está debajo.
		const hidpi: Lienzo = {
			...LIENZO,
			ancho: 3840,
			alto: 4320,
			escalaX: 2,
			escalaY: 2,
			escalaPropiaX: 2,
			escalaPropiaY: 2,
		};
		expect(fondoDeLupa({ x: 0, y: 0 }, hidpi, 8, 136).size).toBe('15360px 17280px');
	});
});

describe('posicionDeLupa', () => {
	const PANTALLA = { ancho: 1920, alto: 1080 };

	test('va abajo y a la derecha del puntero', () => {
		expect(posicionDeLupa({ x: 500, y: 400 }, PANTALLA, 136, 24)).toEqual({ x: 524, y: 424 });
	});

	test('se da vuelta contra el borde en vez de cortarse', () => {
		// Elegir el borde de la pantalla es de los casos más comunes, y es justo
		// donde una lupa que no se da vuelta deja de verse.
		expect(posicionDeLupa({ x: 1900, y: 1070 }, PANTALLA, 136, 24)).toEqual({
			x: 1900 - 24 - 136,
			y: 1070 - 24 - 136,
		});
	});

	test('nunca queda en negativo', () => {
		// En la esquina de arriba a la izquierda hay lugar para el lado de abajo,
		// pero una pantalla más chica que la caja no tiene ninguno.
		const chica = { ancho: 100, alto: 100 };
		const puesta = posicionDeLupa({ x: 10, y: 10 }, chica, 136, 24);
		expect(puesta.x).toBeGreaterThanOrEqual(0);
		expect(puesta.y).toBeGreaterThanOrEqual(0);
	});
});
