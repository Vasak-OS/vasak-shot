import { describe, expect, test } from 'bun:test';
import { withTempObjectUrl } from '../src/tools/object-url';

/** Si sigue viva se la puede leer; si se revocó, resolverla falla. */
async function sigueViva(url: string): Promise<boolean> {
	try {
		await fetch(url);
		return true;
	} catch {
		return false;
	}
}

const blob = () => new Blob([new Uint8Array([1, 2, 3])], { type: 'image/png' });

describe('la URL de objeto que se suelta sola', () => {
	test('la que el trabajo se queda sigue viva, y es la que vuelve', async () => {
		let vista = '';
		const url = await withTempObjectUrl(blob(), async (u) => {
			vista = u;
			return true;
		});

		expect(url).toBe(vista);
		expect(await sigueViva(vista)).toBe(true);
		URL.revokeObjectURL(vista);
	});

	test('la que el trabajo no se queda se revoca, y vuelve nada', async () => {
		let vista = '';
		const url = await withTempObjectUrl(blob(), async (u) => {
			vista = u;
			return false;
		});

		expect(url).toBeNull();
		expect(await sigueViva(vista)).toBe(false);
	});

	test('si el trabajo falla se revoca igual, y el fallo sigue viaje', async () => {
		let vista = '';
		const intento = withTempObjectUrl(blob(), async (u) => {
			vista = u;
			throw new Error('la imagen no cargó');
		});

		await expect(intento).rejects.toThrow('la imagen no cargó');
		await intento.catch(() => {});
		expect(await sigueViva(vista)).toBe(false);
	});
});
