<script setup lang="ts">
/**
 * El lienzo de anotación, encima de la región elegida.
 *
 * **Es también el archivo que se guarda.** El canvas se arma en el tamaño real
 * de la captura —no en el de la pantalla— y lo que se entrega es este mismo mapa
 * de bits, así que la vista previa no puede diferir del resultado: son la misma
 * composición. Es la razón por la que la imagen de fondo se dibuja acá adentro y
 * no se deja ver la de abajo: los efectos que tapan necesitan leer píxeles, y
 * sólo pueden leer los del canvas en el que están.
 */
import { onMounted, ref, watch } from 'vue';
import type { Anotacion } from '@/tools/anotacion';
import type { Lienzo } from '@/tools/lienzo';
import { pintar } from '@/tools/pintar';
import type { Region } from '@/tools/region';

const props = defineProps<{
	region: Region;
	lienzo: Lienzo;
	fondo: string;
	anotaciones: Anotacion[];
	/** La que se está dibujando ahora mismo, todavía sin cerrar. */
	enCurso: Anotacion | null;
}>();

const canvas = ref<HTMLCanvasElement | null>(null);
const imagen = ref<HTMLImageElement | null>(null);

/**
 * El tamaño del canvas: la región, en píxeles **de esta pantalla**.
 *
 * Los de verdad, no los de la captura compuesta. Las dos escalas coinciden salvo
 * cuando hay otro monitor de más escala, y ahí la de la captura diría el doble:
 * el archivo saldría al doble de tamaño e interpolado, distinto del que entrega
 * la misma captura sin anotar.
 */
function medida() {
	return {
		ancho: Math.max(1, Math.round(props.region.ancho * props.lienzo.escalaPropiaX)),
		alto: Math.max(1, Math.round(props.region.alto * props.lienzo.escalaPropiaY)),
	};
}

function componer() {
	const lienzoHtml = canvas.value;
	const img = imagen.value;
	if (!lienzoHtml || !img) return;

	const { ancho, alto } = medida();
	// Asignar el tamaño limpia el canvas, así que se hace siempre antes de
	// dibujar y no sólo cuando cambia.
	lienzoHtml.width = ancho;
	lienzoHtml.height = alto;

	const ctx = lienzoHtml.getContext('2d');
	if (!ctx) return;

	// El pedazo de la captura que corresponde a esta región. La imagen tiene
	// todas las pantallas, así que el origen de esta salida va sumado.
	//
	// El rectángulo de origen va en píxeles **de la captura** y el de destino en
	// los de esta pantalla: `drawImage` se encarga de la diferencia. Son el mismo
	// número salvo que otro monitor tenga más escala, y ahí lo que corresponde es
	// justamente achicar lo que la composición había estirado.
	ctx.drawImage(
		img,
		(props.lienzo.salida.x + props.region.x) * props.lienzo.escalaX,
		(props.lienzo.salida.y + props.region.y) * props.lienzo.escalaY,
		Math.max(1, Math.round(props.region.ancho * props.lienzo.escalaX)),
		Math.max(1, Math.round(props.region.alto * props.lienzo.escalaY)),
		0,
		0,
		ancho,
		alto
	);

	// Lo dibujado va en píxeles del canvas, o sea en los de esta pantalla.
	const encuadre = {
		origen: { x: props.region.x, y: props.region.y },
		escalaX: props.lienzo.escalaPropiaX,
		escalaY: props.lienzo.escalaPropiaY,
	};
	const todas = props.enCurso ? [...props.anotaciones, props.enCurso] : props.anotaciones;
	pintar(ctx, todas, encuadre);
}

/** El PNG de lo que se ve, que es lo que se entrega. */
async function exportar(): Promise<Uint8Array> {
	const lienzoHtml = canvas.value;
	if (!lienzoHtml) throw new Error('no hay lienzo');
	const blob = await new Promise<Blob | null>((listo) => lienzoHtml.toBlob(listo, 'image/png'));
	if (!blob) throw new Error('no se pudo componer la imagen');
	return new Uint8Array(await blob.arrayBuffer());
}

defineExpose({ exportar });

onMounted(() => {
	const img = new Image();
	// Ya está en la caché del WebView: la pantalla la usa de fondo desde que se
	// abrió, y se comprobó que cargara antes de mostrarla.
	img.onload = () => {
		imagen.value = img;
		componer();
	};
	img.src = props.fondo;
});

watch(
	() => [props.region, props.anotaciones, props.enCurso, props.lienzo],
	() => componer(),
	{ deep: true }
);
</script>

<template>
	<canvas
		ref="canvas"
		class="pointer-events-none absolute"
		:style="{
			left: `${props.region.x}px`,
			top: `${props.region.y}px`,
			width: `${props.region.ancho}px`,
			height: `${props.region.alto}px`,
		}"
	></canvas>
</template>
