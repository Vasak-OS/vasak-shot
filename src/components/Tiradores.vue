<script setup lang="ts">
/**
 * Los ocho tiradores de la selección.
 *
 * Se dibujan en porcentajes del rectángulo que los contiene, así que no hay
 * ninguna cuenta acá: mover la selección los mueve solos. Lo único que este
 * componente decide es qué borde agarra cada uno, y eso lo dice su rol.
 */
import { anclaDe, ROLES, type Rol } from '@/tools/region';

const emitir = defineEmits<{ tomar: [rol: Rol, evento: MouseEvent] }>();

/**
 * El cursor de cada tirador.
 *
 * Es lo que avisa que se puede agarrar **antes** de intentarlo. Sin esto, ocho
 * cuadraditos sobre una imagen no se leen como algo que se arrastra.
 */
const CURSORES: Record<Rol, string> = {
	tl: 'nwse-resize',
	t: 'ns-resize',
	tr: 'nesw-resize',
	r: 'ew-resize',
	br: 'nwse-resize',
	b: 'ns-resize',
	bl: 'nesw-resize',
	l: 'ew-resize',
};
</script>

<template>
	<button
		v-for="rol in ROLES"
		:key="rol"
		type="button"
		:aria-label="rol"
		class="-translate-x-1/2 -translate-y-1/2 absolute size-3 rounded-full border border-ui-bg bg-primary"
		:style="{
			left: `${anclaDe(rol).x * 100}%`,
			top: `${anclaDe(rol).y * 100}%`,
			cursor: CURSORES[rol],
		}"
		@mousedown.stop.prevent="emitir('tomar', rol, $event)"
	></button>
</template>
