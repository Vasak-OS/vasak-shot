import { describe, expect, test } from 'bun:test';
import conf from '../src-tauri/tauri.conf.json';

/**
 * De dónde puede venir la imagen que se dibuja en el canvas.
 *
 * No es una preferencia de configuración: es la única regla que hace que anotar
 * una captura funcione. El canvas de anotación dibuja la captura adentro, y el
 * navegador **contamina** un canvas en cuanto entra una imagen de otro origen.
 * Un canvas contaminado no deja leer sus píxeles —difuminar y pixelar dejan de
 * tapar— ni exportarlos —la captura anotada no se guarda, no se copia y no se
 * sube—. Y no avisa: tira una excepción adentro del `watch` que compone, así que
 * lo que se ve es un dibujo que no aparece.
 *
 * Así llegó el fallo la primera vez: el fondo venía de `convertFileSrc`, que
 * devuelve una URL `asset://`, y eso es otro origen. Lo que lo arregla es traer
 * los bytes por el IPC y armar un `blob:`, que hereda el origen del documento.
 */
describe('el origen de la imagen del canvas', () => {
	const csp = conf.app.security.csp;
	const imgSrc = csp.split(';').find((d) => d.trim().startsWith('img-src')) ?? '';

	test('la política deja pasar los `blob:`, que es de donde sale el fondo', () => {
		expect(imgSrc).toContain('blob:');
	});

	test('y no `asset:`, que es el origen que contaminaba el canvas', () => {
		expect(imgSrc).not.toContain('asset:');
		expect(csp).not.toContain('asset.localhost');
	});

	/**
	 * Apagado, y no sólo sin usar.
	 *
	 * Mientras siga encendido, el WebView puede leer archivos del disco por esa
	 * vía y alcanza con un `convertFileSrc` en cualquier parte para que el fallo
	 * vuelva sin que nada falle a la vista.
	 */
	test('el protocolo de archivos está apagado', () => {
		expect(conf.app.security.assetProtocol.enable).toBe(false);
		expect(conf.app.security.assetProtocol.scope).toEqual([]);
	});
});
