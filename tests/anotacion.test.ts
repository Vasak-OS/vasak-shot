import { describe, expect, test } from 'bun:test';
import {
	type Anotacion,
	admiteRelleno,
	claveDe,
	esDeUnPunto,
	esRectangular,
	esTrazo,
	estiloPorOmision,
	HERRAMIENTAS,
	proximoNumero,
	tapa,
	TAPADO_POR_OMISION,
} from '@/tools/anotacion';

describe('las familias de herramientas', () => {
	test('cada herramienta cae en una familia y en una sola', () => {
		// Una que no caiga en ninguna es una que el selector no sabe cómo
		// dibujar: el clic no hace nada y no falla nada.
		for (const tipo of HERRAMIENTAS) {
			const familias = [esRectangular(tipo), esTrazo(tipo), esDeUnPunto(tipo)].filter(Boolean);
			expect(familias).toHaveLength(1);
		}
	});

	test('las que tapan son rectangulares y no admiten relleno', () => {
		for (const tipo of HERRAMIENTAS.filter(tapa)) {
			expect(esRectangular(tipo)).toBe(true);
			expect(admiteRelleno(tipo)).toBe(false);
		}
	});

	test('sólo el recuadro y la elipse se pueden rellenar', () => {
		expect(HERRAMIENTAS.filter(admiteRelleno)).toEqual(['recuadro', 'elipse']);
	});

	test('no hay herramientas repetidas', () => {
		expect(new Set(HERRAMIENTAS).size).toBe(HERRAMIENTAS.length);
	});
});

describe('estiloPorOmision', () => {
	test('las que tapan arrancan con cuánto tapan, no con un grosor de trazo', () => {
		expect(estiloPorOmision('difuminar', '#ff0000').grosor).toBe(TAPADO_POR_OMISION.difuminar);
		expect(estiloPorOmision('pixelar', '#ff0000').grosor).toBe(TAPADO_POR_OMISION.pixelar);
	});

	test('el resaltador arranca más grueso que el lápiz', () => {
		// Un resaltador del grosor de un lápiz no resalta nada.
		expect(estiloPorOmision('resaltador', '#ff0000').grosor).toBeGreaterThan(
			estiloPorOmision('lapiz', '#ff0000').grosor
		);
	});

	test('ninguna arranca rellena', () => {
		for (const tipo of HERRAMIENTAS) {
			expect(estiloPorOmision(tipo, '#ff0000').relleno).toBe(false);
		}
	});
});

describe('proximoNumero', () => {
	function paso(numero: number): Anotacion {
		return { tipo: 'paso', puntos: [{ x: 0, y: 0 }], color: '#f00', grosor: 3, relleno: false, numero };
	}

	test('empieza en uno y sigue contando', () => {
		expect(proximoNumero([])).toBe(1);
		expect(proximoNumero([paso(1), paso(2)])).toBe(3);
	});

	test('las otras anotaciones no cuentan', () => {
		const recuadro: Anotacion = {
			tipo: 'recuadro',
			puntos: [],
			color: '#f00',
			grosor: 3,
			relleno: false,
		};
		expect(proximoNumero([recuadro, paso(1), recuadro])).toBe(2);
	});
});

describe('claveDe', () => {
	test('cada herramienta tiene su clave, con dos niveles y no tres', () => {
		// Un tercer nivel parsea igual y deja el texto vacío: un hueco en la
		// barra en lugar de un error.
		for (const tipo of HERRAMIENTAS) {
			expect(claveDe(tipo).split('.')).toHaveLength(2);
		}
		expect(claveDe('recuadro')).toBe('shot.herramientaRecuadro');
		expect(claveDe('difuminar')).toBe('shot.herramientaDifuminar');
	});

	test('no hay dos herramientas con la misma clave', () => {
		expect(new Set(HERRAMIENTAS.map(claveDe)).size).toBe(HERRAMIENTAS.length);
	});
});
