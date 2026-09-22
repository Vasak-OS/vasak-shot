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
import Lupa from '@/components/Lupa.vue';
import PanelPreferencias from '@/components/PanelPreferencias.vue';
import Tiradores from '@/components/Tiradores.vue';
import { interpolar } from '@/tools/interpolar';
import { type Lienzo, medidaEnCss } from '@/tools/lienzo';
import {
	type Ajustes,
	type AlSoltar,
	type Comando,
	comandoAlSoltar,
	POR_OMISION,
} from '@/tools/preferencias';
import {
	ajustar,
	contiene,
	correr,
	medidasDe,
	type Punto,
	type Region,
	type Rol,
	redimensionar,
	aEntregar as regionAEntregar,
	regionEntre,
} from '@/tools/region';
import { comoRegion, type Ventana, ventanaEn } from '@/tools/ventanas';

const { t } = useI18n();

const lienzo = ref<Lienzo | null>(null);
const fondo = ref('');
const error = ref('');
const trabajando = ref(false);
const aviso = ref('');

/**
 * Lo elegido, en píxeles CSS de esta pantalla.
 *
 * Un `ref` y no algo calculado del arrastre, que es lo que era. El arrastre
 * dejó de ser la única manera de elegir: ahora también se señala una ventana y
 * se corrigen los bordes después, y con la región deducida del gesto no había
 * dónde anotar el resultado de esas dos.
 */
const seleccion = ref<Region | null>(null);

/**
 * El arrastre en curso, si lo hay.
 *
 * Tener el gesto en su propia variable es lo que hace que soltar termine: antes
 * se deducía de que hubiera un punto de partida, y ése sigue existiendo después
 * de soltar porque lo elegido tiene que quedar dibujado.
 */
const arrastre = ref<{ desde: Punto; hasta: Punto } | null>(null);

/**
 * Lo que se está corrigiendo: qué borde, y desde dónde.
 *
 * Se guarda la región y el punto de **cuando se agarró**, no los de recién. Con
 * diferencias sucesivas, un delta que el borde del lienzo recortó se pierde
 * —empujar contra el borde y volver no deja la selección donde estaba— y al
 * cruzar el borde opuesto el rol queda nombrando el de antes.
 *
 * `mover` no es un tirador sino el interior de la selección, pero se arrastra
 * igual y se termina igual, así que va por el mismo camino en lugar de por un
 * tercer estado que habría que apagar en los mismos lugares.
 */
const ajuste = ref<{ rol: Rol | 'mover'; origen: Region; inicio: Punto } | null>(null);

/** Dónde está el puntero, para la lupa y para saber qué ventana se señala. */
const puntero = ref<Punto | null>(null);

/** Las ventanas que había cuando se tomó la captura. Vacía si no se pudo saber. */
const ventanas = ref<Ventana[]>([]);

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

/** El tamaño de esta pantalla, que es el límite de todo lo que se puede elegir. */
const pantalla = computed(() => lienzo.value?.salida ?? { ancho: 0, alto: 0 });

/**
 * La ventana que se está señalando, si es que se está señalando alguna.
 *
 * Sólo mientras no haya nada elegido ni nada en curso: una vez que hay región,
 * el recuadro de una ventana atrás sería ruido sobre lo que se está por
 * ajustar.
 */
const resaltada = computed<Ventana | null>(() => {
	if (seleccion.value || arrastre.value || ajuste.value || panel.value) return null;
	return ventanaEn(ventanas.value, puntero.value);
});

/**
 * Si la lupa se muestra.
 *
 * Mientras se elige y mientras se corrige, que es cuando el píxel exacto
 * importa. No sobre una ventana señalada —ahí el rectángulo lo pone el
 * compositor, no la mano— ni sobre una región ya elegida y quieta.
 */
const conLupa = computed(
	() =>
		!panel.value &&
		puntero.value !== null &&
		(arrastre.value !== null || ajuste.value !== null || (!seleccion.value && !resaltada.value))
);

/**
 * La región que se va a entregar: la elegida, o toda **esta** pantalla si no hay.
 *
 * Esta pantalla y no la captura entera. Las coordenadas que se mandan son las de
 * la ventana, y la ventana cubre una sola salida; entregar el tamaño de la
 * composición pediría un rectángulo que no existe acá. Rust lo traduce después.
 */
const aEntregar = computed<Region | null>(() =>
	regionAEntregar(seleccion.value, lienzo.value?.salida ?? null)
);

const medidas = computed(() => medidasDe(aEntregar.value));

/** Dónde cayó el evento, en píxeles CSS de esta pantalla. */
function puntoDe(evento: MouseEvent): Punto {
	return { x: evento.clientX, y: evento.clientY };
}

function empezar(evento: MouseEvent) {
	// La barra y el panel no son lienzo. Se marcan con `data-sin-arrastre` en
	// lugar de enumerar etiquetas: el panel tiene campos y etiquetas además de
	// botones, y una lista de etiquetas queda vieja en cuanto se agrega una.
	if ((evento.target as HTMLElement).closest('[data-sin-arrastre]')) return;
	const punto = puntoDe(evento);
	// Se anota también acá y no sólo al mover: el selector aparece de golpe
	// bajo un puntero que puede estar quieto, y entonces un clic sin moverlo
	// llegaba a `terminar` sin saber dónde había caído — o sea sin poder elegir
	// la ventana que estaba debajo.
	puntero.value = punto;

	// Adentro de lo ya elegido, arrastrar lo **mueve**. Empezar uno nuevo desde
	// ahí sería no poder corregir la posición sin rehacer la selección entera,
	// que es justamente lo que se quiso evitar.
	if (seleccion.value && contiene(seleccion.value, punto)) {
		ajuste.value = { rol: 'mover', origen: seleccion.value, inicio: punto };
		return;
	}

	arrastre.value = { desde: punto, hasta: punto };
	// Empezar un arrastre nuevo descarta lo anterior: si no, el rectángulo
	// viejo quedaría dibujado mientras se elige otro.
	seleccion.value = null;
}

/** Agarra un tirador. El arrastre lo sigue `mover`, como el de la región. */
function tomarTirador(rol: Rol, evento: MouseEvent) {
	if (!seleccion.value) return;
	ajuste.value = { rol, origen: seleccion.value, inicio: puntoDe(evento) };
}

function mover(evento: MouseEvent) {
	const punto = puntoDe(evento);
	puntero.value = punto;

	if (ajuste.value) {
		// Contra la región y el punto de cuando se agarró, no contra los de
		// recién: así el borde arrastrado está siempre donde está el puntero,
		// aunque la región se haya dado vuelta o haya topado con el borde.
		const { rol, origen, inicio } = ajuste.value;
		seleccion.value =
			rol === 'mover'
				? correr(origen, punto.x - inicio.x, punto.y - inicio.y, pantalla.value)
				: redimensionar(origen, rol, punto, pantalla.value);
		return;
	}

	if (!arrastre.value) return;
	arrastre.value = { desde: arrastre.value.desde, hasta: punto };
	seleccion.value = regionEntre(arrastre.value.desde, punto);
}

/**
 * Soltar cierra el gesto, y entrega.
 *
 * Las dos cosas, y en ese orden. Cerrarlo es lo que hace que la región deje de
 * seguir al puntero. Entregar es el resto: se arrastra sobre lo que se quiere y
 * al levantar el dedo la captura ya está.
 *
 * **Corregir un borde no entrega.** Quien está moviendo un tirador está
 * corrigiendo lo que eligió, y entregar ahí sería no dejarlo terminar — que es
 * justamente lo que los tiradores vinieron a permitir. De todos modos sólo se
 * llega a ellos con «esperar», que es la preferencia que no entrega al soltar.
 *
 * Y con `esperar` no entrega nada tampoco acá: quedan los botones.
 */
async function terminar() {
	if (ajuste.value) {
		ajuste.value = null;
		return;
	}

	if (!arrastre.value) return;
	arrastre.value = null;

	if (seleccion.value === null) {
		// Sin arrastre, lo que vale es lo que se estaba señalando. Un clic sobre
		// una ventana la elige entera, que es la manera de agarrar su borde
		// exacto sin encuadrarla a ojo.
		const ventana = ventanaEn(ventanas.value, puntero.value);
		if (!ventana) return;
		seleccion.value = comoRegion(ventana);
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

	if (conFlechas(evento)) return;

	if (evento.key === 'Escape') {
		void salir();
	} else if (evento.key === 'Enter') {
		void entregar('guardar_y_copiar');
	} else if (evento.key === 'c' && evento.ctrlKey) {
		void entregar('copiar');
	}
}

/** Cuánto se mueve de un tecleo. Diez con Mayús, para cruzar la pantalla. */
const PASO = 1;
const PASO_LARGO = 10;

/**
 * Las flechas corrigen la selección: el único camino que llega al píxel exacto.
 *
 * Con `Ctrl` mueven el borde de abajo a la derecha en lugar de la región
 * entera, o sea redimensionan. Es la combinación que queda libre: `Ctrl+C` ya
 * copia, y `Alt` se lo lleva el compositor.
 *
 * Devuelve si la tecla era suya, para que quien llama no siga buscándole otro
 * significado.
 */
function conFlechas(evento: KeyboardEvent): boolean {
	const ejes: Record<string, Punto> = {
		ArrowLeft: { x: -1, y: 0 },
		ArrowRight: { x: 1, y: 0 },
		ArrowUp: { x: 0, y: -1 },
		ArrowDown: { x: 0, y: 1 },
	};
	const eje = ejes[evento.key];
	if (!eje || !seleccion.value) return false;

	evento.preventDefault();
	const paso = evento.shiftKey ? PASO_LARGO : PASO;
	const dx = eje.x * paso;
	const dy = eje.y * paso;

	seleccion.value = evento.ctrlKey
		? ajustar(seleccion.value, 'br', dx, dy, pantalla.value)
		: correr(seleccion.value, dx, dy, pantalla.value);
	return true;
}

/**
 * Las preferencias, si se pueden leer.
 *
 * Que no se puedan **no** es un error que se muestre: quedan las de siempre y
 * la captura sigue su camino. Lo contrario sería no poder capturar nada porque
 * un archivo de configuración está roto.
 */
async function cargarVentanas() {
	try {
		ventanas.value = await invoke<Ventana[]>('ventanas');
	} catch (e) {
		console.error('No se pudo saber dónde están las ventanas', e);
	}
}

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
	// Sin esperarla: que no se pueda saber dónde están las ventanas no puede
	// demorar el arrastre, que es lo que funciona sin ellas.
	void cargarVentanas();
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
		backgroundSize: `${medidaEnCss(l).ancho}px ${medidaEnCss(l).alto}px`,
		backgroundPosition: `${-l.salida.x}px ${-l.salida.y}px`,
		backgroundRepeat: 'no-repeat',
	};
});

const estilo = computed(() => {
	const r = seleccion.value;
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
		@mouseleave="puntero = null"
	>
		<!-- El velo se apaga en cuanto hay una selección: con los dos, la zona
		     elegida quedaría oscurecida dos veces.
		     Negro y no un color del tema: no es una superficie de la interfaz
		     sino una atenuación sobre la captura, y tiene que oscurecer igual con
		     el tema claro. -->
		<div v-if="!seleccion" class="absolute inset-0 bg-black/55"></div>

		<!-- La ventana que se está señalando. Es el mismo recuadro que la
		     selección pero sin tiradores: todavía no se eligió nada, se está
		     mostrando qué pasaría al hacer clic. -->
		<div
			v-if="resaltada"
			class="absolute rounded-corner border-2 border-primary bg-primary/10"
			:style="{
				left: `${resaltada.x}px`,
				top: `${resaltada.y}px`,
				width: `${resaltada.ancho}px`,
				height: `${resaltada.alto}px`,
			}"
		>
			<span
				class="-top-7 absolute left-0 whitespace-nowrap rounded-corner bg-primary px-2 py-0.5 font-mono text-tx-on-primary text-xs"
			>
				{{ resaltada.ancho }} × {{ resaltada.alto }}
			</span>
		</div>

		<!-- El recorte de la selección se hace con una sombra enorme en lugar de
		     cuatro divs: así el borde queda pegado al rectángulo sin cuentas. -->
		<div
			v-if="seleccion"
			class="absolute rounded-corner border border-primary shadow-[0_0_0_9999px_rgba(0,0,0,0.55)]"
			:class="arrastre ? '' : 'cursor-move'"
			:style="estilo"
		>
			<span
				class="-top-7 absolute left-0 whitespace-nowrap rounded-corner bg-primary px-2 py-0.5 font-mono text-tx-on-primary text-xs"
			>
				{{ medidas }}
			</span>

			<!-- Los tiradores, sólo con el gesto terminado: mientras se
			     arrastra la región ya sigue al puntero y ocho puntos moviéndose
			     con ella son ruido. -->
			<Tiradores v-if="!arrastre" @tomar="tomarTirador" />
		</div>

		<Lupa v-if="conLupa && puntero && lienzo" :punto="puntero" :lienzo="lienzo" :fondo="fondo" />

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
			v-if="!seleccion && !panel"
			class="-translate-x-1/2 -translate-y-1/2 absolute top-1/2 left-1/2 rounded-corner border border-ui-border bg-ui-bg/80 px-4 py-2 text-sm text-tx-main"
		>
			{{ ventanas.length ? t('shot.instruccionConVentanas') : t('shot.instruccion') }}
		</p>
	</main>
</template>
