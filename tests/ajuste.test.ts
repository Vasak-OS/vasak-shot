import { describe, expect, test } from 'bun:test';
import { ajustar, anclaDe, contiene, correr, limitar, type Region, ROLES } from '@/tools/region';

const LIENZO = { ancho: 1920, alto: 1080 };
const CENTRO: Region = { x: 400, y: 300, ancho: 200, alto: 100 };

describe('ajustar', () => {
	test('cada rol mueve el borde que nombra, y ninguno más', () => {
		expect(ajustar(CENTRO, 'l', 50, 0, LIENZO)).toEqual({ x: 450, y: 300, ancho: 150, alto: 100 });
		expect(ajustar(CENTRO, 'r', 50, 0, LIENZO)).toEqual({ x: 400, y: 300, ancho: 250, alto: 100 });
		expect(ajustar(CENTRO, 't', 0, 50, LIENZO)).toEqual({ x: 400, y: 350, ancho: 200, alto: 50 });
		expect(ajustar(CENTRO, 'b', 0, 50, LIENZO)).toEqual({ x: 400, y: 300, ancho: 200, alto: 150 });
	});

	test('un tirador de esquina mueve los dos bordes que toca', () => {
		expect(ajustar(CENTRO, 'tl', 10, 20, LIENZO)).toEqual({
			x: 410,
			y: 320,
			ancho: 190,
			alto: 80,
		});
		expect(ajustar(CENTRO, 'br', 10, 20, LIENZO)).toEqual({
			x: 400,
			y: 300,
			ancho: 210,
			alto: 120,
		});
	});

	test('un borde de lado no mueve el otro eje', () => {
		// `dy` en un tirador de la izquierda no tiene que hacer nada: si lo
		// hiciera, arrastrar de costado movería el rectángulo en diagonal.
		expect(ajustar(CENTRO, 'l', 0, 999, LIENZO)).toEqual(CENTRO);
		expect(ajustar(CENTRO, 't', 999, 0, LIENZO)).toEqual(CENTRO);
	});

	test('cruzar el borde opuesto da vuelta la región, no una medida negativa', () => {
		// Con ancho negativo el CSS deja de dibujar y el rectángulo desaparece
		// justo mientras se lo está arrastrando.
		const dada = ajustar(CENTRO, 'r', -300, 0, LIENZO);
		expect(dada).toEqual({ x: 300, y: 300, ancho: 100, alto: 100 });
		expect(dada.ancho).toBeGreaterThan(0);
	});

	test('no se puede sacar la región del lienzo', () => {
		expect(ajustar(CENTRO, 'l', -9999, 0, LIENZO).x).toBe(0);
		expect(ajustar(CENTRO, 'r', 9999, 0, LIENZO)).toEqual({
			x: 400,
			y: 300,
			ancho: LIENZO.ancho - 400,
			alto: 100,
		});
		expect(ajustar(CENTRO, 'b', 0, 9999, LIENZO).alto).toBe(LIENZO.alto - 300);
	});

	test('los ocho roles hacen algo', () => {
		// Un rol que no mueve nada es un tirador que no se puede agarrar, y eso
		// no se nota hasta que alguien lo intenta.
		for (const rol of ROLES) {
			expect(ajustar(CENTRO, rol, 7, 7, LIENZO)).not.toEqual(CENTRO);
		}
	});
});

describe('anclaDe', () => {
	test('las esquinas y los lados caen donde se los ve', () => {
		expect(anclaDe('tl')).toEqual({ x: 0, y: 0 });
		expect(anclaDe('br')).toEqual({ x: 1, y: 1 });
		expect(anclaDe('t')).toEqual({ x: 0.5, y: 0 });
		expect(anclaDe('r')).toEqual({ x: 1, y: 0.5 });
	});

	test('no hay dos roles en el mismo lugar', () => {
		const lugares = new Set(ROLES.map((r) => JSON.stringify(anclaDe(r))));
		expect(lugares.size).toBe(ROLES.length);
	});
});

describe('correr', () => {
	test('mueve sin cambiar el tamaño', () => {
		const corrida = correr(CENTRO, 30, -20, LIENZO);
		expect(corrida).toEqual({ x: 430, y: 280, ancho: 200, alto: 100 });
	});

	test('contra el borde se detiene en vez de achicarse', () => {
		// Que pierda ancho al llegar al borde sería dejar de mover lo que se
		// eligió y empezar a recortarlo.
		const pegada = correr(CENTRO, -9999, -9999, LIENZO);
		expect(pegada).toEqual({ x: 0, y: 0, ancho: 200, alto: 100 });

		const alFondo = correr(CENTRO, 9999, 9999, LIENZO);
		expect(alFondo.x + alFondo.ancho).toBe(LIENZO.ancho);
		expect(alFondo.y + alFondo.alto).toBe(LIENZO.alto);
		expect(alFondo.ancho).toBe(CENTRO.ancho);
	});

	test('una región más grande que el lienzo queda en el origen', () => {
		const enorme = { x: 0, y: 0, ancho: 4000, alto: 4000 };
		expect(correr(enorme, 50, 50, LIENZO)).toEqual(enorme);
	});
});

describe('limitar', () => {
	test('lo que está adentro no se toca', () => {
		expect(limitar(CENTRO, LIENZO)).toEqual(CENTRO);
	});

	test('lo que sobresale se recorta por ese lado', () => {
		expect(limitar({ x: -50, y: -50, ancho: 200, alto: 200 }, LIENZO)).toEqual({
			x: 0,
			y: 0,
			ancho: 150,
			alto: 150,
		});
	});
});

describe('contiene', () => {
	test('adentro sí, afuera no', () => {
		expect(contiene(CENTRO, { x: 500, y: 350 })).toBe(true);
		expect(contiene(CENTRO, { x: 399, y: 350 })).toBe(false);
		expect(contiene(CENTRO, { x: 500, y: 299 })).toBe(false);
	});

	test('el borde de arriba a la izquierda cuenta; el de abajo a la derecha no', () => {
		// Es lo que evita que dos regiones pegadas se disputen su línea de
		// contacto, y lo mismo que hace el señalado de ventanas.
		expect(contiene(CENTRO, { x: CENTRO.x, y: CENTRO.y })).toBe(true);
		expect(contiene(CENTRO, { x: CENTRO.x + CENTRO.ancho, y: CENTRO.y })).toBe(false);
		expect(contiene(CENTRO, { x: CENTRO.x, y: CENTRO.y + CENTRO.alto })).toBe(false);
	});
});
