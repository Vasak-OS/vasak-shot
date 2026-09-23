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
	/**
	 * A dónde se suben las capturas, o nulo si no se suben a ningún lado.
	 *
	 * **Nulo es el valor de fábrica y no hay ninguno de reserva.** Subir es
	 * publicar: el enlace lo abre cualquiera que lo tenga, y la captura lleva
	 * encima lo que había en la pantalla. Sin dirección escrita a mano, el botón
	 * de subir ni siquiera aparece.
	 */
	subirA: string | null;
	/** El campo del formulario, si se eligió uno distinto del de siempre. */
	subirCampo: string | null;
	/** El servidor de esa dirección, que es lo que hay que preguntar antes. */
	subirServidor: string | null;
}

/**
 * Lo que el panel manda guardar: las preferencias enteras, no un pedazo.
 *
 * Enteras porque el backend escribe el archivo entero: mandar sólo lo que
 * cambió borraría lo demás. Y como objeto y no como cuatro argumentos en fila,
 * que es como se termina guardando la carpeta en el campo de la dirección.
 */
export interface Guardado {
	alSoltar: AlSoltar;
	carpeta: string | null;
	subirA: string | null;
	subirCampo: string | null;
}

/**
 * Las cuatro, en el orden en que se muestran.
 *
 * Arriba la de por omisión, que es lo que está pasando cuando alguien abre el
 * panel por primera vez: buscar en cuál de las cuatro está parado es lo primero
 * que hace quien viene a cambiarla. Las otras tres, de la que más hace a la que
 * menos.
 */
export const ACCIONES: AlSoltar[] = ['esperar', 'guardar-y-copiar', 'guardar', 'copiar'];

/**
 * Mientras el backend no conteste.
 *
 * `esperar` es además lo más prudente para el hueco entre que la ventana abre y
 * que las preferencias llegan: no entrega nada. Si la preferencia guardada dice
 * otra cosa, lo que se pierde es que el primer arrastre no entregue solo; al
 * revés —entregar mientras no se sabe— se pierde un archivo que nadie pidió.
 */
export const POR_OMISION: Ajustes = {
	alSoltar: 'esperar',
	carpeta: null,
	carpetaEfectiva: '',
	subirA: null,
	subirCampo: null,
	subirServidor: null,
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
