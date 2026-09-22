<script setup lang="ts">
/**
 * Las preferencias del selector.
 *
 * Vive **adentro** del selector porque es la única ventana que esta aplicación
 * abre. Y el campo de la carpeta se escribe a mano, sin diálogo del sistema: el
 * selector es una superficie de `zwlr_layer_shell` en la capa de superposición
 * que tapa todo, y un diálogo modal lanzado desde ahí aparece detrás o no
 * aparece.
 */
import { useI18n } from '@vasakgroup/tauri-plugin-i18n';
import { ref, watch } from 'vue';
import {
	ACCIONES,
	type Ajustes,
	type AlSoltar,
	claveDe,
	type Guardado,
} from '@/tools/preferencias';

const { t } = useI18n();

const props = defineProps<{ ajustes: Ajustes; error: string }>();
const emitir = defineEmits<{
	guardar: [cambios: Guardado];
	cerrar: [];
}>();

/**
 * Lo guardado, con un campo cambiado.
 *
 * El backend escribe el archivo entero, así que mandar sólo lo que cambió
 * borraría lo demás: elegir una acción no puede llevarse puesta la dirección
 * para subir.
 */
function conCambio(parte: Partial<Guardado>): Guardado {
	return {
		alSoltar: props.ajustes.alSoltar,
		carpeta: props.ajustes.carpeta,
		subirA: props.ajustes.subirA,
		subirCampo: props.ajustes.subirCampo,
		...parte,
	};
}

/** Lo escrito en los campos de subida, que tampoco es lo guardado. */
const urlEscrita = ref(props.ajustes.subirA ?? '');
const campoEscrito = ref(props.ajustes.subirCampo ?? '');
const sinAplicarSubida = ref(false);

watch(
	() => [props.ajustes.subirA, props.ajustes.subirCampo],
	([url, campo]) => {
		if (sinAplicarSubida.value) return;
		urlEscrita.value = url ?? '';
		campoEscrito.value = campo ?? '';
	}
);

function aplicarSubida() {
	sinAplicarSubida.value = false;
	emitir(
		'guardar',
		conCambio({
			subirA: urlEscrita.value.trim() || null,
			subirCampo: campoEscrito.value.trim() || null,
		})
	);
}

/**
 * Lo que hay escrito en el campo, que no es lo guardado hasta que se aplica.
 *
 * Separado a propósito: escribir media ruta y cerrar el panel no puede mandar
 * las capturas a esa media ruta.
 */
const escrita = ref(props.ajustes.carpeta ?? '');

/** Si lo que hay en el campo todavía no se aplicó. */
const sinAplicar = ref(false);

// Al guardar, el backend devuelve cómo quedó —`~` expandido, espacios
// recortados—, y el campo tiene que mostrar eso y no lo que se tecleó.
//
// **Salvo que se esté escribiendo.** Elegir una acción también guarda, y esa
// respuesta trae la carpeta: sin esta guarda, tocar una opción a mitad de
// escribir una ruta borraba lo tecleado sin que nadie lo hubiera pedido.
watch(
	() => props.ajustes.carpeta,
	(carpeta) => {
		if (sinAplicar.value) return;
		escrita.value = carpeta ?? '';
	}
);

function elegir(accion: AlSoltar) {
	// Un clic en una opción es una decisión tomada: se guarda sola. La carpeta
	// no, porque a mitad de escribir una ruta no hay ninguna decisión todavía —
	// por eso viaja la guardada y no la del campo.
	emitir('guardar', conCambio({ alSoltar: accion }));
}

function aplicarCarpeta() {
	sinAplicar.value = false;
	emitir('guardar', conCambio({ carpeta: escrita.value }));
}

function restablecerCarpeta() {
	sinAplicar.value = false;
	escrita.value = '';
	emitir('guardar', conCambio({ carpeta: null }));
}
</script>

<template>
	<section
		data-sin-arrastre
		class="-translate-x-1/2 absolute bottom-24 left-1/2 w-[30rem] max-w-[calc(100vw-2rem)] rounded-corner border border-ui-border bg-ui-surface/95 p-4 text-tx-main shadow-lg"
	>
		<header class="mb-3 flex items-center justify-between">
			<h2 class="font-medium text-sm">{{ t('shot.preferencias') }}</h2>
			<button
				type="button"
				class="rounded-corner px-2 py-1 text-tx-muted text-xs hover:bg-ui-bg"
				@click="emitir('cerrar')"
			>
				{{ t('shot.cerrar') }}
			</button>
		</header>

		<fieldset class="mb-4">
			<legend class="mb-1.5 text-tx-muted text-xs">{{ t('shot.alSoltarTitulo') }}</legend>
			<div class="flex flex-col gap-1">
				<label
					v-for="accion in ACCIONES"
					:key="accion"
					class="flex cursor-pointer items-center gap-2 rounded-corner px-2 py-1 text-sm hover:bg-ui-bg"
				>
					<input
						type="radio"
						name="al-soltar"
						class="accent-primary"
						:value="accion"
						:checked="accion === props.ajustes.alSoltar"
						@change="elegir(accion)"
					/>
					{{ t(claveDe(accion)) }}
				</label>
			</div>
		</fieldset>

		<div>
			<label class="mb-1.5 block text-tx-muted text-xs" for="carpeta">
				{{ t('shot.carpeta') }}
			</label>
			<div class="flex items-center gap-1.5">
				<input
					id="carpeta"
					v-model="escrita"
					type="text"
					spellcheck="false"
					class="min-w-0 flex-1 rounded-corner border border-ui-border bg-ui-bg px-2 py-1 font-mono text-xs"
					:placeholder="props.ajustes.carpetaEfectiva"
					@input="sinAplicar = true"
					@keydown.enter.prevent="aplicarCarpeta()"
				/>
				<button
					type="button"
					class="rounded-corner bg-primary px-3 py-1 text-sm text-tx-on-primary hover:brightness-110"
					@click="aplicarCarpeta()"
				>
					{{ t('shot.aplicar') }}
				</button>
				<button
					type="button"
					class="rounded-corner px-2 py-1 text-tx-muted text-xs hover:bg-ui-bg"
					:disabled="props.ajustes.carpeta === null"
					:class="{ 'opacity-40': props.ajustes.carpeta === null }"
					@click="restablecerCarpeta()"
				>
					{{ t('shot.restablecer') }}
				</button>
			</div>

			<!-- Dónde van ahora mismo, siempre a la vista: la preferencia puede
			     estar vacía y aun así las capturas van a algún lado, y ese lado
			     depende del idioma de la instalación y de si ya había capturas
			     guardadas con el nombre anterior. -->
			<p class="mt-1.5 break-all font-mono text-[10px] text-tx-muted">
				{{ props.ajustes.carpetaEfectiva }}
			</p>

			<p v-if="props.error" class="mt-1.5 text-status-error text-xs">{{ props.error }}</p>
		</div>

		<!-- Subir es publicar, así que este campo empieza vacío y se queda vacío
		     hasta que alguien escriba adónde. Sin dirección no hay botón de
		     subir en la barra. -->
		<div class="mt-4 border-ui-border border-t pt-3">
			<label class="mb-1.5 block text-tx-muted text-xs" for="subir-a">
				{{ t('shot.subirA') }}
			</label>
			<div class="flex items-center gap-1.5">
				<input
					id="subir-a"
					v-model="urlEscrita"
					type="text"
					spellcheck="false"
					placeholder="https://"
					class="min-w-0 flex-1 rounded-corner border border-ui-border bg-ui-bg px-2 py-1 font-mono text-xs"
					@input="sinAplicarSubida = true"
					@keydown.enter.prevent="aplicarSubida()"
				/>
				<input
					id="subir-campo"
					v-model="campoEscrito"
					type="text"
					spellcheck="false"
					:placeholder="t('shot.subirCampo')"
					:aria-label="t('shot.subirCampo')"
					class="w-24 min-w-0 rounded-corner border border-ui-border bg-ui-bg px-2 py-1 font-mono text-xs"
					@input="sinAplicarSubida = true"
					@keydown.enter.prevent="aplicarSubida()"
				/>
				<button
					type="button"
					class="rounded-corner bg-primary px-3 py-1 text-sm text-tx-on-primary hover:brightness-110"
					@click="aplicarSubida()"
				>
					{{ t('shot.aplicar') }}
				</button>
			</div>
			<p class="mt-1.5 text-[10px] text-tx-muted">{{ t('shot.subirAviso') }}</p>
		</div>
	</section>
</template>
