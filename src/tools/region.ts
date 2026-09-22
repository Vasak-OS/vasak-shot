/**
 * La región que se está eligiendo con el ratón.
 *
 * Vive aparte del componente para poder probarla: son cuentas chicas donde un
 * error no se ve —un rectángulo que no sigue al puntero, una selección de un
 * píxel que igual se guarda— y las tres se rompen por separado.
 *
 * La normalización está también en Rust, y no es duplicación: la de Rust protege
 * el recorte de medidas negativas, esta protege lo que se **dibuja**. Con ancho
 * negativo el CSS no dibuja nada, así que el rectángulo desaparecería al arrastrar
 * hacia arriba o hacia la izquierda.
 */

export interface Punto {
	x: number;
	y: number;
}

export interface Region {
	x: number;
	y: number;
	ancho: number;
	alto: number;
}

/**
 * Cuántos píxeles de lado tiene que tener una selección para contar.
 *
 * Un clic sin arrastrar produce un rectángulo de cero o un píxel. Guardar eso
 * deja un archivo que no sirve, y peor: parece que la herramienta funcionó.
 */
export const MINIMO = 2;

/**
 * La región entre dos puntos, o `null` si no alcanza el mínimo.
 *
 * Arrastrar desde cualquier esquina da el mismo rectángulo: se toma el mínimo de
 * cada eje como origen y la distancia absoluta como medida.
 */
export function regionEntre(desde: Punto | null, hasta: Punto | null): Region | null {
	if (!desde || !hasta) return null;

	const ancho = Math.abs(hasta.x - desde.x);
	const alto = Math.abs(hasta.y - desde.y);
	if (ancho < MINIMO || alto < MINIMO) return null;

	return {
		x: Math.min(desde.x, hasta.x),
		y: Math.min(desde.y, hasta.y),
		ancho,
		alto,
	};
}

/**
 * Qué se va a entregar: lo elegido, o toda la pantalla si no se eligió nada.
 *
 * Que sin selección se guarde la pantalla entera es deliberado: apretar Intro sin
 * arrastrar es la forma más rápida de capturar todo, y no tener que elegir «toda
 * la pantalla» primero ahorra el paso más común.
 */
export function aEntregar(
	elegida: Region | null,
	pantalla: { ancho: number; alto: number } | null
): Region | null {
	if (elegida) return elegida;
	if (!pantalla) return null;
	return { x: 0, y: 0, ancho: pantalla.ancho, alto: pantalla.alto };
}

/** Las medidas para mostrar, como las lee una persona. */
export function medidasDe(region: Region | null): string {
	return region ? `${region.ancho} × ${region.alto}` : '';
}

/** Un tirador: qué bordes mueve. */
export type Rol = 'tl' | 't' | 'tr' | 'r' | 'br' | 'b' | 'bl' | 'l';

/**
 * Los ocho, en el orden de las agujas del reloj desde arriba a la izquierda.
 *
 * Las letras son las iniciales en inglés —`top`, `bottom`, `left`, `right`—
 * porque son cuatro distintas y eso deja preguntar por una con `includes`: `tl`
 * mueve el de arriba y el de la izquierda sin que haga falta una tabla.
 */
export const ROLES: Rol[] = ['tl', 't', 'tr', 'r', 'br', 'b', 'bl', 'l'];

/**
 * Si el punto cae adentro de la región.
 *
 * El borde de abajo y el de la derecha quedan afuera, igual que en las
 * ventanas: una región pegada a otra no puede contener las dos su línea de
 * contacto.
 */
export function contiene(region: Region, punto: Punto): boolean {
	return (
		punto.x >= region.x &&
		punto.x < region.x + region.ancho &&
		punto.y >= region.y &&
		punto.y < region.y + region.alto
	);
}

/** Dónde cae un tirador dentro del rectángulo, de 0 a 1 por eje. */
export function anclaDe(rol: Rol): { x: number; y: number } {
	const x = rol.includes('l') ? 0 : rol.includes('r') ? 1 : 0.5;
	const y = rol.includes('t') ? 0 : rol.includes('b') ? 1 : 0.5;
	return { x, y };
}

/**
 * Mete una región adentro del lienzo, recortándola.
 *
 * Recortar y no correr: quien llama a esto está moviendo un borde, y correr el
 * rectángulo entero porque un borde se pasó movería también el opuesto, que la
 * persona no tocó.
 */
export function limitar(region: Region, lienzo: { ancho: number; alto: number }): Region {
	const izquierda = Math.min(Math.max(region.x, 0), lienzo.ancho);
	const arriba = Math.min(Math.max(region.y, 0), lienzo.alto);
	const derecha = Math.min(Math.max(region.x + region.ancho, 0), lienzo.ancho);
	const abajo = Math.min(Math.max(region.y + region.alto, 0), lienzo.alto);
	return {
		x: izquierda,
		y: arriba,
		ancho: derecha - izquierda,
		alto: abajo - arriba,
	};
}

/**
 * Mueve el o los bordes que nombra el rol, y normaliza.
 *
 * Normalizar acá no es un detalle: arrastrando el borde derecho más allá del
 * izquierdo, el ancho sale negativo y el CSS deja de dibujar. Con la
 * normalización el rectángulo se **da vuelta**, que es lo que hace cualquier
 * herramienta de selección y lo que la mano espera.
 */
export function ajustar(
	region: Region,
	rol: Rol,
	dx: number,
	dy: number,
	lienzo: { ancho: number; alto: number }
): Region {
	let izquierda = region.x;
	let arriba = region.y;
	let derecha = region.x + region.ancho;
	let abajo = region.y + region.alto;

	if (rol.includes('l')) izquierda += dx;
	if (rol.includes('r')) derecha += dx;
	if (rol.includes('t')) arriba += dy;
	if (rol.includes('b')) abajo += dy;

	return limitar(
		{
			x: Math.min(izquierda, derecha),
			y: Math.min(arriba, abajo),
			ancho: Math.abs(derecha - izquierda),
			alto: Math.abs(abajo - arriba),
		},
		lienzo
	);
}

/**
 * El rectángulo que resulta de llevar el borde de `rol` hasta `punto`.
 *
 * `region` es la de **cuando se agarró el tirador**, no la de recién, y el
 * punto es el absoluto del puntero. Por eso y no por diferencias sucesivas,
 * que es como estaba: acumular deltas tiene dos problemas que no se ven hasta
 * que pasan.
 *
 * Uno, que al cruzar el borde opuesto la región se da vuelta pero el rol sigue
 * nombrando el borde de antes, así que el siguiente movimiento agarra el que
 * ahora está del otro lado y el puntero deja de arrastrar nada.
 *
 * Y dos, que un delta que el límite del lienzo recortó se pierde: empujar
 * contra el borde y volver no deja la región donde estaba.
 *
 * Con el ancla fija las dos desaparecen solas — el borde arrastrado está
 * siempre exactamente donde está el puntero.
 */
export function redimensionar(
	region: Region,
	rol: Rol,
	punto: Punto,
	lienzo: { ancho: number; alto: number }
): Region {
	const izquierda = rol.includes('l') ? punto.x : region.x;
	const derecha = rol.includes('r') ? punto.x : region.x + region.ancho;
	const arriba = rol.includes('t') ? punto.y : region.y;
	const abajo = rol.includes('b') ? punto.y : region.y + region.alto;

	return limitar(
		{
			x: Math.min(izquierda, derecha),
			y: Math.min(arriba, abajo),
			ancho: Math.abs(derecha - izquierda),
			alto: Math.abs(abajo - arriba),
		},
		lienzo
	);
}

/**
 * Corre la región entera sin cambiarle el tamaño.
 *
 * Contra el borde se **detiene**, no se achica: lo que se está haciendo es
 * mover, y una región que pierde ancho al llegar al borde deja de ser la que se
 * eligió. Si no entra en el lienzo —no debería, pero un lienzo más chico que la
 * selección es lo que pasa si algo cambió debajo— queda pegada al origen.
 */
export function correr(
	region: Region,
	dx: number,
	dy: number,
	lienzo: { ancho: number; alto: number }
): Region {
	const tope = (valor: number, medida: number, limite: number) =>
		Math.max(0, Math.min(valor, limite - medida));
	return {
		...region,
		x: tope(region.x + dx, region.ancho, lienzo.ancho),
		y: tope(region.y + dy, region.alto, lienzo.alto),
	};
}
