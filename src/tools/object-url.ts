/**
 * Una URL de objeto que se suelta sola si el trabajo no se la queda.
 *
 * Lo que devuelve `URL.createObjectURL` no tiene dueño: vive mientras viva el
 * documento aunque nadie la mire, y una captura de todas las pantallas son
 * varios megabytes. El único momento en el que se sabe si hay que seguir
 * teniéndola es justo después de usarla, y ahí hay tres salidas —se adoptó, no
 * se adoptó, o el trabajo falló— de las que dos tienen que revocarla. Escribirlas
 * a mano en cada sitio es exactamente cómo se pierde una: la que se olvida es
 * siempre la del camino que no se prueba.
 *
 * El trabajo contesta si se la queda. Si contesta que no —o si falla— la URL se
 * revoca antes de volver, y lo que falló sigue viaje.
 */
export async function withTempObjectUrl(
	blob: Blob,
	work: (url: string) => Promise<boolean>
): Promise<string | null> {
	const url = URL.createObjectURL(blob);
	let adopted = false;
	try {
		adopted = await work(url);
	} finally {
		if (!adopted) URL.revokeObjectURL(url);
	}
	return adopted ? url : null;
}
