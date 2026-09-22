import { describe, expect, test } from 'bun:test';
import type { Anotacion, Herramienta } from '@/tools/anotacion';
import { type Encuadre, pintar } from '@/tools/pintar';

/**
 * Un contexto de canvas de mentira, que anota lo que le piden.
 *
 * Bun no tiene canvas, y lo que hay que comprobar acá no son los píxeles —de
 * eso se ocupan las pruebas de los efectos— sino **qué se le pide dibujar y en
 * qué orden**: que el orden de la lista se respete, que el resaltador sea
 * translúcido de una sola pasada, que la flecha ponga la punta en el extremo al
 * que se soltó.
 */
function contextoDeMentira(ancho = 400, alto = 300) {
	const log: string[] = [];
	const estado: Record<string, unknown> = {};
	const ctx = {
		canvas: { width: ancho, height: alto },
		save: () => log.push('save'),
		restore: () => log.push('restore'),
		beginPath: () => log.push('beginPath'),
		closePath: () => log.push('closePath'),
		moveTo: (x: number, y: number) => log.push(`moveTo ${x} ${y}`),
		lineTo: (x: number, y: number) => log.push(`lineTo ${Math.round(x)} ${Math.round(y)}`),
		stroke: () => log.push(`stroke alfa=${ctx.globalAlpha} grosor=${ctx.lineWidth}`),
		fill: () => log.push('fill'),
		fillRect: (x: number, y: number, w: number, h: number) =>
			log.push(`fillRect ${x} ${y} ${w} ${h}`),
		strokeRect: (x: number, y: number, w: number, h: number) =>
			log.push(`strokeRect ${x} ${y} ${w} ${h}`),
		ellipse: (x: number, y: number) => log.push(`ellipse ${x} ${y}`),
		arc: (x: number, y: number, r: number) => log.push(`arc ${x} ${y} ${Math.round(r)}`),
		fillText: (texto: string, x: number, y: number) => log.push(`fillText ${texto} ${x} ${y}`),
		getImageData: (x: number, y: number, w: number, h: number) => {
			log.push(`getImageData ${x} ${y} ${w} ${h}`);
			return { data: new Uint8ClampedArray(w * h * 4), width: w, height: h };
		},
		putImageData: (_d: unknown, x: number, y: number) => log.push(`putImageData ${x} ${y}`),
		globalAlpha: 1,
		lineWidth: 1,
		lineCap: '',
		lineJoin: '',
		font: '',
		textAlign: '',
		textBaseline: '',
		strokeStyle: '',
		fillStyle: '',
		estado,
	};
	return { ctx: ctx as unknown as CanvasRenderingContext2D, log };
}

const SIN_ESCALA: Encuadre = { origen: { x: 0, y: 0 }, escalaX: 1, escalaY: 1 };

function anotacion(tipo: Herramienta, extra: Partial<Anotacion> = {}): Anotacion {
	return {
		tipo,
		puntos: [
			{ x: 10, y: 20 },
			{ x: 110, y: 120 },
		],
		color: '#ff0000',
		grosor: 3,
		relleno: false,
		...extra,
	};
}

describe('pintar', () => {
	test('el orden de la lista se respeta', () => {
		// Una flecha antes de un difuminado que la cruza queda difuminada;
		// después, queda nítida encima. Componer los efectos aparte sería más
		// fácil y daría el resultado equivocado.
		const { ctx, log } = contextoDeMentira();
		pintar(ctx, [anotacion('flecha'), anotacion('difuminar'), anotacion('recuadro')], SIN_ESCALA);

		const orden = log.filter((l) => l.startsWith('putImageData') || l.startsWith('strokeRect'));
		expect(orden[0]).toStartWith('putImageData');
		expect(orden[1]).toStartWith('strokeRect');
	});

	test('el recuadro se dibuja relleno o al aire, según se pidió', () => {
		const alAire = contextoDeMentira();
		pintar(alAire.ctx, [anotacion('recuadro')], SIN_ESCALA);
		expect(alAire.log).toContain('strokeRect 10 20 100 100');

		const relleno = contextoDeMentira();
		pintar(relleno.ctx, [anotacion('recuadro', { relleno: true })], SIN_ESCALA);
		expect(relleno.log).toContain('fillRect 10 20 100 100');
	});

	test('la flecha pone la punta en el extremo al que se soltó', () => {
		const { ctx, log } = contextoDeMentira();
		pintar(ctx, [anotacion('flecha')], SIN_ESCALA);
		// Después del cuerpo, un camino cerrado y relleno que arranca en el final.
		const cuerpo = log.indexOf('moveTo 10 20');
		const cabeza = log.indexOf('moveTo 110 120');
		expect(cuerpo).toBeGreaterThanOrEqual(0);
		expect(cabeza).toBeGreaterThan(cuerpo);
		expect(log.slice(cabeza)).toContain('closePath');
		expect(log.slice(cabeza)).toContain('fill');
	});

	test('el resaltador va translúcido, más grueso y de una sola pasada', () => {
		// Trazo por trazo, cada superposición se oscurecería de nuevo hasta
		// volverse opaca justo donde la mano fue más lenta.
		const { ctx, log } = contextoDeMentira();
		pintar(
			ctx,
			[
				anotacion('resaltador', {
					puntos: [
						{ x: 0, y: 0 },
						{ x: 10, y: 10 },
						{ x: 20, y: 20 },
					],
					grosor: 12,
				}),
			],
			SIN_ESCALA
		);
		const pasadas = log.filter((l) => l.startsWith('stroke '));
		expect(pasadas).toHaveLength(1);
		// El grosor es el que dice la barra, sin factores escondidos: lo que lo
		// hace un resaltador es con qué grosor se estrena.
		expect(pasadas[0]).toBe('stroke alfa=0.35 grosor=12');
	});

	test('un trazo de un solo punto igual deja marca', () => {
		const { ctx, log } = contextoDeMentira();
		pintar(ctx, [anotacion('lapiz', { puntos: [{ x: 5, y: 5 }] })], SIN_ESCALA);
		expect(log.filter((l) => l.startsWith('stroke '))).toHaveLength(1);
		expect(log).toContain('lineTo 5 5');
	});

	test('el paso dibuja el círculo y el número adentro', () => {
		const { ctx, log } = contextoDeMentira();
		pintar(ctx, [anotacion('paso', { puntos: [{ x: 50, y: 60 }], numero: 3 })], SIN_ESCALA);
		expect(log.some((l) => l.startsWith('arc 50 60'))).toBe(true);
		expect(log).toContain('fillText 3 50 60');
	});

	test('el encuadre lleva las coordenadas al lienzo', () => {
		// Las anotaciones están en coordenadas del selector y el lienzo es del
		// tamaño de la región, en píxeles de la captura. Sin esto, todo se
		// dibuja corrido por la esquina de la selección.
		const { ctx, log } = contextoDeMentira();
		pintar(ctx, [anotacion('recuadro')], { origen: { x: 10, y: 20 }, escalaX: 2, escalaY: 2 });
		expect(log).toContain('strokeRect 0 0 200 200');
	});

	test('una anotación sin puntos no dibuja nada', () => {
		const { ctx, log } = contextoDeMentira();
		pintar(ctx, [anotacion('recuadro', { puntos: [] }), anotacion('difuminar', { puntos: [] })], SIN_ESCALA);
		expect(log).toEqual([]);
	});
});

describe('las zonas tapadas', () => {
	test('el difuminado toma un margen y devuelve sólo lo de adentro', () => {
		// Si sólo pudiera mirar adentro, el borde de la zona quedaría legible.
		const { ctx, log } = contextoDeMentira();
		pintar(ctx, [anotacion('difuminar', { grosor: 10 })], SIN_ESCALA);

		// La zona va de (10,20) a (110,120) y el margen es diez: por la
		// izquierda sólo hay diez píxeles disponibles, así que se toma desde
		// cero y el ancho queda en 120, no en 130.
		const tomado = log.find((l) => l.startsWith('getImageData'));
		expect(tomado).toBe('getImageData 0 10 120 120');
		expect(log).toContain('putImageData 0 10');
	});

	test('el mosaico se alinea con la grilla del lienzo, no con la zona', () => {
		// Alineado a la zona, mover la selección un píxel recalcula el mosaico
		// entero y se ve como un parpadeo mientras se arrastra.
		const { ctx, log } = contextoDeMentira();
		pintar(
			ctx,
			[
				anotacion('pixelar', {
					grosor: 12,
					puntos: [
						{ x: 17, y: 25 },
						{ x: 117, y: 125 },
					],
				}),
			],
			SIN_ESCALA
		);
		const tomado = log.find((l) => l.startsWith('getImageData'));
		// 17 → 12 y 25 → 24: los dos múltiplos de doce de más abajo.
		expect(tomado).toStartWith('getImageData 12 24 ');
	});

	test('una zona de medida cero no se toca', () => {
		const { ctx, log } = contextoDeMentira();
		pintar(
			ctx,
			[anotacion('difuminar', { puntos: [{ x: 10, y: 10 }, { x: 10, y: 10 }] })],
			SIN_ESCALA
		);
		expect(log).toEqual([]);
	});
});

describe('la escala del tapado', () => {
	test('el radio y el bloque van en píxeles del lienzo, no del selector', () => {
		// El grosor se elige mirando la pantalla y el efecto trabaja sobre la
		// captura, que en una pantalla HiDPI tiene el doble. Sin convertirlo,
		// lo que tapa queda a la mitad de lo pedido — y quedarse corto es
		// justamente el error que importa en algo que tapa.
		const { ctx, log } = contextoDeMentira(800, 600);
		pintar(ctx, [anotacion('difuminar', { grosor: 10 })], {
			origen: { x: 0, y: 0 },
			escalaX: 2,
			escalaY: 2,
		});
		// La zona va de (20,40) a (220,240) en el lienzo, y el margen es 20.
		expect(log.find((l) => l.startsWith('getImageData'))).toBe('getImageData 0 20 240 240');
	});

	test('el mosaico se alinea con la grilla ya escalada', () => {
		const { ctx, log } = contextoDeMentira(800, 600);
		pintar(
			ctx,
			[
				anotacion('pixelar', {
					grosor: 6,
					puntos: [
						{ x: 10, y: 10 },
						{ x: 60, y: 60 },
					],
				}),
			],
			{ origen: { x: 0, y: 0 }, escalaX: 2, escalaY: 2 }
		);
		// Bloque 6 en el selector son 12 en el lienzo; 20 cae en el múltiplo 12.
		expect(log.find((l) => l.startsWith('getImageData'))).toStartWith('getImageData 12 12 ');
	});
});
