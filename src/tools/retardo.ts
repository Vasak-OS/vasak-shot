/**
 * El retardo antes de disparar.
 *
 * Sirve para capturar lo que se cierra al perder el foco —un menú abierto, un
 * desplegable— y por eso elegirlo **cierra el selector**: durante la espera no
 * puede haber nada de esta aplicación en pantalla. Quien captura de nuevo es
 * otro proceso, que arranca, espera y recién ahí toma los píxeles.
 */

/**
 * Los que se ofrecen, en segundos.
 *
 * Cuatro y no un campo donde escribir: entre apretar la tecla y sostener un
 * menú abierto no hay tiempo para teclear un número, y tres segundos alcanzan
 * para abrir un menú, cinco para navegarlo y diez para algo con varios pasos.
 * El cero está porque es lo que está pasando ahora, y una lista donde no figura
 * el valor actual obliga a adivinar cuál es.
 */
export const RETARDOS = [0, 3, 5, 10];

/**
 * El techo, el mismo que Rust rechaza.
 *
 * Repetido a propósito y no leído de allá: acá sólo apaga un botón, allá impide
 * que un `--retardo 1000` deje un proceso invisible esperando diecisiete
 * minutos. El que protege de verdad es el de Rust, que es el que ve todos los
 * caminos —la línea de órdenes también—; éste es para no ofrecer lo que el
 * otro va a rechazar.
 */
export const TECHO = 60;

/**
 * Si ese retardo dispara una captura nueva.
 *
 * El cero no: volver a lanzar sin esperar capturaría el selector que todavía se
 * está cerrando, o sea una foto de la propia herramienta.
 */
export function vuelveACapturar(segundos: number): boolean {
	return Number.isInteger(segundos) && segundos > 0 && segundos <= TECHO;
}
