/**
 * Lo que vino con el molde de la plantilla y nadie usa.
 *
 * `useReactiveIcon()` eran noventa y una líneas que **ningún archivo
 * importaba**. Lo que lo hace peligroso no es que ocupe lugar: es que está
 * disponible. Un composable muerto no se ve raro —se lee como una pieza de la
 * casa—, así que el primero que necesite un icono lo va a usar en vez de
 * `ThemeIcon`, que hace lo mismo con **un solo** oyente del cambio de tema para
 * toda la ventana contra uno por instancia.
 *
 * Así terminó habiendo una copia distinta en cada repositorio: al contarlas
 * quedaban nueve, y no eran nueve copias del mismo archivo sino cinco firmas
 * que ya no son intercambiables.
 *
 * Con él se fue `assets/vue.svg`, el logo de Vue que trae `create-vue`, que
 * tampoco nombraba nadie.
 *
 * `main.ts` es la excepción y es de fondo: el menú contextual del escritorio no
 * dibuja con Vue, pide una **función** que resuelva el nombre a una ruta porque
 * lo pinta el complemento fuera de esta ventana. `ThemeIcon` no sirve ahí.
 */

import { describe, expect, test } from 'bun:test';
import { Glob } from 'bun';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

// `fileURLToPath` y no `.pathname`: éste deja los caracteres codificados tal
// como están, así que un checkout en una ruta con un espacio llega con `%20` y
// `scanSync` no encuentra nada. Una guardia que no encuentra archivos pasa.
const raiz = fileURLToPath(new URL('../src/', import.meta.url));

const fuentes = await Promise.all(
	[...new Glob('**/*.{vue,ts,svg}').scanSync(raiz)].map(
		async (ruta) => [ruta, await Bun.file(join(raiz, ruta)).text()] as const
	)
);
const rutas = fuentes.map(([ruta]) => ruta);

describe('el molde', () => {
	test('hay algo que mirar', () => {
		// Sin esto, las de abajo pasan con la lista vacía, que es en lo que
		// quedan si el patrón deja de encontrar archivos. Una guardia que se
		// apaga sola dice que sí.
		expect(rutas).toContain('App.vue');
		expect(rutas.length).toBeGreaterThan(3);
	});

	test('no dejó el composable de iconos', () => {
		expect(rutas.filter((ruta) => ruta.includes('useReactiveIcon'))).toEqual([]);
	});

	test('ni el logo de Vue', () => {
		expect(rutas.filter((ruta) => ruta.endsWith('vue.svg'))).toEqual([]);
	});

	test('y nadie resuelve iconos del tema por su cuenta en un componente', () => {
		// La forma de la copia, no su nombre: pedirle la ruta al complemento y
		// volver a pedírsela cuando cambia el tema.
		const culpables = fuentes
			.filter(
				([ruta, texto]) =>
					ruta !== 'main.ts' && /getIconSource|getSymbolSource|vicons:theme-changed/.test(texto)
			)
			.map(([ruta]) => ruta);

		expect(culpables).toEqual([]);
	});
});
