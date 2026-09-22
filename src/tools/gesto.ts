/**
 * El gesto de dibujar: de dónde cayó el puntero a una anotación.
 *
 * Aparte del componente porque las tres reglas que tiene se rompen solas y en
 * silencio: una herramienta rectangular que acumula puntos como un trazo, un
 * trazo que se queda con el primero, o un clic de cero píxeles que igual deja
 * una anotación invisible en la lista y en el historial.
 */

import {
	type Anotacion,
	type Estilo,
	esDeUnPunto,
	esRectangular,
	type Herramienta,
} from '@/tools/anotacion';
import { MINIMO, type Punto } from '@/tools/region';

/** Arranca una anotación donde cayó el puntero. */
export function comenzar(
	tipo: Herramienta,
	punto: Punto,
	estilo: Estilo,
	numero?: number
): Anotacion {
	return {
		tipo,
		// Las rectangulares arrancan con los dos extremos en el mismo lugar: el
		// segundo es el que sigue al puntero.
		puntos: esRectangular(tipo) ? [punto, punto] : [punto],
		color: estilo.color,
		grosor: estilo.grosor,
		relleno: estilo.relleno,
		...(tipo === 'paso' ? { numero } : {}),
	};
}

/**
 * Sigue al puntero.
 *
 * La rectangular mueve su segundo extremo; el trazo **suma** un punto. Que el
 * trazo moviera el último sería dibujar una sola línea recta que persigue la
 * mano, y eso se ve como que el lápiz no anda.
 */
export function continuar(anotacion: Anotacion, punto: Punto): Anotacion {
	if (esDeUnPunto(anotacion.tipo)) return anotacion;
	if (esRectangular(anotacion.tipo)) {
		return { ...anotacion, puntos: [anotacion.puntos[0], punto] };
	}
	return { ...anotacion, puntos: [...anotacion.puntos, punto] };
}

/**
 * Si la anotación vale la pena guardar.
 *
 * Un clic sin arrastrar con una herramienta rectangular deja un rectángulo de
 * cero píxeles: no se ve, y sin embargo ocupa un lugar en el historial, así que
 * deshacer parece no hacer nada. El mínimo es el mismo que el de la selección.
 */
export function vale(anotacion: Anotacion): boolean {
	if (esDeUnPunto(anotacion.tipo)) {
		return anotacion.tipo !== 'texto' || (anotacion.texto ?? '').trim().length > 0;
	}
	if (esRectangular(anotacion.tipo)) {
		if (anotacion.puntos.length < 2) return false;
		const [a, b] = anotacion.puntos;
		// La línea y la flecha valen por su largo, no por su caja: una línea
		// perfectamente horizontal tiene alto cero y es una línea igual.
		if (anotacion.tipo === 'linea' || anotacion.tipo === 'flecha') {
			return Math.hypot(b.x - a.x, b.y - a.y) >= MINIMO;
		}
		return Math.abs(b.x - a.x) >= MINIMO && Math.abs(b.y - a.y) >= MINIMO;
	}
	return anotacion.puntos.length > 0;
}
