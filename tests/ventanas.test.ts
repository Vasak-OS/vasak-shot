import { describe, expect, test } from 'bun:test';
import { comoRegion, type Ventana, ventanaEn } from '@/tools/ventanas';

/** Ordenadas como las manda Rust: de la de más adelante a la de más atrás. */
const VENTANAS: Ventana[] = [
	{ x: 100, y: 100, ancho: 400, alto: 300 },
	{ x: 0, y: 0, ancho: 1920, alto: 1080 },
];

describe('ventanaEn', () => {
	test('gana la de más adelante cuando se superponen', () => {
		// Las dos contienen el punto. La primera de la lista es la que se
		// enfocó último, o sea la que se ve.
		expect(ventanaEn(VENTANAS, { x: 200, y: 200 })).toEqual(VENTANAS[0]);
	});

	test('fuera de la de arriba queda la de abajo', () => {
		expect(ventanaEn(VENTANAS, { x: 800, y: 800 })).toEqual(VENTANAS[1]);
	});

	test('el borde de abajo y el de la derecha no cuentan como adentro', () => {
		// Medio píxel de diferencia manda a la ventana de atrás. Con el borde
		// incluido, dos ventanas pegadas se pisan en su línea de contacto.
		const sola: Ventana[] = [{ x: 0, y: 0, ancho: 100, alto: 100 }];
		expect(ventanaEn(sola, { x: 0, y: 0 })).toEqual(sola[0]);
		expect(ventanaEn(sola, { x: 99, y: 99 })).toEqual(sola[0]);
		expect(ventanaEn(sola, { x: 100, y: 50 })).toBeNull();
		expect(ventanaEn(sola, { x: 50, y: 100 })).toBeNull();
	});

	test('sin puntero o sin ventanas no hay ninguna', () => {
		expect(ventanaEn(VENTANAS, null)).toBeNull();
		expect(ventanaEn([], { x: 10, y: 10 })).toBeNull();
	});
});

describe('comoRegion', () => {
	test('una ventana es una región', () => {
		expect(comoRegion({ x: 5, y: 6, ancho: 7, alto: 8 })).toEqual({
			x: 5,
			y: 6,
			ancho: 7,
			alto: 8,
		});
	});
});
