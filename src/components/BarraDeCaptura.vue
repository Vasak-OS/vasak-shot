<script setup lang="ts">
/**
 * Qué capturar: con cuánto retardo, y de qué pantalla.
 *
 * Aparece **antes** de elegir nada, que es cuando estas dos preguntas tienen
 * sentido; con una región ya elegida su lugar lo ocupa la barra de anotación.
 * Las dos hacen lo mismo desde el punto de vista de quien mira: cambian qué va a
 * salir en la foto, no cómo se dibuja encima.
 */
import { useI18n } from '@vasakgroup/tauri-plugin-i18n';
import { computed } from 'vue';
import { RETARDOS } from '@/tools/retardo';
import { etiquetaDe, type Monitor, type Salidas, todas } from '@/tools/salidas';

const { t } = useI18n();

const props = defineProps<{
	salidas: Salidas;
	/** Mientras se está entregando algo, para no pedir dos cosas a la vez. */
	trabajando: boolean;
}>();

const emitir = defineEmits<{
	retardo: [segundos: number];
	pantalla: [monitor: Monitor];
}>();

/**
 * Las pantallas se ofrecen sólo si hay más de una.
 *
 * Con un solo monitor, un botón que dice el nombre del conector no agrega nada:
 * esa pantalla ya se captura con un clic en el fondo.
 */
const pantallas = computed(() => (props.salidas.otras.length > 0 ? todas(props.salidas) : []));
</script>

<template>
	<div
		data-sin-arrastre
		class="-translate-x-1/2 absolute bottom-24 left-1/2 flex max-w-[calc(100vw-2rem)] flex-wrap items-center justify-center gap-1 rounded-corner border border-ui-border bg-ui-bg/90 p-1.5"
	>
		<span class="px-2 text-sm text-tx-muted">{{ t('shot.retardo') }}</span>

		<!-- El cero queda marcado y no hace nada: es lo que está pasando ahora.
		     Sacarlo de la lista obligaría a adivinar cuál es el valor actual. -->
		<button
			v-for="segundos in RETARDOS"
			:key="segundos"
			type="button"
			class="rounded-corner px-2.5 py-1.5 font-mono text-sm"
			:class="segundos === 0 ? 'bg-primary text-tx-on-primary' : 'text-tx-main hover:bg-ui-surface'"
			:disabled="props.trabajando"
			@click="emitir('retardo', segundos)"
		>
			{{ segundos === 0 ? t('shot.sinRetardo') : `${segundos} s` }}
		</button>

		<template v-if="pantallas.length > 0">
			<span class="h-5 w-px bg-ui-border"></span>
			<span class="px-2 text-sm text-tx-muted">{{ t('shot.pantalla') }}</span>

			<!-- La de acá también va, y entrega igual que las otras: es lo mismo
			     que un clic en el fondo, pero dicho con todas las letras. -->
			<button
				v-for="monitor in pantallas"
				:key="monitor.nombre"
				type="button"
				class="rounded-corner px-2.5 py-1.5 text-sm text-tx-main hover:bg-ui-surface disabled:opacity-50"
				:disabled="props.trabajando"
				@click="emitir('pantalla', monitor)"
			>
				{{ etiquetaDe(monitor) }}
			</button>
		</template>
	</div>
</template>
