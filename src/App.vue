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
import PanelPreferencias from '@/components/PanelPreferencias.vue';
import { interpolar } from '@/tools/interpolar';
import {
	type Ajustes,
	type AlSoltar,
	type Comando,
	comandoAlSoltar,
	POR_OMISION,
} from '@/tools/preferencias';
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

/** Dónde empezó el arrastre, y dónde está ahora. Nulo si no hay región elegida. */
const desde = ref<{ x: number; y: number } | null>(null);
const hasta = ref<{ x: number; y: number } | null>(null);

/**
 * Si el botón está apretado ahora mismo.
 *
 * Aparte de `desde`, y ahí está el asunto: `desde` sigue con valor **después**
 * de soltar, porque la región elegida tiene que quedar dibujada. Mientras el
 * arrastre se dedujo de que `desde` existiera, soltar no terminaba nada y el
 * rectángulo seguía persiguiendo al puntero hasta que alguien volvía a apretar.
 */
const arrastrando = ref(false);

/** Las preferencias, con las de siempre mientras el backend no conteste. */
const ajustes = ref<Ajustes>(POR_OMISION);

/**
 * La lectura de las preferencias, mientras está en curso.
 *
 * Se guarda para poder esperarla al soltar. Sin eso, un arrastre que termina
 * antes de que el backend conteste se entrega con los valores de siempre —o
 * sea guardando— aunque la preferencia diga «copiar» o «esperar»: un archivo en
 * el disco que nadie pidió. La ventana aparece de golpe y el gesto puede
 * empezar en el primer cuadro.
 */
const cargando = ref<Promise<void> | null>(null);
const panel = ref(false);
const errorPanel = ref('');

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
	// La barra y el panel no son lienzo. Se marcan con `data-sin-arrastre` en
	// lugar de enumerar etiquetas: el panel tiene campos y etiquetas además de
	// botones, y una lista de etiquetas queda vieja en cuanto se agrega una.
	if ((evento.target as HTMLElement).closest('[data-sin-arrastre]')) return;
	arrastrando.value = true;
	desde.value = { x: evento.clientX, y: evento.clientY };
	hasta.value = { x: evento.clientX, y: evento.clientY };
}

function mover(evento: MouseEvent) {
	if (!arrastrando.value) return;
	hasta.value = { x: evento.clientX, y: evento.clientY };
}

/**
 * Soltar cierra la selección, y entrega.
 *
 * Las dos cosas, y en ese orden. Cerrarla es lo que hace que la región deje de
 * seguir al puntero —el arrastre queda hecho y `desde` no se limpia, porque lo
 * elegido tiene que seguir dibujado—. Entregar es el resto del gesto: se
 * arrastra sobre lo que se quiere y al levantar el dedo la captura ya está.
 *
 * Con `esperar` no entrega nada y quedan los botones, que es lo que van a
 * necesitar anotar y ajustar la selección.
 */
async function terminar() {
	if (!arrastrando.value) return;
	arrastrando.value = false;

	if (region.value === null) {
		// Un clic suelto no entrega: tocar la pantalla sin querer no puede
		// guardar la pantalla entera.
		desde.value = null;
		hasta.value = null;
		return;
	}

	// Las preferencias primero: entregar con las de siempre porque todavía no
	// llegaron es escribir un archivo que la preferencia decía que no.
	await cargando.value;

	const comando = comandoAlSoltar(ajustes.value.alSoltar);
	if (comando) void entregar(comando);
}

async function salir() {
	await getCurrentWindow().close();
}

async function entregar(comando: Comando) {
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
	// Con el panel abierto se está escribiendo una ruta: Intro la aplica y
	// Ctrl+C copia texto. Que el atajo de la ventana se los lleve significaría
	// guardar una captura desde adentro de un campo de texto.
	if (panel.value) {
		if (evento.key === 'Escape') panel.value = false;
		return;
	}

	if (evento.key === 'Escape') {
		void salir();
	} else if (evento.key === 'Enter') {
		void entregar('guardar_y_copiar');
	} else if (evento.key === 'c' && evento.ctrlKey) {
		void entregar('copiar');
	}
}

/**
 * Las preferencias, si se pueden leer.
 *
 * Que no se puedan **no** es un error que se muestre: quedan las de siempre y
 * la captura sigue su camino. Lo contrario sería no poder capturar nada porque
 * un archivo de configuración está roto.
 */
async function cargarAjustes() {
	try {
		ajustes.value = await invoke<Ajustes>('ajustes');
	} catch (e) {
		console.error('No se pudieron leer las preferencias', e);
	}
}

async function guardarAjustes(alSoltar: AlSoltar, carpeta: string | null) {
	errorPanel.value = '';
	try {
		// El backend devuelve cómo quedó: `~` expandido y espacios recortados no
		// son lo que se tecleó, y el panel tiene que mostrar lo guardado.
		ajustes.value = await invoke<Ajustes>('guardar_ajustes', { alSoltar, carpeta });
	} catch (e) {
		errorPanel.value = String(e);
	}
}

onMounted(async () => {
	window.addEventListener('keydown', alTeclado);
	cargando.value = cargarAjustes();
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
		@mouseup="terminar()"
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

		<PanelPreferencias
			v-if="panel"
			:ajustes="ajustes"
			:error="errorPanel"
			@guardar="guardarAjustes"
			@cerrar="panel = false"
		/>

		<div
			data-sin-arrastre
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
			<span class="h-5 w-px bg-ui-border"></span>
			<!-- La rueda dentada va dibujada acá y no por el resolvedor de
			     iconos: el composable que lo hacía se sacó por no usarse, y
			     traerlo de vuelta por un icono sería reabrir la copia. -->
			<button
				type="button"
				class="rounded-corner p-1.5 text-tx-muted hover:bg-ui-surface"
				:title="t('shot.preferencias')"
				:aria-label="t('shot.preferencias')"
				@click="panel = !panel"
			>
				<svg
					class="size-4"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
					stroke-linejoin="round"
					aria-hidden="true"
				>
					<circle cx="12" cy="12" r="3" />
					<path
						d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09a1.65 1.65 0 0 0-1.08-1.51 1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"
					/>
				</svg>
			</button>
		</div>

		<!-- Con fondo propio, no suelto sobre la imagen: el cuadro congelado puede
		     ser cualquier cosa —una terminal llena de texto, una foto clara— y sin
		     un respaldo la instrucción quedaba ilegible justo cuando más se
		     necesita, que es la primera vez que alguien abre esto. -->
		<p
			v-if="!region && !panel"
			class="-translate-x-1/2 -translate-y-1/2 absolute top-1/2 left-1/2 rounded-corner border border-ui-border bg-ui-bg/80 px-4 py-2 text-sm text-tx-main"
		>
			{{ t('shot.instruccion') }}
		</p>
	</main>
</template>
