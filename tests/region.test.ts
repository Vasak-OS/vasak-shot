import { describe, expect, test } from 'bun:test';
import { aEntregar, medidasDe, MINIMO, regionEntre } from '@/tools/region';

describe('regionEntre', () => {
	test('arrastrar desde las cuatro esquinas da el mismo rectángulo', () => {
		// Con medidas negativas el CSS no dibuja nada, así que el rectángulo
		// desaparecería al arrastrar hacia arriba o hacia la izquierda.
		const esperada = { x: 10, y: 20, ancho: 30, alto: 40 };
		expect(regionEntre({ x: 10, y: 20 }, { x: 40, y: 60 })).toEqual(esperada);
		expect(regionEntre({ x: 40, y: 20 }, { x: 10, y: 60 })).toEqual(esperada);
		expect(regionEntre({ x: 10, y: 60 }, { x: 40, y: 20 })).toEqual(esperada);
		expect(regionEntre({ x: 40, y: 60 }, { x: 10, y: 20 })).toEqual(esperada);
	});

	test('un clic sin arrastrar no es una selección', () => {
		// Guardar un rectángulo de un píxel deja un archivo que no sirve, y peor:
		// parece que la herramienta funcionó.
		expect(regionEntre({ x: 10, y: 10 }, { x: 10, y: 10 })).toBeNull();
		expect(regionEntre({ x: 10, y: 10 }, { x: 11, y: 40 })).toBeNull();
		expect(regionEntre({ x: 10, y: 10 }, { x: 40, y: 11 })).toBeNull();
	});

	test('justo en el mínimo sí cuenta', () => {
		expect(regionEntre({ x: 0, y: 0 }, { x: MINIMO, y: MINIMO })).toEqual({
			x: 0,
			y: 0,
			ancho: MINIMO,
			alto: MINIMO,
		});
	});

	test('sin puntos no hay región', () => {
		expect(regionEntre(null, { x: 1, y: 1 })).toBeNull();
		expect(regionEntre({ x: 1, y: 1 }, null)).toBeNull();
		expect(regionEntre(null, null)).toBeNull();
	});
});

describe('aEntregar', () => {
	test('sin selección se entrega toda la pantalla', () => {
		// Apretar Intro sin arrastrar es la forma más rápida de capturar todo.
		expect(aEntregar(null, { ancho: 1920, alto: 1080 })).toEqual({
			x: 0,
			y: 0,
			ancho: 1920,
			alto: 1080,
		});
	});

	test('con selección se entrega la selección', () => {
		const elegida = { x: 5, y: 5, ancho: 100, alto: 50 };
		expect(aEntregar(elegida, { ancho: 1920, alto: 1080 })).toEqual(elegida);
	});

	test('sin pantalla y sin selección no hay nada que entregar', () => {
		// Pasa mientras la captura todavía está cargando: entregar algo acá
		// mandaría una región inventada al recorte.
		expect(aEntregar(null, null)).toBeNull();
	});
});

describe('medidasDe', () => {
	test('las medidas se leen con el signo de multiplicación', () => {
		expect(medidasDe({ x: 0, y: 0, ancho: 1920, alto: 1080 })).toBe('1920 × 1080');
	});

	test('sin región no se muestra nada, en lugar de «null × null»', () => {
		expect(medidasDe(null)).toBe('');
	});
});
