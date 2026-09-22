import { describe, expect, test } from 'bun:test';
import { RETARDOS, TECHO, vuelveACapturar } from '@/tools/retardo';

describe('vuelveACapturar', () => {
	test('el cero no vuelve a lanzar nada', () => {
		// Volver a lanzar sin esperar capturaría el selector que todavía se está
		// cerrando: una foto de la propia herramienta.
		expect(vuelveACapturar(0)).toBe(false);
	});

	test('los que se ofrecen, salvo el cero, capturan de nuevo', () => {
		// Si esto se separa, la barra ofrece un botón que no hace nada.
		for (const segundos of RETARDOS.filter((s) => s > 0)) {
			expect(vuelveACapturar(segundos)).toBe(true);
		}
	});

	test('un retardo absurdo no se pide, aunque llegue de algún lado', () => {
		// El que protege de verdad es Rust, que ve también la línea de órdenes.
		// Éste es para no pedir lo que el otro va a rechazar.
		expect(vuelveACapturar(TECHO)).toBe(true);
		expect(vuelveACapturar(TECHO + 1)).toBe(false);
		expect(vuelveACapturar(-3)).toBe(false);
		expect(vuelveACapturar(2.5)).toBe(false);
		expect(vuelveACapturar(Number.NaN)).toBe(false);
	});

	test('el cero está en la lista, porque es lo que está pasando', () => {
		// Una lista de retardos donde no figura el actual obliga a adivinar cuál
		// es.
		expect(RETARDOS[0]).toBe(0);
	});
});
