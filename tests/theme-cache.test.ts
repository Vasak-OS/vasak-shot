import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { applyTheme, saveTheme, THEME_KEY } from '../src/tools/theme-cache';

/** Un elemento raíz mínimo: la clase `dark` y un `style` con variables, sin DOM. */
function fakeRoot(dark = false, vars: Record<string, string> = {}) {
	const props = new Map(Object.entries(vars));
	const classes = new Set(dark ? ['dark'] : []);
	const style = {
		get length() {
			return props.size;
		},
		[Symbol.iterator]: () => props.keys(),
		getPropertyValue: (name: string) => props.get(name) ?? '',
		setProperty: (name: string, value: string) => void props.set(name, value),
	};
	const root = {
		style,
		classList: { contains: (c: string) => classes.has(c), add: (c: string) => classes.add(c) },
	};
	return { root: root as unknown as HTMLElement, props, classes };
}

/** Un `Storage` en memoria. */
function fakeStorage(initial: Record<string, string> = {}) {
	const data = new Map(Object.entries(initial));
	return {
		getItem: (k: string) => data.get(k) ?? null,
		setItem: (k: string, v: string) => void data.set(k, v),
	} as unknown as Storage;
}

const throwingStorage = {
	getItem() {
		throw new Error('denegado');
	},
	setItem() {
		throw new Error('denegado');
	},
} as unknown as Storage;

describe('tema recordado', () => {
	test('la clave es la que lee el script del head', () => {
		expect(THEME_KEY).toBe('vsk-theme');
	});

	test('saveTheme guarda la clase oscura y sólo las variables --*', () => {
		const { root } = fakeRoot(true, { '--ui-background': '#111', color: 'red', '--primary': '#09f' });
		const storage = fakeStorage();
		saveTheme(root, storage);
		expect(JSON.parse(storage.getItem(THEME_KEY) as string)).toEqual({
			dark: true,
			vars: { '--ui-background': '#111', '--primary': '#09f' },
		});
	});

	test('applyTheme reaplica la clase y las variables recordadas', () => {
		const storage = fakeStorage({
			[THEME_KEY]: JSON.stringify({ dark: true, vars: { '--primary': '#09f' } }),
		});
		const { root, props, classes } = fakeRoot();
		applyTheme(root, storage);
		expect(classes.has('dark')).toBe(true);
		expect(props.get('--primary')).toBe('#09f');
	});

	test('ida y vuelta: lo guardado de una raíz se reaplica en otra', () => {
		const storage = fakeStorage();
		saveTheme(fakeRoot(true, { '--ui-surface': '#222' }).root, storage);
		const destino = fakeRoot();
		applyTheme(destino.root, storage);
		expect(destino.classes.has('dark')).toBe(true);
		expect(destino.props.get('--ui-surface')).toBe('#222');
	});

	test('sin clave no toca nada', () => {
		const { root, props, classes } = fakeRoot();
		applyTheme(root, fakeStorage());
		expect(classes.size).toBe(0);
		expect(props.size).toBe(0);
	});

	test('un JSON inválido no lanza ni toca nada', () => {
		const { root, classes } = fakeRoot();
		expect(() => applyTheme(root, fakeStorage({ [THEME_KEY]: '{roto' }))).not.toThrow();
		expect(classes.size).toBe(0);
	});

	test('un Storage que lanza no rompe el arranque', () => {
		const { root } = fakeRoot(true, { '--primary': '#09f' });
		expect(() => saveTheme(root, throwingStorage)).not.toThrow();
		expect(() => applyTheme(root, throwingStorage)).not.toThrow();
	});
});

describe('cableado del tema recordado', () => {
	const read = (path: string) => readFileSync(new URL(path, import.meta.url), 'utf8');

	test('index.html reaplica el tema antes de montar', () => {
		const html = read('../index.html');
		const script = html.indexOf('vsk-theme');
		expect(script).toBeGreaterThan(-1);
		expect(html).toContain('classList.add("dark")');
		expect(html).toContain('setProperty');
		// Tiene que correr antes del bundle que monta Vue.
		expect(script).toBeLessThan(html.indexOf('type="module"'));
	});

	test('main.ts guarda el tema tras cargar la configuración', () => {
		const main = read('../src/main.ts');
		expect(main).toMatch(/\.loadConfig\(\)\s*\.then\(\(\) => saveTheme\(\)\)/);
	});
});
