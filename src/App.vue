<script setup lang="ts">
/**
 * La superficie de selección.
 *
 * Muestra el cuadro **ya capturado** —congelado— y deja elegir una región encima.
 * Que la imagen esté quieta no es un detalle estético: la selección se hace sobre
 * lo que la persona vio al apretar la tecla, no sobre una pantalla que sigue
 * cambiando debajo mientras arrastra.
 */
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useI18n } from '@vasakgroup/tauri-plugin-i18n';
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import BarraDeAnotacion from '@/components/BarraDeAnotacion.vue';
import BarraDeCaptura from '@/components/BarraDeCaptura.vue';
import CapaDeAnotaciones from '@/components/CapaDeAnotaciones.vue';
import ConfirmarSubida from '@/components/ConfirmarSubida.vue';
import Lupa from '@/components/Lupa.vue';
import PanelPreferencias from '@/components/PanelPreferencias.vue';
import Tiradores from '@/components/Tiradores.vue';
import {
	type Anotacion,
	type Estilo,
	esDeUnPunto,
	estiloPorOmision,
	type Herramienta,
	proximoNumero,
} from '@/tools/anotacion';
import { comenzar, continuar, vale } from '@/tools/gesto';
import * as historial from '@/tools/historial';
import { interpolar } from '@/tools/interpolar';
import { type Lienzo, medidaEnCss } from '@/tools/lienzo';
import { withTempObjectUrl } from '@/tools/object-url';
import {
	type Ajustes,
	type Comando,
	comandoAlSoltar,
	type Guardado,
	POR_OMISION,
} from '@/tools/preferencias';
import {
	ajustar,
	comoRegion,
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
import { vuelveACapturar } from '@/tools/retardo';
import { type Monitor, type Salidas, SIN_SALIDAS } from '@/tools/salidas';
import { type Ventana, ventanaEn } from '@/tools/ventanas';

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

/** Las pantallas que entraron en la captura, en coordenadas de ésta. */
const salidas = ref<Salidas>(SIN_SALIDAS);

/** Qué se dibuja. Nulo es el modo en el que se ajusta la selección. */
const herramienta = ref<Herramienta | null>(null);

/** Lo dibujado, con su historial. */
const dibujo = ref<historial.Historial>(historial.crear());

/** Lo que se está dibujando ahora mismo, todavía sin cerrar. */
const enCurso = ref<Anotacion | null>(null);

/** El texto que se está escribiendo, si es que se está escribiendo alguno. */
const escribiendo = ref<{ punto: Punto; texto: string } | null>(null);

/**
 * El color, el grosor y el relleno de cada herramienta.
 *
 * Por herramienta y no uno solo: elegir rojo para la flecha no tiene por qué
 * cambiar el del resaltador. Arranca vacío y cada una se estrena con lo suyo.
 */
const estilos = ref<Partial<Record<Herramienta, Estilo>>>({});

/** El color con el que se estrena cualquier herramienta. */
const COLOR_INICIAL = '#e01b24';

const estiloActual = computed<Estilo>(() => {
	const tipo = herramienta.value;
	if (!tipo) return { color: COLOR_INICIAL, grosor: 3, relleno: false };
	return estilos.value[tipo] ?? estiloPorOmision(tipo, COLOR_INICIAL);
});

/** El lienzo de anotación, para pedirle el PNG al entregar. */
const capa = ref<{ exportar: () => Promise<Uint8Array> } | null>(null);

/** El campo del texto, para darle el foco en cuanto aparece. */
const campoDeTexto = ref<HTMLInputElement | null>(null);

const puedeDeshacer = computed(() => historial.puedeDeshacer(dibujo.value));
const puedeRehacer = computed(() => historial.puedeRehacer(dibujo.value));
const deshacerDibujo = historial.deshacer;
const rehacerDibujo = historial.rehacer;

/** Las preferencias, con las de por omisión mientras el backend no conteste. */
const ajustes = ref<Ajustes>(POR_OMISION);

/**
 * La lectura de las preferencias, mientras está en curso.
 *
 * Se guarda para poder esperarla al soltar. La ventana aparece de golpe y el
 * gesto puede empezar en el primer cuadro, así que sin esto un arrastre que
 * termina antes de que el backend conteste se entrega con lo que haya —y lo
 * que hay es lo de por omisión, que ahora no entrega nada, pero la preferencia
 * guardada puede decir «guardar y copiar» y entonces el archivo saldría igual
 * sin que nadie lo haya pedido en esta sesión.
 */
const cargando = ref<Promise<void> | null>(null);
const panel = ref(false);
const errorPanel = ref('');

/**
 * Si se está preguntando antes de subir.
 *
 * La pregunta no es un adorno: subir publica, y el resto de la herramienta
 * guarda y copia en esta máquina. Ver `ConfirmarSubida`.
 */
const confirmandoSubida = ref(false);

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
	if (confirmandoSubida.value) return null;
	return ventanaEn(ventanas.value, puntero.value);
});

/**
 * La pantalla entera, cuando no se está señalando nada más chico.
 *
 * Es el tercer escalón del resaltado, y el orden va de lo más chico y explícito
 * a lo más grande: región arrastrada, después ventana, después pantalla. Sin
 * esto, apretar Intro sin arrastrar ya entregaba esta pantalla entera — pero no
 * había manera de enterarse antes de que pasara.
 *
 * No apaga la lupa, a diferencia de una ventana señalada: acá no hay ningún
 * borde que el compositor haya puesto, y el píxel exacto por donde empezar a
 * arrastrar sigue importando.
 */
const pantallaResaltada = computed<Monitor | null>(() => {
	if (seleccion.value || arrastre.value || ajuste.value || panel.value) return null;
	if (resaltada.value || !puntero.value) return null;
	return salidas.value.actual;
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
		!confirmandoSubida.value &&
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
	// Con la pregunta de subir abierta, el lienzo no responde: lo que hay que
	// hacer es contestarla, y un arrastre debajo cambiaría lo que se va a subir.
	if (confirmandoSubida.value) return;
	const punto = puntoDe(evento);
	// Se anota también acá y no sólo al mover: el selector aparece de golpe
	// bajo un puntero que puede estar quieto, y entonces un clic sin moverlo
	// llegaba a `terminar` sin saber dónde había caído — o sea sin poder elegir
	// la ventana que estaba debajo.
	puntero.value = punto;

	// Con una herramienta tomada, adentro de la selección se dibuja. Ajustar la
	// selección vuelve a estar a mano soltando la herramienta, que es lo que
	// hace el primer botón de la barra.
	if (herramienta.value && seleccion.value && contiene(seleccion.value, punto)) {
		empezarADibujar(herramienta.value, punto);
		return;
	}

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

/**
 * Arranca una anotación.
 *
 * Las de un punto no tienen arrastre: el texto abre su campo y el paso queda
 * puesto ahí mismo, con el número que le toca.
 */
function empezarADibujar(tipo: Herramienta, punto: Punto) {
	if (tipo === 'texto') {
		escribiendo.value = { punto, texto: '' };
		return;
	}
	const anotacion = comenzar(tipo, punto, estiloActual.value, proximoNumero(dibujo.value.items));
	if (esDeUnPunto(tipo)) {
		cerrarAnotacion(anotacion);
		return;
	}
	enCurso.value = anotacion;
}

/** Cierra la anotación y la suma al historial, si vale la pena guardarla. */
function cerrarAnotacion(anotacion: Anotacion) {
	if (vale(anotacion)) dibujo.value = historial.agregar(dibujo.value, anotacion);
	enCurso.value = null;
}

/** Agarra un tirador. El arrastre lo sigue `mover`, como el de la región. */
function tomarTirador(rol: Rol, evento: MouseEvent) {
	if (!seleccion.value) return;
	ajuste.value = { rol, origen: seleccion.value, inicio: puntoDe(evento) };
}

function mover(evento: MouseEvent) {
	const punto = puntoDe(evento);
	puntero.value = punto;

	if (enCurso.value) {
		enCurso.value = continuar(enCurso.value, punto);
		return;
	}

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
 * Soltar cierra el gesto, y entrega si la preferencia lo dice.
 *
 * Cerrarlo es lo que hace que la región deje de seguir al puntero, y pasa
 * siempre. Entregar depende: **por omisión no entrega**, porque al soltar
 * recién empieza lo que se puede hacer con la captura —anotarla, tapar algo,
 * correr un borde—, y entregarla sola es no dejar hacer nada de eso. Con
 * «guardar y copiar» sí: se arrastra sobre lo que se quiere y al levantar el
 * dedo la captura ya está.
 *
 * **Corregir un borde no entrega nunca.** Quien está moviendo un tirador está
 * corrigiendo lo que eligió, y entregar ahí sería no dejarlo terminar — que es
 * justamente lo que los tiradores vinieron a permitir.
 */
async function terminar() {
	if (enCurso.value) {
		cerrarAnotacion(enCurso.value);
		return;
	}

	if (ajuste.value) {
		ajuste.value = null;
		return;
	}

	if (!arrastre.value) return;
	arrastre.value = null;

	if (seleccion.value === null) {
		// Sin arrastre, lo que vale es lo que se estaba señalando. Un clic sobre
		// una ventana la elige entera, que es la manera de agarrar su borde
		// exacto sin encuadrarla a ojo; sobre el fondo, la pantalla entera, que
		// es lo que el resaltado venía mostrando.
		const ventana = ventanaEn(ventanas.value, puntero.value);
		const region = ventana ? comoRegion(ventana) : aEntregar.value;
		if (!region) return;
		seleccion.value = region;
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

/**
 * Entrega lo elegido.
 *
 * Con anotaciones va **el mapa de bits ya compuesto**, no la región: lo que se
 * guarda tiene que ser exactamente lo que se vio, y el único dibujante es el
 * canvas. Sin anotaciones sigue yendo la región y nada más, que son cuatro
 * números: el PNG pesa megabytes y no hay por qué cruzarlos cuando nadie dibujó.
 *
 * La región puede venir de afuera: es como se entrega **otra pantalla**, que no
 * se puede elegir con el ratón porque esta ventana no llega hasta ella. Va en
 * coordenadas de ésta, que es el mismo espacio en el que Rust traduce todo lo
 * demás.
 */
async function entregar(comando: Comando, explicita?: Region) {
	const r = explicita ?? aEntregar.value;
	if (!r || trabajando.value) return;
	trabajando.value = true;
	error.value = '';
	try {
		// Con anotaciones, los bytes van solos como cuerpo crudo del pedido:
		// adentro de un JSON serían una lista de números y costarían un orden de
		// magnitud más.
		// Con una región explícita —otra pantalla— no van las anotaciones: lo
		// dibujado está sobre **esta**, y el lienzo de anotación ni siquiera
		// cubre lo que se está pidiendo.
		const lienzoDeAnotacion = !explicita && dibujo.value.items.length > 0 ? capa.value : null;
		const ruta = lienzoDeAnotacion
			? await invoke<string | null>(`${comando}_anotada`, await lienzoDeAnotacion.exportar())
			: await invoke<string | null>(comando, { region: r });
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
	// Escribiendo un texto, las teclas son del campo. Que el atajo de la
	// ventana se lleve la «c» de «captura» sería copiar en vez de escribir.
	if (escribiendo.value) {
		if (evento.key === 'Escape') escribiendo.value = null;
		return;
	}

	// Con el panel abierto se está escribiendo una ruta: Intro la aplica y
	// Ctrl+C copia texto. Que el atajo de la ventana se los lleve significaría
	// guardar una captura desde adentro de un campo de texto.
	if (panel.value) {
		if (evento.key === 'Escape') panel.value = false;
		return;
	}

	// Y con la pregunta de subir abierta, Escape la cancela y lo demás no hace
	// nada: que Intro llegue a la ventana sería guardar una captura mientras se
	// está decidiendo si publicarla.
	if (confirmandoSubida.value) {
		if (evento.key === 'Escape') confirmandoSubida.value = false;
		return;
	}

	// Después de la guarda del panel: ahí se está escribiendo una ruta, y
	// `Ctrl+Z` es el deshacer del campo de texto, no el del dibujo.
	if (evento.key.toLowerCase() === 'z' && evento.ctrlKey) {
		evento.preventDefault();
		dibujo.value = evento.shiftKey
			? historial.rehacer(dibujo.value)
			: historial.deshacer(dibujo.value);
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

/** Cierra el texto que se está escribiendo, si dice algo. */
function cerrarTexto() {
	const abierto = escribiendo.value;
	if (!abierto) return;
	escribiendo.value = null;
	cerrarAnotacion({
		...comenzar('texto', abierto.punto, estiloActual.value),
		texto: abierto.texto,
	});
}

/** Guarda el estilo elegido para la herramienta que está tomada. */
function cambiarEstilo(estilo: Estilo) {
	const tipo = herramienta.value;
	if (!tipo) return;
	estilos.value = { ...estilos.value, [tipo]: estilo };
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

/**
 * Los textos del aviso de guardado, ya traducidos.
 *
 * Los manda el frontend porque el aviso lo muestra **otro proceso** —uno sin
 * ventana, donde el plugin de idioma no existe— y el catálogo está de este
 * lado. Que no lleguen no es un error: quedan los de reserva, en español.
 */
async function traducirAviso() {
	try {
		await invoke('traducir_aviso', {
			textos: {
				guardada: t('shot.avisoGuardada'),
				abrir: t('shot.avisoAbrir'),
				carpeta: t('shot.avisoCarpeta'),
				copiar: t('shot.avisoCopiar'),
			},
		});
	} catch (e) {
		console.error('No se pudieron traducir los textos del aviso', e);
	}
}

async function cargarSalidas() {
	try {
		salidas.value = await invoke<Salidas>('salidas');
	} catch (e) {
		console.error('No se pudo saber qué pantallas hay', e);
	}
}

/**
 * Vuelve a capturar dentro de unos segundos, con esta ventana cerrada.
 *
 * Cerrarla es la mitad del asunto: lo que se quiere fotografiar es un menú
 * abierto, y el menú no se puede abrir con el selector tapando la pantalla.
 * Quien captura de nuevo es otro proceso; éste se va.
 */
async function conRetardo(segundos: number) {
	// El cero es el valor que ya está puesto: el botón está para mostrar cuál
	// es, no para volver a lanzar nada.
	if (!vuelveACapturar(segundos)) return;
	trabajando.value = true;
	try {
		await invoke('recapturar', { retardo: segundos });
	} catch (e) {
		error.value = String(e);
		trabajando.value = false;
		return;
	}
	await salir();
}

/**
 * Entrega una pantalla entera, sea ésta o cualquier otra.
 *
 * Con la acción de las preferencias, salvo que sea «esperar»: elegir una
 * pantalla en la barra es un pedido explícito, igual que apretar Intro, y ahí
 * no hay nada que esperar.
 */
async function entregarPantalla(monitor: Monitor) {
	// Las preferencias primero, igual que al soltar el botón: las salidas y los
	// ajustes se piden a la vez, y la barra aparece en cuanto llegan las
	// primeras. Sin esperar, elegir una pantalla apenas abre la ventana
	// guardaría un archivo que la preferencia decía que no.
	await cargando.value;
	await entregar(
		comandoAlSoltar(ajustes.value.alSoltar) ?? 'guardar_y_copiar',
		comoRegion(monitor)
	);
}

async function cargarAjustes() {
	try {
		ajustes.value = await invoke<Ajustes>('ajustes');
	} catch (e) {
		console.error('No se pudieron leer las preferencias', e);
	}
}

async function guardarAjustes(cambios: Guardado) {
	errorPanel.value = '';
	try {
		// El backend devuelve cómo quedó: `~` expandido y espacios recortados no
		// son lo que se tecleó, y el panel tiene que mostrar lo guardado. La
		// dirección para subir, además, puede no ser aceptada.
		// Desarmado y no tal cual: `invoke` pide un objeto con índice de cadena, y
		// una interfaz no lo tiene.
		ajustes.value = await invoke<Ajustes>('guardar_ajustes', { ...cambios });
	} catch (e) {
		errorPanel.value = String(e);
	}
}

/**
 * Sube lo elegido, ya contestada la pregunta.
 *
 * El enlace queda en el portapapeles —lo hace Rust— y a la vista un rato más
 * largo que el aviso de guardado: es lo único que queda de la captura, y
 * cerrarse enseguida sería perderlo.
 */
async function subir() {
	confirmandoSubida.value = false;
	const r = aEntregar.value;
	if (!r || trabajando.value) return;
	trabajando.value = true;
	error.value = '';
	try {
		const lienzoDeAnotacion = dibujo.value.items.length > 0 ? capa.value : null;
		const enlace = lienzoDeAnotacion
			? await invoke<string>('subir_anotada', await lienzoDeAnotacion.exportar())
			: await invoke<string>('subir', { region: r });
		aviso.value = interpolar(t('shot.subida'), enlace);
		setTimeout(() => void salir(), 2500);
	} catch (e) {
		error.value = String(e);
		trabajando.value = false;
	}
}

/** Si la ventana ya se cerró: lo que quedó a medio cargar no tiene dueño. */
let desmontado = false;

onMounted(async () => {
	window.addEventListener('keydown', alTeclado);
	cargando.value = cargarAjustes();
	// Sin esperarlas: que no se pueda saber dónde están las ventanas ni cuántas
	// pantallas hay no puede demorar el arrastre, que es lo que funciona sin
	// las dos cosas.
	void cargarVentanas();
	void cargarSalidas();
	void traducirAviso();
	try {
		const l = await invoke<Lienzo>('lienzo');
		lienzo.value = l;
		// Los bytes por el IPC y un `blob:`, y no la URL `asset://` que devolvía
		// `convertFileSrc`.
		//
		// No es una preferencia de estilo: `asset://` es **otro origen**, y una
		// imagen de otro origen contamina el canvas donde se la dibuja. Un canvas
		// contaminado no deja leer sus píxeles —o sea que difuminar y pixelar no
		// tapan nada— ni exportarlos —o sea que una captura anotada no se puede
		// guardar, copiar ni subir—. Un `blob:` hereda el origen de este
		// documento, así que el canvas queda limpio. Ver el comando `imagen`.
		const bytes = await invoke<ArrayBuffer>('imagen');
		const blob = new Blob([bytes], { type: 'image/png' });
		// Se comprueba que cargue **antes** de usarla como fondo.
		//
		// Sin esto, una imagen bloqueada dejaba la ventana transparente sobre el
		// escritorio vivo, y eso se ve casi igual que el cuadro congelado: la
		// selección parecía funcionar mientras en realidad se estaba eligiendo
		// sobre una pantalla que seguía moviéndose. Una falla que se disfraza de
		// funcionamiento es peor que una que se ve.
		fondo.value =
			(await withTempObjectUrl(blob, async (url) => {
				await new Promise<void>((listo, falla) => {
					const prueba = new Image();
					prueba.onload = () => listo();
					prueba.onerror = () => falla(new Error(t('shot.errorImagen')));
					prueba.src = url;
				});
				// Si la ventana se cerró mientras la imagen cargaba, `onUnmounted` ya
				// pasó y no vuelve: quedársela sería dejarla sin nadie que la suelte.
				return !desmontado;
			})) ?? '';
	} catch (e) {
		error.value = String(e);
	}
});

watch(escribiendo, async (abierto) => {
	if (!abierto) return;
	await nextTick();
	campoDeTexto.value?.focus();
});

onUnmounted(() => {
	desmontado = true;
	window.removeEventListener('keydown', alTeclado);
	// El `blob:` vive mientras viva el documento aunque nadie lo mire, y son
	// varios megabytes. La ventana se cierra enseguida y el proceso se lleva
	// todo, pero soltarlo acá es lo que hace que eso sea una casualidad y no la
	// única razón por la que no se acumula.
	if (fondo.value) URL.revokeObjectURL(fondo.value);
});

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

		<!-- La pantalla entera, cuando no se señala nada más chico. El mismo
		     recuadro que una ventana y por la misma razón: mostrar qué pasaría
		     al hacer clic antes de que pase. Sin fondo propio, que sobre una
		     pantalla entera sería un velo de más. -->
		<div
			v-if="pantallaResaltada"
			class="pointer-events-none absolute inset-0 rounded-corner border-2 border-primary"
		>
			<span
				class="absolute top-2 left-2 whitespace-nowrap rounded-corner bg-primary px-2 py-0.5 font-mono text-tx-on-primary text-xs"
			>
				{{ pantallaResaltada.nombre }} · {{ pantallaResaltada.ancho }} ×
				{{ pantallaResaltada.alto }}
			</span>
		</div>

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
			<Tiradores v-if="!arrastre && !herramienta" @tomar="tomarTirador" />
		</div>

		<!-- El lienzo de anotación va **después** del recuadro de la selección:
		     tapa la región con su propia copia de la captura, que es lo que deja
		     que las zonas tapadas lean píxeles. -->
		<CapaDeAnotaciones
			v-if="seleccion && lienzo && fondo"
			ref="capa"
			:region="seleccion"
			:lienzo="lienzo"
			:fondo="fondo"
			:anotaciones="dibujo.items"
			:en-curso="enCurso"
		/>

		<!-- El campo del texto, donde se hizo clic. Con foco automático: abrirlo
		     y tener que hacer un clic más para escribir sería un paso de más en
		     un gesto de tres segundos. -->
		<input
			v-if="escribiendo"
			ref="campoDeTexto"
			v-model="escribiendo.texto"
			data-sin-arrastre
			type="text"
			class="absolute rounded-corner border border-primary bg-ui-bg/90 px-1 text-tx-main"
			:style="{ left: `${escribiendo.punto.x}px`, top: `${escribiendo.punto.y}px` }"
			@keydown.enter.stop.prevent="cerrarTexto()"
			@blur="cerrarTexto()"
		/>

		<!-- Antes de elegir nada: qué capturar. Con una región elegida su lugar
		     lo ocupa la barra de anotación, que responde la otra pregunta. -->
		<BarraDeCaptura
			v-if="!seleccion && !panel"
			:salidas="salidas"
			:trabajando="trabajando"
			@retardo="conRetardo"
			@pantalla="entregarPantalla"
		/>

		<BarraDeAnotacion
			v-if="seleccion && !panel"
			:herramienta="herramienta"
			:estilo="estiloActual"
			:puede-deshacer="puedeDeshacer"
			:puede-rehacer="puedeRehacer"
			@elegir="herramienta = $event"
			@estilo="cambiarEstilo"
			@deshacer="dibujo = deshacerDibujo(dibujo)"
			@rehacer="dibujo = rehacerDibujo(dibujo)"
		/>

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

		<ConfirmarSubida
			v-if="confirmandoSubida && ajustes.subirServidor"
			:servidor="ajustes.subirServidor"
			:trabajando="trabajando"
			@subir="subir()"
			@cancelar="confirmandoSubida = false"
		/>

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
			<!-- Sólo con una dirección configurada: sin ella no hay a dónde
			     subir, y un botón que pregunta y después falla es peor que no
			     tenerlo. Sin atajo de teclado, tampoco: publicar no puede estar
			     a una tecla de distancia. -->
			<button
				v-if="ajustes.subirServidor"
				type="button"
				class="rounded-corner px-3 py-1.5 text-sm text-tx-main hover:bg-ui-surface disabled:opacity-50"
				:disabled="trabajando"
				@click="confirmandoSubida = true"
			>
				{{ t('shot.subir') }}
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
