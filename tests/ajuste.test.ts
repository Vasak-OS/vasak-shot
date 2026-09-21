import { describe, expect, test } from 'bun:test';
import {
	ajustar,
	anclaDe,
	contiene,
	correr,
	limitar,
	redimensionar,
	type Region,
	ROLES,
} from '@/tools/region';

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

describe('redimensionar', () => {
	const PUNTO_FIJO = { x: 400, y: 300 };

	test('el borde arrastrado queda donde está el puntero', () => {
		expect(redimensionar(CENTRO, 'r', { x: 700, y: 999 }, LIENZO)).toEqual({
			x: 400,
			y: 300,
			ancho: 300,
			alto: 100,
		});
		expect(redimensionar(CENTRO, 'b', { x: 999, y: 500 }, LIENZO)).toEqual({
			x: 400,
			y: 300,
			ancho: 200,
			alto: 200,
		});
	});

	test('después de cruzar el borde opuesto, el puntero sigue arrastrando', () => {
		// Con diferencias sucesivas esto se rompía: al cruzar, la región se da
		// vuelta pero el rol sigue nombrando el borde de antes, así que el
		// movimiento siguiente agarraba el que ahora estaba del otro lado.
		const cruzada = redimensionar(CENTRO, 'r', { x: 350, y: 350 }, LIENZO);
		expect(cruzada).toEqual({ x: 350, y: 300, ancho: 50, alto: 100 });

		// Segundo movimiento en la misma dirección: el borde sigue al puntero.
		const masLejos = redimensionar(CENTRO, 'r', { x: 340, y: 350 }, LIENZO);
		expect(masLejos.x).toBe(340);
		expect(masLejos.x + masLejos.ancho).toBe(PUNTO_FIJO.x);

		// Y cambiando de dirección, también.
		const devuelta = redimensionar(CENTRO, 'r', { x: 380, y: 350 }, LIENZO);
		expect(devuelta.x).toBe(380);
		expect(devuelta.x + devuelta.ancho).toBe(PUNTO_FIJO.x);
	});

	test('el ancla no se mueve', () => {
		// Lo que el tirador no toca se queda donde está, venga el puntero de
		// donde venga.
		for (const x of [0, 100, 500, 1900]) {
			const dada = redimensionar(CENTRO, 'l', { x, y: 0 }, LIENZO);
			expect(dada.x + dada.ancho).toBe(Math.max(PUNTO_FIJO.x + CENTRO.ancho, x));
			expect(dada.y).toBe(CENTRO.y);
			expect(dada.alto).toBe(CENTRO.alto);
		}
	});

	test('no se puede arrastrar un borde fuera de la pantalla', () => {
		const afuera = redimensionar(CENTRO, 'br', { x: 99999, y: 99999 }, LIENZO);
		expect(afuera.x + afuera.ancho).toBe(LIENZO.ancho);
		expect(afuera.y + afuera.alto).toBe(LIENZO.alto);
	});
});

describe('correr contra el borde, y volver', () => {
	test('empujar contra el borde y volver deja la región donde estaba', () => {
		// Con diferencias sucesivas el delta que el límite recortó se perdía, y
		// la selección volvía corrida.
		const origen = CENTRO;
		const contra = correr(origen, -9999, 0, LIENZO);
		expect(contra.x).toBe(0);
		// Lo que hace el componente: siempre contra la región original.
		expect(correr(origen, 0, 0, LIENZO)).toEqual(origen);
	});
});
