<script setup lang="ts">
/**
 * La pregunta antes de subir.
 *
 * **Subir es publicar**, y eso no se parece a copiar al portapapeles ni a dejar
 * un archivo en una carpeta: el enlace lo abre cualquiera que lo tenga, y la
 * captura lleva encima lo que hubiera en la pantalla. Por eso hay una pregunta
 * en el medio y por eso dice a qué servidor va — que es el dato que cambia la
 * respuesta.
 *
 * Cuánto tiempo va a quedar ahí no lo sabemos y se dice así: el servidor lo
 * elige quien lo configuró, y prometer un plazo que no controlamos sería peor
 * que no decir nada.
 */
import { useI18n } from '@vasakgroup/tauri-plugin-i18n';
import { interpolar } from '@/tools/interpolar';

const { t } = useI18n();

const props = defineProps<{ servidor: string; trabajando: boolean }>();
const emitir = defineEmits<{ subir: []; cancelar: [] }>();
</script>

<template>
	<section
		data-sin-arrastre
		class="-translate-x-1/2 absolute bottom-24 left-1/2 w-[28rem] max-w-[calc(100vw-2rem)] rounded-corner border border-ui-border bg-ui-surface/95 p-4 text-tx-main shadow-lg"
	>
		<h2 class="mb-1.5 font-medium text-sm">{{ t('shot.subirTitulo') }}</h2>
		<p class="text-tx-muted text-xs">
			{{ interpolar(t('shot.subirDestino'), props.servidor) }}
		</p>
		<p class="mt-1.5 text-tx-muted text-xs">{{ t('shot.subirAviso') }}</p>

		<div class="mt-3 flex items-center justify-end gap-1.5">
			<button
				type="button"
				class="rounded-corner px-3 py-1.5 text-sm text-tx-muted hover:bg-ui-bg"
				@click="emitir('cancelar')"
			>
				{{ t('shot.cancelar') }}
			</button>
			<button
				type="button"
				class="rounded-corner bg-primary px-3 py-1.5 font-medium text-sm text-tx-on-primary hover:brightness-110 disabled:opacity-50"
				:disabled="props.trabajando"
				@click="emitir('subir')"
			>
				{{ t('shot.subir') }}
			</button>
		</div>
	</section>
</template>
