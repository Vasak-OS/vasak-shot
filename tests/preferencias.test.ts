import { describe, expect, test } from 'bun:test';
import { ACCIONES, type AlSoltar, claveDe, comandoAlSoltar, POR_OMISION } from '@/tools/preferencias';

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

	test('la primera es la de por omisión', () => {
		// Quien abre el panel por primera vez tiene que encontrar arriba lo que
		// está pasando: buscar en cuál de las cuatro está parado es lo primero
		// que hace quien viene a cambiarla.
		expect(ACCIONES[0]).toBe(POR_OMISION.alSoltar);
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

describe('POR_OMISION', () => {
	test('no se sube a ningún lado hasta que alguien escriba adónde', () => {
		// Es la mitad de la función, y por eso está fijado de los dos lados: un
		// servicio puesto de fábrica convierte un botón mal apretado en una
		// publicación. Mientras esto sea nulo, el botón de subir no aparece.
		expect(POR_OMISION.subirA).toBeNull();
		expect(POR_OMISION.subirServidor).toBeNull();
	});

	test('mientras el backend no conteste no se entrega nada', () => {
		// La ventana abre de golpe y el gesto puede terminar antes que la
		// respuesta. Esperar deja la selección congelada; entregar sin saber qué
		// dice la preferencia escribe un archivo que nadie pidió.
		expect(POR_OMISION.alSoltar).toBe('esperar');
		expect(comandoAlSoltar(POR_OMISION.alSoltar)).toBeNull();
		expect(POR_OMISION.carpeta).toBeNull();
	});
});

