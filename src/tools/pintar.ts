/**
 * Dibuja las anotaciones sobre un lienzo de canvas.
 *
 * **Es el único dibujante.** La vista previa y el archivo que se guarda salen de
 * llamar a esto sobre el mismo canvas, así que no pueden diferir: lo que se ve
 * es, literalmente, el mapa de bits que se entrega. Esa es la razón por la que
 * la composición final dejó de hacerse en Rust — dos dibujantes distintos no dan
 * el mismo borde ni la misma letra, y tapar una zona exige que coincidan.
 *
 * Las anotaciones vienen en coordenadas del selector; el canvas está en píxeles
 * de la captura. El encuadre es lo que traduce entre las dos cosas, y va por
 * parámetro en vez de por `ctx.scale` porque los efectos leen píxeles crudos,
 * que no pasan por la transformación del contexto.
 */

import { type Anotacion, esDeUnPunto, esRectangular, esTrazo, tapa } from '@/tools/anotacion';
import { difuminar, type Mapa, pixelar } from '@/tools/efectos';
import type { Punto } from '@/tools/region';

export interface Encuadre {
	/** La esquina de la región elegida, en coordenadas del selector. */
	origen: Punto;
	escalaX: number;
	escalaY: number;
}

/** Cuánto se ve el resaltador. Translúcido, o taparía lo que resalta. */
const ALFA_RESALTADOR = 0.35;

/** El cuerpo de la flecha, sin la punta. */
const PUNTA_MINIMA = 14;

/** De coordenadas del selector a píxeles del canvas. */
function enElLienzo(punto: Punto, encuadre: Encuadre): Punto {
	return {
		x: (punto.x - encuadre.origen.x) * encuadre.escalaX,
		y: (punto.y - encuadre.origen.y) * encuadre.escalaY,
	};
}

/** El grosor en píxeles del canvas. Un promedio: los ejes casi nunca difieren. */
function grosorEnElLienzo(grosor: number, encuadre: Encuadre): number {
	return Math.max(1, (grosor * (encuadre.escalaX + encuadre.escalaY)) / 2);
}

/** El rectángulo entre dos puntos, ya en el lienzo y normalizado. */
function rectangulo(anotacion: Anotacion, encuadre: Encuadre) {
	const a = enElLienzo(anotacion.puntos[0], encuadre);
	const b = enElLienzo(anotacion.puntos[1], encuadre);
	return {
		x: Math.min(a.x, b.x),
		y: Math.min(a.y, b.y),
		ancho: Math.abs(b.x - a.x),
		alto: Math.abs(b.y - a.y),
	};
}

/** El tamaño del texto y del número de un paso, a partir del grosor. */
export function tamanioDeTexto(grosor: number): number {
	return grosor * 5 + 10;
}

/**
 * Pinta la lista entera, **en orden**.
 *
 * El orden no es un detalle de implementación: una flecha dibujada antes de un
 * difuminado que la cruza queda difuminada, y dibujada después queda nítida
 * encima. Componer los efectos por un lado y las formas por el otro sería más
 * fácil y daría el resultado equivocado.
 */
export function pintar(
	ctx: CanvasRenderingContext2D,
	anotaciones: Anotacion[],
	encuadre: Encuadre
): void {
	for (const anotacion of anotaciones) {
		if (anotacion.puntos.length === 0) continue;
		if (tapa(anotacion.tipo)) {
			if (anotacion.puntos.length >= 2) aplicarEfecto(ctx, anotacion, encuadre);
			continue;
		}
		ctx.save();
		dibujar(ctx, anotacion, encuadre);
		ctx.restore();
	}
}

function dibujar(ctx: CanvasRenderingContext2D, a: Anotacion, encuadre: Encuadre): void {
	ctx.strokeStyle = a.color;
	ctx.fillStyle = a.color;
	ctx.lineWidth = grosorEnElLienzo(a.grosor, encuadre);
	ctx.lineCap = 'round';
	ctx.lineJoin = 'round';

	if (esTrazo(a.tipo)) {
		trazo(ctx, a, encuadre);
		return;
	}
	if (esDeUnPunto(a.tipo)) {
		deUnPunto(ctx, a, encuadre);
		return;
	}
	if (!esRectangular(a.tipo) || a.puntos.length < 2) return;

	if (a.tipo === 'recuadro') {
		const r = rectangulo(a, encuadre);
		if (a.relleno) ctx.fillRect(r.x, r.y, r.ancho, r.alto);
		else ctx.strokeRect(r.x, r.y, r.ancho, r.alto);
		return;
	}

	if (a.tipo === 'elipse') {
		const r = rectangulo(a, encuadre);
		ctx.beginPath();
		ctx.ellipse(r.x + r.ancho / 2, r.y + r.alto / 2, r.ancho / 2, r.alto / 2, 0, 0, Math.PI * 2);
		if (a.relleno) ctx.fill();
		else ctx.stroke();
		return;
	}

	const desde = enElLienzo(a.puntos[0], encuadre);
	const hasta = enElLienzo(a.puntos[1], encuadre);
	ctx.beginPath();
	ctx.moveTo(desde.x, desde.y);
	ctx.lineTo(hasta.x, hasta.y);
	ctx.stroke();

	if (a.tipo === 'flecha') punta(ctx, desde, hasta, ctx.lineWidth);
}

/** La punta de la flecha, en el extremo al que se soltó. */
function punta(ctx: CanvasRenderingContext2D, desde: Punto, hasta: Punto, grosor: number): void {
	const angulo = Math.atan2(hasta.y - desde.y, hasta.x - desde.x);
	const largo = Math.max(PUNTA_MINIMA, grosor * 4);
	const abre = Math.PI / 7;

	// Rellena y no dos líneas: con el trazo redondeado, dos líneas dejan una
	// punta roma que a grosor chico no se lee como flecha.
	ctx.beginPath();
	ctx.moveTo(hasta.x, hasta.y);
	ctx.lineTo(hasta.x - largo * Math.cos(angulo - abre), hasta.y - largo * Math.sin(angulo - abre));
	ctx.lineTo(hasta.x - largo * Math.cos(angulo + abre), hasta.y - largo * Math.sin(angulo + abre));
	ctx.closePath();
	ctx.fill();
}

/**
 * Un trazo a mano alzada, o un resaltado.
 *
 * Un solo camino y una sola pasada: el resaltador es translúcido, y trazo por
 * trazo cada superposición se oscurecería de nuevo hasta volverse opaca justo
 * donde la mano fue más lenta.
 */
function trazo(ctx: CanvasRenderingContext2D, a: Anotacion, encuadre: Encuadre): void {
	// Translúcido, pero del grosor que dice la barra: lo que lo hace un
	// resaltador y no un lápiz es con qué grosor se estrena, no un factor
	// escondido que hace que el número de la barra signifique otra cosa.
	if (a.tipo === 'resaltador') ctx.globalAlpha = ALFA_RESALTADOR;
	ctx.beginPath();
	const primero = enElLienzo(a.puntos[0], encuadre);
	ctx.moveTo(primero.x, primero.y);
	for (const punto of a.puntos.slice(1)) {
		const p = enElLienzo(punto, encuadre);
		ctx.lineTo(p.x, p.y);
	}
	// Un trazo de un solo punto no dibuja nada con `lineTo`: se cierra sobre sí
	// mismo para que un toque deje la marca redonda que la mano espera.
	if (a.puntos.length === 1) ctx.lineTo(primero.x + 0.01, primero.y);
	ctx.stroke();
}

/** El texto y los pasos numerados, que se ponen con un clic. */
function deUnPunto(ctx: CanvasRenderingContext2D, a: Anotacion, encuadre: Encuadre): void {
	const p = enElLienzo(a.puntos[0], encuadre);
	const tamanio = tamanioDeTexto(a.grosor) * ((encuadre.escalaX + encuadre.escalaY) / 2);
	ctx.font = `${a.tipo === 'paso' ? 'bold ' : ''}${tamanio}px sans-serif`;
	ctx.textBaseline = a.tipo === 'paso' ? 'middle' : 'top';
	ctx.textAlign = a.tipo === 'paso' ? 'center' : 'left';

	if (a.tipo === 'texto') {
		ctx.fillText(a.texto ?? '', p.x, p.y);
		return;
	}

	const radio = tamanio * 0.8;
	ctx.beginPath();
	ctx.arc(p.x, p.y, radio, 0, Math.PI * 2);
	ctx.fill();
	// El número en blanco sobre el color elegido: el círculo puede ser de
	// cualquier color y el número tiene que leerse igual.
	ctx.fillStyle = '#ffffff';
	ctx.fillText(String(a.numero ?? 1), p.x, p.y);
}

/**
 * Difuminar o pixelar una zona.
 *
 * Los píxeles se sacan **con un margen** alrededor de la zona y se devuelven
 * sólo los de adentro. El difuminado que sólo puede mirar adentro deja legible
 * justo el borde —la ventana se llena de copias de la primera columna—, y el
 * mosaico alineado al rectángulo en lugar de a la grilla del lienzo parpadea
 * entero mientras se arrastra la zona.
 */
function aplicarEfecto(ctx: CanvasRenderingContext2D, a: Anotacion, encuadre: Encuadre): void {
	const r = rectangulo(a, encuadre);
	if (r.ancho < 1 || r.alto < 1) return;

	// **En píxeles del lienzo, no del selector.** El grosor se elige mirando la
	// pantalla y el efecto trabaja sobre la captura, que en una pantalla HiDPI
	// tiene el doble: sin convertirlo, lo que tapa queda a la mitad de lo que se
	// pidió — y en algo que tapa, quedarse corto es el error que importa. Es la
	// misma conversión escalar que el grosor de un trazo; con ejes de escala
	// distinta ninguna de las dos es exacta, y esa es la que ya se eligió.
	const intensidad = Math.round(grosorEnElLienzo(a.grosor, encuadre));
	const lienzo = ctx.canvas;
	const margen = a.tipo === 'difuminar' ? intensidad : 0;
	const alinear = (valor: number) =>
		a.tipo === 'pixelar' ? Math.floor(valor / intensidad) * intensidad : valor - margen;

	const x = Math.max(0, Math.floor(alinear(r.x)));
	const y = Math.max(0, Math.floor(alinear(r.y)));
	const derecha = Math.min(lienzo.width, Math.ceil(r.x + r.ancho) + margen);
	const abajo = Math.min(lienzo.height, Math.ceil(r.y + r.alto) + margen);
	if (derecha <= x || abajo <= y) return;

	const datos = ctx.getImageData(x, y, derecha - x, abajo - y);
	const mapa: Mapa = { datos: datos.data, ancho: datos.width, alto: datos.height };
	const adentro = {
		x: r.x - x,
		y: r.y - y,
		ancho: r.ancho,
		alto: r.alto,
	};

	if (a.tipo === 'pixelar') pixelar(mapa, adentro, intensidad);
	else difuminar(mapa, adentro, intensidad);

	// Sólo se devuelve lo de adentro: el margen se tomó para poder mirar, no
	// para taparlo.
	ctx.putImageData(datos, x, y, adentro.x, adentro.y, adentro.ancho, adentro.alto);
}
