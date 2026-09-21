import { describe, expect, test } from 'bun:test';
import { ACCIONES, type AlSoltar, claveDe, comandoAlSoltar } from '@/tools/preferencias';

describe('comandoAlSoltar', () => {
	test('cada acción dispara el comando que nombra', () => {
		expect(comandoAlSoltar('guardar-y-copiar')).toBe('guardar_y_copiar');
		expect(comandoAlSoltar('guardar')).toBe('guardar');
		expect(comandoAlSoltar('copiar')).toBe('copiar');
	});

	test('esperar no dispara nada', () => {
		// No es un caso raro ni un valor sin implementar: es lo que van a
		// necesitar anotar y ajustar la selección, que pasan después de soltar.
		expect(comandoAlSoltar('esperar')).toBeNull();
	});

	test('las cuatro acciones están cubiertas', () => {
		// Sumar una quinta sin decidir qué hace dejaría un `undefined` que el
		// componente trataría como «no entregues nada», o sea un botón que no
		// hace nada y no falla.
		for (const accion of ACCIONES) {
			const comando = comandoAlSoltar(accion);
			expect(comando === null || typeof comando === 'string').toBe(true);
		}
		expect(ACCIONES.length).toBe(4);
		expect(new Set(ACCIONES).size).toBe(4);
	});

	test('la primera es la que la herramienta hacía siempre', () => {
		// Quien abre el panel por primera vez tiene que encontrar arriba lo que
		// ya venía pasando, no una opción nueva.
		expect(ACCIONES[0]).toBe('guardar-y-copiar');
	});
});

describe('claveDe', () => {
	test('cada acción tiene su clave en el catálogo', () => {
		expect(claveDe('guardar-y-copiar')).toBe('shot.alSoltarGuardarYCopiar');
		expect(claveDe('guardar')).toBe('shot.alSoltarGuardar');
		expect(claveDe('esperar')).toBe('shot.alSoltarEsperar');
	});

	test('la clave tiene dos niveles, no tres', () => {
		// El catálogo es grupo y clave. Un tercer nivel parsea igual y deja el
		// texto vacío: un hueco en la interfaz en vez de un error.
		for (const accion of ACCIONES as AlSoltar[]) {
			expect(claveDe(accion).split('.')).toHaveLength(2);
			expect(claveDe(accion)).not.toContain('-');
		}
	});

	test('dos acciones no comparten clave', () => {
		const claves = new Set(ACCIONES.map(claveDe));
		expect(claves.size).toBe(ACCIONES.length);
	});
});
