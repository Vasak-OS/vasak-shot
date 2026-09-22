/**
 * Difuminar y pixelar, sobre los píxeles crudos.
 *
 * Acá y no adentro del canvas para poder probarlo de verdad: que una zona quede
 * **tapada** no es algo que se pueda mirar a ojo en una captura de pantalla de
 * prueba —un difuminado flojo sobre texto grande se lee igual, y un mosaico
 * chico sobre texto monoespaciado se puede revertir—, y es exactamente la clase
 * de falla que se descubre después de haber compartido la imagen.
 *
 * Trabajan **sobre** el arreglo que reciben, como `putImageData` espera.
 */

/** Un mapa de píxeles RGBA, como el que da `getImageData`. */
export interface Mapa {
	datos: Uint8ClampedArray;
	ancho: number;
	alto: number;
}

export interface Rect {
	x: number;
	y: number;
	ancho: number;
	alto: number;
}

/** El rectángulo, metido adentro del mapa y en enteros. */
function acotar(mapa: Mapa, rect: Rect): Rect | null {
	const x = Math.max(0, Math.floor(rect.x));
	const y = Math.max(0, Math.floor(rect.y));
	const derecha = Math.min(mapa.ancho, Math.ceil(rect.x + rect.ancho));
	const abajo = Math.min(mapa.alto, Math.ceil(rect.y + rect.alto));
	if (derecha <= x || abajo <= y) return null;
	return { x, y, ancho: derecha - x, alto: abajo - y };
}

/**
 * Reemplaza cada bloque por su promedio.
 *
 * El bloque **no** se alinea con el origen del rectángulo sino con el del mapa:
 * así mover la zona tapada un píxel no cambia el mosaico entero, que se ve como
 * un parpadeo mientras se arrastra.
 */
export function pixelar(mapa: Mapa, rect: Rect, bloque: number): void {
	const zona = acotar(mapa, rect);
	if (!zona || bloque < 2) return;

	const primero = Math.floor(zona.x / bloque) * bloque;
	const primeraFila = Math.floor(zona.y / bloque) * bloque;

	for (let by = primeraFila; by < zona.y + zona.alto; by += bloque) {
		for (let bx = primero; bx < zona.x + zona.ancho; bx += bloque) {
			const desdeX = Math.max(bx, zona.x);
			const desdeY = Math.max(by, zona.y);
			const hastaX = Math.min(bx + bloque, zona.x + zona.ancho);
			const hastaY = Math.min(by + bloque, zona.y + zona.alto);
			if (hastaX <= desdeX || hastaY <= desdeY) continue;

			const suma = [0, 0, 0, 0];
			let cuenta = 0;
			for (let y = desdeY; y < hastaY; y++) {
				for (let x = desdeX; x < hastaX; x++) {
					const i = (y * mapa.ancho + x) * 4;
					suma[0] += mapa.datos[i];
					suma[1] += mapa.datos[i + 1];
					suma[2] += mapa.datos[i + 2];
					suma[3] += mapa.datos[i + 3];
					cuenta++;
				}
			}
			for (let y = desdeY; y < hastaY; y++) {
				for (let x = desdeX; x < hastaX; x++) {
					const i = (y * mapa.ancho + x) * 4;
					mapa.datos[i] = suma[0] / cuenta;
					mapa.datos[i + 1] = suma[1] / cuenta;
					mapa.datos[i + 2] = suma[2] / cuenta;
					mapa.datos[i + 3] = suma[3] / cuenta;
				}
			}
		}
	}
}

/**
 * Promedia cada píxel con sus vecinos, cuatro veces.
 *
 * Cuatro pasadas de caja y no una: una sola deja un promedio rectangular que
 * sobre texto todavía se lee como texto. Encadenadas se acercan a una campana,
 * que es lo que de verdad borra la forma de las letras.
 *
 * **Lee de afuera del rectángulo y escribe sólo adentro.** Acotar también la
 * lectura parece más prolijo y deja legible justamente el borde: en la primera
 * columna la ventana se llena de copias de esa misma columna, así que conserva
 * la mitad de su valor original. Medido, con texto de terminal contra el borde
 * de la zona quedaba contraste de sobra para leerlo. Traer píxeles de al lado no
 * filtra nada: lo de afuera se sigue viendo igual, no se tocó.
 */
export function difuminar(mapa: Mapa, rect: Rect, radio: number): void {
	const zona = acotar(mapa, rect);
	if (!zona || radio < 1) return;
	for (let pasada = 0; pasada < 4; pasada++) {
		caja(mapa, zona, radio, pasada % 2 === 0);
	}
}

/** Una pasada de caja, horizontal o vertical, con ventana corrediza. */
function caja(mapa: Mapa, zona: Rect, radio: number, horizontal: boolean): void {
	const largo = horizontal ? zona.ancho : zona.alto;
	const lineas = horizontal ? zona.alto : zona.ancho;
	const ventana = radio * 2 + 1;
	/** El tamaño del mapa sobre el eje que se recorre. */
	const limite = horizontal ? mapa.ancho : mapa.alto;
	/** Dónde empieza la zona sobre ese eje. */
	const desde = horizontal ? zona.x : zona.y;

	// El índice del píxel `n` de la línea `l`, acotado al **mapa**: la ventana
	// puede asomarse fuera de la zona, y ahí está la mitad de la gracia.
	const indice = (l: number, n: number) => {
		const acotado = Math.min(Math.max(desde + n, 0), limite - 1);
		const x = horizontal ? acotado : zona.x + l;
		const y = horizontal ? zona.y + l : acotado;
		return (y * mapa.ancho + x) * 4;
	};

	const linea = new Float32Array(largo * 4);

	for (let l = 0; l < lineas; l++) {
		const suma = [0, 0, 0, 0];
		for (let k = -radio; k <= radio; k++) {
			const i = indice(l, k);
			for (let c = 0; c < 4; c++) suma[c] += mapa.datos[i + c];
		}
		for (let n = 0; n < largo; n++) {
			for (let c = 0; c < 4; c++) linea[n * 4 + c] = suma[c] / ventana;
			const sale = indice(l, n - radio);
			const entra = indice(l, n + radio + 1);
			for (let c = 0; c < 4; c++) suma[c] += mapa.datos[entra + c] - mapa.datos[sale + c];
		}
		for (let n = 0; n < largo; n++) {
			const i = indice(l, n);
			for (let c = 0; c < 4; c++) mapa.datos[i + c] = linea[n * 4 + c];
		}
	}
}
