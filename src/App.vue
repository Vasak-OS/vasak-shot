<script setup lang="ts">
/**
 * La superficie de selección.
 *
 * Muestra el cuadro **ya capturado** —congelado— y deja elegir una región encima.
 * Que la imagen esté quieta no es un detalle estético: la selección se hace sobre
 * lo que la persona vio al apretar la tecla, no sobre una pantalla que sigue
 * cambiando debajo mientras arrastra.
 */
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useI18n } from '@vasakgroup/tauri-plugin-i18n';
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { interpolar } from '@/tools/interpolar';
import { medidasDe, type Region, aEntregar as regionAEntregar, regionEntre } from '@/tools/region';

interface Salida {
	x: number;
	y: number;
	ancho: number;
	alto: number;
}

interface Lienzo {
	ruta: string;
	/** El tamaño de la captura entera, con todas las salidas. */
	ancho: number;
	alto: number;
	/** La salida que esta ventana está tapando, en unidades del layout. */
	salida: Salida;
	/** Píxeles de la captura por unidad del layout, por eje. */
	escalaX: number;
	escalaY: number;
}

const { t } = useI18n();

const lienzo = ref<Lienzo | null>(null);
const fondo = ref('');
const error = ref('');
const trabajando = ref(false);
const aviso = ref('');

/** Dónde empezó el arrastre, y dónde está ahora. Nulo si no se está arrastrando. */
const desde = ref<{ x: number; y: number } | null>(null);
const hasta = ref<{ x: number; y: number } | null>(null);

/**
 * La región elegida, en píxeles de la imagen.
 *
 * Se normaliza acá además de en Rust: el rectángulo que se dibuja tiene que
 * seguir al puntero en cualquier dirección, y con medidas negativas el CSS no
 * dibuja nada. La normalización de Rust es la que protege el recorte; esta, lo
 * que se ve.
 */
const region = computed<Region | null>(() => regionEntre(desde.value, hasta.value));

/**
 * La región que se va a entregar: la elegida, o toda **esta** pantalla si no hay.
 *
 * Esta pantalla y no la captura entera. Las coordenadas que se mandan son las de
 * la ventana, y la ventana cubre una sola salida; entregar el tamaño de la
 * composición pediría un rectángulo que no existe acá. Rust lo traduce después.
 */
const aEntregar = computed<Region | null>(() =>
	regionAEntregar(region.value, lienzo.value?.salida ?? null)
);

const medidas = computed(() => medidasDe(aEntregar.value));

function empezar(evento: MouseEvent) {
	if ((evento.target as HTMLElement).closest('button')) return;
	desde.value = { x: evento.clientX, y: evento.clientY };
	hasta.value = { x: evento.clientX, y: evento.clientY };
}

function mover(evento: MouseEvent) {
	if (!desde.value) return;
	hasta.value = { x: evento.clientX, y: evento.clientY };
}

function terminar() {
	// El arrastre queda hecho; no se limpia `desde` porque la región elegida
	// tiene que seguir visible para poder confirmarla.
	if (region.value === null) {
		desde.value = null;
		hasta.value = null;
	}
}

async function salir() {
	await getCurrentWindow().close();
}

async function entregar(comando: 'guardar' | 'copiar' | 'guardar_y_copiar') {
	const r = aEntregar.value;
	if (!r || trabajando.value) return;
	trabajando.value = true;
	error.value = '';
	try {
		const ruta = await invoke<string | null>(comando, { region: r });
		aviso.value =
			comando === 'copiar' ? t('shot.copiada') : interpolar(t('shot.guardadaEn'), ruta ?? '');
		// Se cierra sola: la captura ya está donde tenía que estar, y dejar la
		// ventana abierta obligaría a un paso más para nada.
		setTimeout(() => void salir(), 450);
	} catch (e) {
		error.value = String(e);
		trabajando.value = false;
	}
}

function alTeclado(evento: KeyboardEvent) {
	if (evento.key === 'Escape') {
		void salir();
	} else if (evento.key === 'Enter') {
		void entregar('guardar_y_copiar');
	} else if (evento.key === 'c' && evento.ctrlKey) {
		void entregar('copiar');
	}
}

onMounted(async () => {
	window.addEventListener('keydown', alTeclado);
	try {
		const l = await invoke<Lienzo>('lienzo');
		lienzo.value = l;
		// `convertFileSrc` y no `file://`: la política de contenido no permite
		// rutas absolutas de archivo, y está bien que no lo haga. Requiere que
		// `assetProtocol` esté habilitado en `tauri.conf.json` — sin eso la URL
		// queda bloqueada.
		const url = convertFileSrc(l.ruta);

		// Se comprueba que cargue **antes** de usarla como fondo.
		//
		// Sin esto, una imagen bloqueada dejaba la ventana transparente sobre el
		// escritorio vivo, y eso se ve casi igual que el cuadro congelado: la
		// selección parecía funcionar mientras en realidad se estaba eligiendo
		// sobre una pantalla que seguía moviéndose. Una falla que se disfraza de
		// funcionamiento es peor que una que se ve.
		await new Promise<void>((listo, falla) => {
			const prueba = new Image();
			prueba.onload = () => listo();
			prueba.onerror = () => falla(new Error(t('shot.errorImagen')));
			prueba.src = url;
		});
		fondo.value = url;
	} catch (e) {
		error.value = String(e);
	}
});

onUnmounted(() => window.removeEventListener('keydown', alTeclado));

/**
 * El fondo: **el pedazo de la captura que corresponde a esta pantalla**.
 *
 * Antes era `backgroundSize: 100% 100%`, o sea la composición entera estirada
 * dentro de una sola salida. Con dos monitores de 1920x1080 apilados, eso mostraba
 * los dos achatados a la mitad — y lo que se elegía no era lo que se recortaba.
 *
 * La cuenta tiene dos pasos. Primero la imagen se lleva a unidades del layout
 * dividiendo por la escala, así un píxel CSS es una unidad del layout. Después se
 * corre el origen hasta la esquina de esta salida, con posición negativa.
 */
const estiloFondo = computed(() => {
	const l = lienzo.value;
	if (!fondo.value || !l) return {};
	return {
		backgroundImage: `url(${fondo.value})`,
		backgroundSize: `${l.ancho / l.escalaX}px ${l.alto / l.escalaY}px`,
		backgroundPosition: `${-l.salida.x}px ${-l.salida.y}px`,
		backgroundRepeat: 'no-repeat',
	};
});

const estilo = computed(() => {
	const r = region.value;
	if (!r) return { display: 'none' };
	return {
		left: `${r.x}px`,
		top: `${r.y}px`,
		width: `${r.ancho}px`,
		height: `${r.alto}px`,
	};
});
</script>

<template>
	<main
		class="fixed inset-0 select-none overflow-hidden"
		:style="estiloFondo"
		@mousedown="empezar"
		@mousemove="mover"
		@mouseup="terminar"
	>
		<!-- El velo se apaga en cuanto hay una selección: con los dos, la zona
		     elegida quedaría oscurecida dos veces.
		     Negro y no un color del tema: no es una superficie de la interfaz
		     sino una atenuación sobre la captura, y tiene que oscurecer igual con
		     el tema claro. -->
		<div v-if="!region" class="absolute inset-0 bg-black/55"></div>

		<!-- El recorte de la selección se hace con una sombra enorme en lugar de
		     cuatro divs: así el borde queda pegado al rectángulo sin cuentas. -->
		<div
			v-if="region"
			class="absolute rounded-corner border border-primary shadow-[0_0_0_9999px_rgba(0,0,0,0.55)]"
			:style="estilo"
		>
			<span
				class="-top-7 absolute left-0 whitespace-nowrap rounded-corner bg-primary px-2 py-0.5 font-mono text-tx-on-primary text-xs"
			>
				{{ medidas }}
			</span>
		</div>

		<div
			v-if="error"
			class="absolute top-6 left-1/2 -translate-x-1/2 rounded-corner bg-status-error/90 px-4 py-2 text-sm text-tx-on-primary"
		>
			{{ error }}
		</div>

		<div
			v-if="aviso"
			class="absolute top-6 left-1/2 -translate-x-1/2 rounded-corner border border-ui-border bg-ui-bg/80 px-4 py-2 text-sm text-tx-main"
		>
			{{ aviso }}
		</div>

		<div
			class="absolute bottom-8 left-1/2 flex -translate-x-1/2 items-center gap-1.5 rounded-corner border border-ui-border bg-ui-bg/80 p-1.5"
		>
			<span class="px-2 font-mono text-tx-muted text-xs">{{ medidas }}</span>
			<span class="h-5 w-px bg-ui-border"></span>
			<button
				type="button"
				class="flex items-center gap-1.5 rounded-corner px-3 py-1.5 text-sm text-tx-main hover:bg-ui-surface disabled:opacity-50"
				:disabled="trabajando"
				@click="entregar('copiar')"
			>
				{{ t('shot.copiar') }}
				<kbd class="font-mono text-[10px] text-tx-muted">{{ t('shot.teclaCopiar') }}</kbd>
			</button>
			<button
				type="button"
				class="flex items-center gap-1.5 rounded-corner bg-primary px-3 py-1.5 font-medium text-sm text-tx-on-primary hover:brightness-110 disabled:opacity-50"
				:disabled="trabajando"
				@click="entregar('guardar_y_copiar')"
			>
				{{ t('shot.guardar') }}
				<kbd class="font-mono text-[10px] text-tx-on-primary/70">{{ t('shot.teclaGuardar') }}</kbd>
			</button>
			<span class="h-5 w-px bg-ui-border"></span>
			<button
				type="button"
				class="rounded-corner px-3 py-1.5 text-sm text-tx-muted hover:bg-ui-surface"
				@click="salir()"
			>
				{{ t('shot.cancelar') }}
				<kbd class="ml-1 font-mono text-[10px] text-tx-muted">{{ t('shot.teclaCancelar') }}</kbd>
			</button>
		</div>

		<!-- Con fondo propio, no suelto sobre la imagen: el cuadro congelado puede
		     ser cualquier cosa —una terminal llena de texto, una foto clara— y sin
		     un respaldo la instrucción quedaba ilegible justo cuando más se
		     necesita, que es la primera vez que alguien abre esto. -->
		<p
			v-if="!region"
			class="-translate-x-1/2 -translate-y-1/2 absolute top-1/2 left-1/2 rounded-corner border border-ui-border bg-ui-bg/80 px-4 py-2 text-sm text-tx-main"
		>
			{{ t('shot.instruccion') }}
		</p>
	</main>
</template>
