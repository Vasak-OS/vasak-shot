<script setup lang="ts">
/**
 * La barra de anotación: qué dibujar y con qué.
 *
 * El color, el grosor y el relleno se recuerdan **por herramienta**. Elegir rojo
 * para la flecha no tiene por qué cambiar el del resaltador, y a la vez tener
 * que elegirlo cada vez es peor que no poder elegirlo.
 */
import { useI18n } from '@vasakgroup/tauri-plugin-i18n';
import { computed } from 'vue';
import {
	admiteRelleno,
	claveDe,
	type Estilo,
	GROSORES,
	HERRAMIENTAS,
	type Herramienta,
	TAPADOS,
	tapa,
} from '@/tools/anotacion';

const { t } = useI18n();

const props = defineProps<{
	herramienta: Herramienta | null;
	estilo: Estilo;
	puedeDeshacer: boolean;
	puedeRehacer: boolean;
}>();

const emitir = defineEmits<{
	elegir: [herramienta: Herramienta | null];
	estilo: [estilo: Estilo];
	deshacer: [];
	rehacer: [];
}>();

/**
 * Los colores, y por qué son estos.
 *
 * Seis, no una rueda: elegir un color exacto no es lo que hace falta al anotar
 * una captura, y una rueda de color abre un paso más en el medio de un gesto que
 * dura tres segundos. Son los que se leen sobre casi cualquier fondo.
 */
const COLORES = ['#e01b24', '#f5c211', '#33d17a', '#3584e4', '#ffffff', '#000000'];

const medidas = computed(() => (props.herramienta && tapa(props.herramienta) ? TAPADOS : GROSORES));
const conRelleno = computed(() => props.herramienta !== null && admiteRelleno(props.herramienta));
/** Las que tapan no tienen color: no dibujan nada, transforman lo de abajo. */
const conColor = computed(() => props.herramienta !== null && !tapa(props.herramienta));

function cambiar(parte: Partial<Estilo>) {
	emitir('estilo', { ...props.estilo, ...parte });
}
</script>

<template>
	<div
		data-sin-arrastre
		class="-translate-x-1/2 absolute bottom-24 left-1/2 flex max-w-[calc(100vw-2rem)] flex-wrap items-center justify-center gap-1 rounded-corner border border-ui-border bg-ui-bg/90 p-1.5"
	>
		<!-- Sin herramienta: el modo en el que se ajusta la selección. Va
		     primero porque es al que se vuelve. -->
		<button
			type="button"
			class="rounded-corner px-2.5 py-1.5 text-sm"
			:class="props.herramienta === null ? 'bg-primary text-tx-on-primary' : 'text-tx-main hover:bg-ui-surface'"
			:title="t('shot.ajustar')"
			@click="emitir('elegir', null)"
		>
			{{ t('shot.ajustar') }}
		</button>

		<span class="h-5 w-px bg-ui-border"></span>

		<button
			v-for="tipo in HERRAMIENTAS"
			:key="tipo"
			type="button"
			class="rounded-corner px-2.5 py-1.5 text-sm"
			:class="props.herramienta === tipo ? 'bg-primary text-tx-on-primary' : 'text-tx-main hover:bg-ui-surface'"
			:title="t(claveDe(tipo))"
			@click="emitir('elegir', tipo)"
		>
			{{ t(claveDe(tipo)) }}
		</button>

		<template v-if="props.herramienta">
			<span class="h-5 w-px bg-ui-border"></span>

			<button
				v-for="color in COLORES"
				v-show="conColor"
				:key="color"
				type="button"
				class="size-5 rounded-full border"
				:class="props.estilo.color === color ? 'border-primary ring-2 ring-primary' : 'border-ui-border'"
				:style="{ backgroundColor: color }"
				:aria-label="color"
				@click="cambiar({ color })"
			></button>

			<span v-show="conColor" class="h-5 w-px bg-ui-border"></span>

			<button
				v-for="grosor in medidas"
				:key="grosor"
				type="button"
				class="rounded-corner px-2 py-1 font-mono text-xs"
				:class="props.estilo.grosor === grosor ? 'bg-primary text-tx-on-primary' : 'text-tx-muted hover:bg-ui-surface'"
				@click="cambiar({ grosor })"
			>
				{{ grosor }}
			</button>

			<button
				v-show="conRelleno"
				type="button"
				class="rounded-corner px-2.5 py-1.5 text-sm"
				:class="props.estilo.relleno ? 'bg-primary text-tx-on-primary' : 'text-tx-main hover:bg-ui-surface'"
				@click="cambiar({ relleno: !props.estilo.relleno })"
			>
				{{ t('shot.relleno') }}
			</button>
		</template>

		<span class="h-5 w-px bg-ui-border"></span>

		<button
			type="button"
			class="rounded-corner px-2.5 py-1.5 text-sm text-tx-main hover:bg-ui-surface disabled:opacity-40"
			:disabled="!props.puedeDeshacer"
			:title="t('shot.deshacer')"
			@click="emitir('deshacer')"
		>
			{{ t('shot.deshacer') }}
		</button>
		<button
			type="button"
			class="rounded-corner px-2.5 py-1.5 text-sm text-tx-main hover:bg-ui-surface disabled:opacity-40"
			:disabled="!props.puedeRehacer"
			:title="t('shot.rehacer')"
			@click="emitir('rehacer')"
		>
			{{ t('shot.rehacer') }}
		</button>
	</div>
</template>
