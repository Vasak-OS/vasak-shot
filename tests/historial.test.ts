import { describe, expect, test } from 'bun:test';
import type { Anotacion } from '@/tools/anotacion';
import {
	agregar,
	crear,
	deshacer,
	puedeDeshacer,
	puedeRehacer,
	rehacer,
	TOPE,
} from '@/tools/historial';

function unaAnotacion(x = 0): Anotacion {
	return {
		tipo: 'recuadro',
		puntos: [
			{ x, y: 0 },
			{ x: x + 10, y: 10 },
		],
		color: '#ff0000',
		grosor: 3,
		relleno: false,
	};
}

describe('deshacer y rehacer', () => {
	test('sin nada dibujado no hay nada que deshacer', () => {
		const vacio = crear();
		expect(puedeDeshacer(vacio)).toBe(false);
		expect(puedeRehacer(vacio)).toBe(false);
		// Y pedirlo igual no rompe ni inventa una lista.
		expect(deshacer(vacio)).toBe(vacio);
		expect(rehacer(vacio)).toBe(vacio);
	});

	test('deshacer saca lo último y rehacer lo devuelve', () => {
		let h = agregar(agregar(crear(), unaAnotacion(0)), unaAnotacion(20));
		expect(h.items).toHaveLength(2);

		h = deshacer(h);
		expect(h.items).toHaveLength(1);
		expect(h.items[0].puntos[0].x).toBe(0);

		h = rehacer(h);
		expect(h.items).toHaveLength(2);
		expect(h.items[1].puntos[0].x).toBe(20);
	});

	test('dibujar algo nuevo borra lo que había para rehacer', () => {
		// La rama que se había deshecho ya no lleva a ningún lado, y dejarla
		// disponible haría reaparecer un dibujo que nadie pidió.
		let h = agregar(agregar(crear(), unaAnotacion(0)), unaAnotacion(20));
		h = deshacer(h);
		expect(puedeRehacer(h)).toBe(true);

		h = agregar(h, unaAnotacion(40));
		expect(puedeRehacer(h)).toBe(false);
		expect(h.items).toHaveLength(2);
		expect(h.items[1].puntos[0].x).toBe(40);
	});

	test('deshacer no comparte los puntos con la lista de ahora', () => {
		// Si la foto de la pila fuera la misma lista, deshacer devolvería lo que
		// se acaba de modificar y no habría a dónde volver.
		const original = unaAnotacion(0);
		let h = agregar(crear(), original);
		h = agregar(h, unaAnotacion(20));
		h.items[0].puntos[0].x = 999;

		h = deshacer(h);
		expect(h.items[0].puntos[0].x).toBe(0);
	});

	test('un trazo entero es una entrada, no una por punto', () => {
		// Un trazo a mano alzada tiene cientos de puntos intermedios. Si cada uno
		// fuera una entrada, deshacerlo pediría apretar cientos de veces.
		const trazo: Anotacion = {
			tipo: 'lapiz',
			puntos: Array.from({ length: 200 }, (_, i) => ({ x: i, y: i })),
			color: '#000000',
			grosor: 3,
			relleno: false,
		};
		const h = agregar(crear(), trazo);
		expect(h.atras).toHaveLength(1);
		expect(deshacer(h).items).toHaveLength(0);
	});

	test('la pila tiene techo', () => {
		// Cada entrada es una copia de la lista entera: sin techo, una sesión
		// larga se lleva la memoria de a poco y nadie lo relaciona.
		let h = crear();
		for (let i = 0; i < TOPE + 50; i++) h = agregar(h, unaAnotacion(i));
		expect(h.atras).toHaveLength(TOPE);
		// Y lo que sigue adentro del techo se sigue pudiendo deshacer.
		expect(puedeDeshacer(h)).toBe(true);
	});

	test('deshacer hasta el fondo deja la lista vacía y no sigue', () => {
		let h = agregar(agregar(crear(), unaAnotacion(0)), unaAnotacion(20));
		h = deshacer(deshacer(h));
		expect(h.items).toHaveLength(0);
		expect(puedeDeshacer(h)).toBe(false);
		expect(deshacer(h).items).toHaveLength(0);
	});
});
