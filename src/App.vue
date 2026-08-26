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

interface Lienzo {
	ruta: string;
	ancho: number;
	alto: number;
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

/** La región que se va a entregar: la elegida, o toda la pantalla si no hay. */
const aEntregar = computed<Region | null>(() => regionAEntregar(region.value, lienzo.value));

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
		:style="fondo ? { backgroundImage: `url(${fondo})`, backgroundSize: '100% 100%' } : {}"
		@mousedown="empezar"
		@mousemove="mover"
		@mouseup="terminar"
	>
		<!-- El velo se apaga en cuanto hay una selección: con los dos, la zona
		     elegida quedaría oscurecida dos veces. -->
		<div v-if="!region" class="absolute inset-0 bg-black/55"></div>

		<!-- El recorte de la selección se hace con una sombra enorme en lugar de
		     cuatro divs: así el borde queda pegado al rectángulo sin cuentas. -->
		<div
			v-if="region"
			class="absolute rounded-[2px] border border-primary shadow-[0_0_0_9999px_rgba(0,0,0,0.55)]"
			:style="estilo"
		>
			<span
				class="-top-7 absolute left-0 whitespace-nowrap rounded bg-primary px-2 py-0.5 font-mono text-white text-xs"
			>
				{{ medidas }}
			</span>
		</div>

		<div
			v-if="error"
			class="absolute top-6 left-1/2 -translate-x-1/2 rounded-corner bg-status-error/90 px-4 py-2 text-sm text-white"
		>
			{{ error }}
		</div>

		<div
			v-if="aviso"
			class="absolute top-6 left-1/2 -translate-x-1/2 rounded-corner bg-black/80 px-4 py-2 text-sm text-white"
		>
			{{ aviso }}
		</div>

		<div
			class="absolute bottom-8 left-1/2 flex -translate-x-1/2 items-center gap-1.5 rounded-corner border border-white/15 bg-black/85 p-1.5"
		>
			<span class="px-2 font-mono text-white/70 text-xs">{{ medidas }}</span>
			<span class="h-5 w-px bg-white/20"></span>
			<button
				type="button"
				class="flex items-center gap-1.5 rounded px-3 py-1.5 text-sm text-white/80 hover:bg-white/10 disabled:opacity-50"
				:disabled="trabajando"
				@click="entregar('copiar')"
			>
				{{ t('shot.copiar') }}
				<kbd class="font-mono text-[10px] text-white/50">Ctrl+C</kbd>
			</button>
			<button
				type="button"
				class="flex items-center gap-1.5 rounded bg-primary px-3 py-1.5 font-medium text-sm text-white hover:brightness-110 disabled:opacity-50"
				:disabled="trabajando"
				@click="entregar('guardar_y_copiar')"
			>
				{{ t('shot.guardar') }}
				<kbd class="font-mono text-[10px] text-white/70">Intro</kbd>
			</button>
			<span class="h-5 w-px bg-white/20"></span>
			<button
				type="button"
				class="rounded px-3 py-1.5 text-sm text-white/60 hover:bg-white/10"
				@click="salir()"
			>
				{{ t('shot.cancelar') }}
				<kbd class="ml-1 font-mono text-[10px] text-white/40">Esc</kbd>
			</button>
		</div>

		<p
			v-if="!region"
			class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 text-white/70 text-sm"
		>
			{{ t('shot.instruccion') }}
		</p>
	</main>
</template>
