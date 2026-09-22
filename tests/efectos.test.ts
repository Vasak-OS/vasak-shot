import { describe, expect, test } from 'bun:test';
import { TAPADO_POR_OMISION } from '@/tools/anotacion';
import { difuminar, type Mapa, pixelar } from '@/tools/efectos';

/** Un mapa de `ancho × alto`, pintado por una función de posición. */
function mapa(ancho: number, alto: number, gris: (x: number, y: number) => number): Mapa {
	const datos = new Uint8ClampedArray(ancho * alto * 4);
	for (let y = 0; y < alto; y++) {
		for (let x = 0; x < ancho; x++) {
			const i = (y * ancho + x) * 4;
			const v = gris(x, y);
			datos[i] = v;
			datos[i + 1] = v;
			datos[i + 2] = v;
			datos[i + 3] = 255;
			}
	}
	return { datos, ancho, alto };
}

/** Un renglón de terminal: barras de dos píxeles, que es el ancho de un palo. */
const RENGLON = (x: number) => (Math.floor(x / 2) % 2 === 0 ? 0 : 255);

/**
 * El contraste **local** más alto de una zona: lo que decide si algo se lee.
 *
 * Local y no de la zona entera: un degradado suave de punta a punta da un número
 * enorme y no se parece en nada a un texto legible. Lo que hace legible a una
 * letra es el salto entre píxeles vecinos, así que se mide en ventanas del
 * tamaño de un par de palos.
 */
function contrasteLocal(
	m: Mapa,
	x0: number,
	y0: number,
	ancho: number,
	alto: number,
	ventana = 6
): number {
	let peor = 0;
	for (let y = y0; y < y0 + alto; y++) {
		for (let x = x0; x + ventana <= x0 + ancho; x++) {
			let min = 255;
			let max = 0;
			for (let k = 0; k < ventana; k++) {
				const v = m.datos[(y * m.ancho + x + k) * 4];
				min = Math.min(min, v);
				max = Math.max(max, v);
			}
			peor = Math.max(peor, max - min);
		}
	}
	return peor;
}

describe('pixelar', () => {
	test('no queda ningún píxel del original', () => {
		// Un damero de negro y blanco: el promedio de cada bloque es gris, así
		// que ningún píxel puede seguir valiendo lo que valía. Es la prueba de
		// que el mosaico tapa y no sólo afea.
		const original = mapa(48, 48, (x, y) => ((x + y) % 2 === 0 ? 0 : 255));
		const tapado = mapa(48, 48, (x, y) => ((x + y) % 2 === 0 ? 0 : 255));
		pixelar(tapado, { x: 0, y: 0, ancho: 48, alto: 48 }, TAPADO_POR_OMISION.pixelar);

		for (let i = 0; i < original.datos.length; i += 4) {
			expect(tapado.datos[i]).not.toBe(original.datos[i]);
		}
	});

	test('un renglón de texto deja de tener contraste', () => {
		const m = mapa(64, 16, RENGLON);
		pixelar(m, { x: 0, y: 0, ancho: 64, alto: 16 }, TAPADO_POR_OMISION.pixelar);
		// Con barras de dos píxeles y bloques de doce, cada bloque promedia seis
		// barras: lo que queda es gris parejo.
		expect(contrasteLocal(m, 0, 0, 64, 16)).toBeLessThan(40);
	});

	test('fuera del rectángulo no se toca nada', () => {
		const m = mapa(40, 40, () => 0);
		m.datos[(35 * 40 + 35) * 4] = 200;
		pixelar(m, { x: 0, y: 0, ancho: 20, alto: 20 }, 10);
		expect(m.datos[(35 * 40 + 35) * 4]).toBe(200);
	});

	test('un bloque de menos de dos píxeles no hace nada', () => {
		// Sería tapar con bloques de un píxel, o sea no tapar — y peor: parecería
		// que se tapó.
		const m = mapa(10, 10, (x) => x * 20);
		const antes = Uint8ClampedArray.from(m.datos);
		pixelar(m, { x: 0, y: 0, ancho: 10, alto: 10 }, 1);
		expect(m.datos).toEqual(antes);
	});
});

describe('difuminar', () => {
	test('un renglón de texto deja de ser legible', () => {
		// La intensidad por omisión se elige mirando este caso y no una cara en
		// una foto: es lo que de verdad se comparte.
		const m = mapa(80, 20, RENGLON);
		expect(contrasteLocal(m, 10, 5, 60, 10)).toBe(255);

		difuminar(m, { x: 0, y: 0, ancho: 80, alto: 20 }, TAPADO_POR_OMISION.difuminar);
		expect(contrasteLocal(m, 10, 5, 60, 10)).toBeLessThan(30);
	});

	test('el borde de la zona queda tan tapado como el medio', () => {
		// Acotar también la **lectura** al rectángulo dejaba legible justo el
		// borde: en la primera columna la ventana se llena de copias de esa
		// misma columna y conserva la mitad de su valor. Es el caso de alguien
		// que encuadra la zona justo sobre el texto que quiere tapar.
		const m = mapa(80, 20, RENGLON);
		difuminar(m, { x: 0, y: 0, ancho: 80, alto: 20 }, TAPADO_POR_OMISION.difuminar);
		expect(contrasteLocal(m, 0, 0, 6, 20)).toBeLessThan(30);
		expect(contrasteLocal(m, 74, 0, 6, 20)).toBeLessThan(30);
	});

	test('fuera del rectángulo no se toca nada', () => {
		const m = mapa(60, 20, (x) => (x < 30 ? 0 : 255));
		difuminar(m, { x: 0, y: 0, ancho: 30, alto: 20 }, 8);
		for (let x = 30; x < 60; x++) {
			expect(m.datos[x * 4]).toBe(255);
		}
	});

	test('el canal alfa sobrevive', () => {
		// Difuminar el alfa de una captura opaca tiene que dejarla opaca: un
		// borde semitransparente en el medio de una imagen es un agujero.
		const m = mapa(40, 40, RENGLON);
		difuminar(m, { x: 0, y: 0, ancho: 40, alto: 40 }, 6);
		for (let i = 3; i < m.datos.length; i += 4) {
			expect(m.datos[i]).toBe(255);
		}
	});

	test('un radio de cero no hace nada', () => {
		const m = mapa(20, 20, RENGLON);
		const antes = Uint8ClampedArray.from(m.datos);
		difuminar(m, { x: 0, y: 0, ancho: 20, alto: 20 }, 0);
		expect(m.datos).toEqual(antes);
	});
});
