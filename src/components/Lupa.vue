<script setup lang="ts">
/**
 * La lupa: la captura ampliada alrededor del puntero.
 *
 * Es el **mismo** `background-image` que la pantalla, con otra escala y otro
 * origen. No lee píxeles ni pregunta nada, y eso es también lo que garantiza
 * que no se pueda desincronizar de lo que se ve debajo.
 */
import { type CSSProperties, computed } from 'vue';
import type { Lienzo } from '@/tools/lienzo';
import { AUMENTO, fondoDeLupa, posicionDeLupa, TAMANIO } from '@/tools/lupa';
import type { Punto } from '@/tools/region';

const props = defineProps<{ punto: Punto; lienzo: Lienzo; fondo: string }>();

const caja = computed(() => posicionDeLupa(props.punto, props.lienzo.salida));

// Tipado como `CSSProperties` y no suelto: `imageRendering` acepta unos pocos
// valores, y un `pixelated` mal escrito dejaría la lupa borrosa sin avisar.
const estilo = computed<CSSProperties>(() => {
	const fondo = fondoDeLupa(props.punto, props.lienzo);
	return {
		left: `${caja.value.x}px`,
		top: `${caja.value.y}px`,
		width: `${TAMANIO}px`,
		height: `${TAMANIO}px`,
		backgroundImage: `url(${props.fondo})`,
		backgroundSize: fondo.size,
		backgroundPosition: fondo.position,
		backgroundRepeat: 'no-repeat',
		// Sin esto el navegador suaviza al ampliar y la lupa muestra una mancha
		// donde tendría que mostrar píxeles, que es todo lo que se le pide.
		imageRendering: 'pixelated',
	};
});
</script>

<template>
	<div
		class="pointer-events-none absolute overflow-hidden rounded-corner border border-ui-border shadow-lg"
		:style="estilo"
	>
		<!-- La cruz, y el recuadro del píxel exacto que se está señalando. -->
		<div class="absolute top-0 left-1/2 h-full w-px bg-primary/60"></div>
		<div class="absolute top-1/2 left-0 h-px w-full bg-primary/60"></div>
		<div
			class="-translate-x-1/2 -translate-y-1/2 absolute top-1/2 left-1/2 border border-primary"
			:style="{ width: `${AUMENTO}px`, height: `${AUMENTO}px` }"
		></div>

		<span
			class="absolute bottom-0 left-0 w-full bg-ui-bg/80 text-center font-mono text-[10px] text-tx-main"
		>
			{{ props.punto.x }}, {{ props.punto.y }}
		</span>
	</div>
</template>
