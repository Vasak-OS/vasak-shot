import { describe, expect, test } from 'bun:test';
import type { Estilo } from '@/tools/anotacion';
import { comenzar, continuar, vale } from '@/tools/gesto';

const ESTILO: Estilo = { color: '#ff0000', grosor: 3, relleno: false };

describe('comenzar', () => {
	test('una rectangular arranca con los dos extremos juntos', () => {
		const a = comenzar('recuadro', { x: 10, y: 20 }, ESTILO);
		expect(a.puntos).toEqual([
			{ x: 10, y: 20 },
			{ x: 10, y: 20 },
		]);
	});

	test('un trazo arranca con un punto solo', () => {
		expect(comenzar('lapiz', { x: 10, y: 20 }, ESTILO).puntos).toHaveLength(1);
	});

	test('el paso se lleva su número y las demás no', () => {
		expect(comenzar('paso', { x: 0, y: 0 }, ESTILO, 4).numero).toBe(4);
		expect(comenzar('recuadro', { x: 0, y: 0 }, ESTILO, 4).numero).toBeUndefined();
	});

	test('el estilo de la herramienta es el que queda en la anotación', () => {
		const a = comenzar('elipse', { x: 0, y: 0 }, { color: '#00ff00', grosor: 8, relleno: true });
		expect(a.color).toBe('#00ff00');
		expect(a.grosor).toBe(8);
		expect(a.relleno).toBe(true);
	});
});

describe('continuar', () => {
	test('la rectangular mueve su segundo extremo', () => {
		let a = comenzar('recuadro', { x: 0, y: 0 }, ESTILO);
		a = continuar(a, { x: 50, y: 60 });
		a = continuar(a, { x: 80, y: 90 });
		expect(a.puntos).toEqual([
			{ x: 0, y: 0 },
			{ x: 80, y: 90 },
		]);
	});

	test('el trazo suma puntos', () => {
		// Si moviera el último, el lápiz dibujaría una sola línea recta que
		// persigue la mano — que se ve como que no anda.
		let a = comenzar('lapiz', { x: 0, y: 0 }, ESTILO);
		a = continuar(a, { x: 1, y: 1 });
		a = continuar(a, { x: 2, y: 2 });
		expect(a.puntos).toHaveLength(3);
	});

	test('las de un punto no se mueven', () => {
		const a = comenzar('paso', { x: 5, y: 5 }, ESTILO, 1);
		expect(continuar(a, { x: 99, y: 99 })).toBe(a);
	});
});

describe('vale', () => {
	test('un clic sin arrastrar no deja un rectángulo invisible', () => {
		// Ocuparía un lugar en el historial, así que deshacer parecería no
		// hacer nada.
		expect(vale(comenzar('recuadro', { x: 10, y: 10 }, ESTILO))).toBe(false);
		expect(vale(comenzar('difuminar', { x: 10, y: 10 }, ESTILO))).toBe(false);
	});

	test('una línea horizontal vale aunque su caja tenga alto cero', () => {
		const linea = continuar(comenzar('linea', { x: 0, y: 5 }, ESTILO), { x: 100, y: 5 });
		expect(vale(linea)).toBe(true);
		const recuadro = continuar(comenzar('recuadro', { x: 0, y: 5 }, ESTILO), { x: 100, y: 5 });
		expect(vale(recuadro)).toBe(false);
	});

	test('un texto vacío no se guarda', () => {
		const a = comenzar('texto', { x: 0, y: 0 }, ESTILO);
		expect(vale({ ...a, texto: '' })).toBe(false);
		expect(vale({ ...a, texto: '   ' })).toBe(false);
		expect(vale({ ...a, texto: 'hola' })).toBe(true);
	});

	test('un paso vale siempre: es un clic y ya está', () => {
		expect(vale(comenzar('paso', { x: 0, y: 0 }, ESTILO, 1))).toBe(true);
	});

	test('un trazo de un punto vale: es un toque', () => {
		expect(vale(comenzar('lapiz', { x: 0, y: 0 }, ESTILO))).toBe(true);
	});
});
