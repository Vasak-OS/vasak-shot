/**
 * Qué hace soltar el botón, y dónde va la captura.
 *
 * Acá vive sólo la traducción de «preferencia» a «comando»: leer, normalizar y
 * guardar el archivo es cosa de Rust, que es el que lo tiene. Dos lugares
 * interpretando el mismo JSON es la forma más rápida de que uno de los dos se
 * equivoque en silencio.
 */

/** Qué se entrega al soltar el botón del ratón. */
export type AlSoltar = 'guardar-y-copiar' | 'guardar' | 'copiar' | 'esperar';

/** Los comandos que el backend sabe atender con una región. */
export type Comando = 'guardar' | 'copiar' | 'guardar_y_copiar';

/** Las preferencias, más la carpeta que de verdad se está usando. */
export interface Ajustes {
	alSoltar: AlSoltar;
	/** La carpeta elegida a mano, o nula si no se eligió ninguna. */
	carpeta: string | null;
	/** Dónde van las capturas ahora mismo, ya resuelto. */
	carpetaEfectiva: string;
}

/**
 * Las cuatro, en el orden en que se muestran.
 *
 * De la que más hace a la que menos: quien abre el panel por primera vez lee de
 * arriba hacia abajo, y arriba tiene que estar lo que ya venía pasando.
 */
export const ACCIONES: AlSoltar[] = ['guardar-y-copiar', 'guardar', 'copiar', 'esperar'];

/**
 * Mientras el backend no conteste.
 *
 * Es lo que la herramienta hizo siempre, así que una respuesta que tarda no
 * cambia el comportamiento — sólo lo retrasa.
 */
export const POR_OMISION: Ajustes = {
	alSoltar: 'guardar-y-copiar',
	carpeta: null,
	carpetaEfectiva: '',
};

/**
 * Qué comando dispara soltar el botón, o `null` si no dispara ninguno.
 *
 * `esperar` devuelve `null` y eso no es un caso raro: es el valor que van a
 * necesitar anotar y ajustar la selección, que pasan **después** de soltar y no
 * existen si soltar ya entregó.
 */
export function comandoAlSoltar(alSoltar: AlSoltar): Comando | null {
	switch (alSoltar) {
		case 'guardar-y-copiar':
			return 'guardar_y_copiar';
		case 'guardar':
			return 'guardar';
		case 'copiar':
			return 'copiar';
		case 'esperar':
			return null;
	}
}

/**
 * La clave del catálogo que describe cada acción.
 *
 * Llana y no anidada —`shot.alSoltarCopiar`, no `shot.alSoltar.copiar`— porque
 * el catálogo tiene **dos** niveles: grupo y clave. Un tercero parsea igual y
 * deja el texto vacío, que es un hueco en la interfaz en lugar de un error.
 */
export function claveDe(alSoltar: AlSoltar): string {
	const pascal = alSoltar
		.split('-')
		.map((parte) => parte.charAt(0).toUpperCase() + parte.slice(1))
		.join('');
	return `shot.alSoltar${pascal}`;
}
