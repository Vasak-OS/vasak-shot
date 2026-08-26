# vasak-shot

Las capturas de pantalla de VasakOS.

## Cómo se usa

| Cómo | Qué hace |
|---|---|
| `Impr Pant` | Abre el selector: arrastrá una zona, o apretá Intro para toda la pantalla |
| `Mayús+Impr Pant` | Guarda toda la pantalla directo, sin interfaz |
| `vasak-shot` | Lo mismo que la tecla: abre el selector |
| `vasak-shot --pantalla` | Guarda y copia toda la pantalla, imprime la ruta y sale |

En el selector: **Intro** guarda y copia, **Ctrl+C** copia sin guardar, **Esc**
cancela. Sin arrastrar nada, Intro captura la pantalla entera — es el camino más
corto para el caso más común.

Las capturas van a `~/Imágenes/Capturas`, o el equivalente en el idioma de la
instalación: la carpeta la elige `user-dirs.dirs`, y no es «Pictures» en todas las
máquinas.

## El orden importa, y es al revés de lo que parece

**Primero los píxeles, después la ventana.** Crear una ventana de Tauri bajo
demanda tarda entre uno y dos segundos —medido en el escritorio y en el selector de
acentos—, y una herramienta de capturas que abre ventana y *después* captura pierde
justo el momento que se quería guardar: el menú que estaba abierto se cerró, el
cursor se movió, la notificación desapareció.

Así que se captura al arrancar y la ventana muestra ese cuadro **congelado**.
Medido en esta máquina:

```
capturar   136 ms   1920x1080
recortar   295 ms   300x200
```

Los 430 ms ocurren enteros *después* de que el instante ya está en disco, así que
la lentitud de la interfaz deja de importar. Y de paso la selección se hace sobre
una imagen quieta en lugar de sobre una pantalla que sigue cambiando debajo.

## Lo que no se reimplementa

Los píxeles los toma **`grim`**. Ya habla `zwlr_screencopy` correctamente, maneja
varias salidas con sus escalas, y viene instalado. Reescribirlo sería rehacer la
parte difícil para llegar al mismo lugar.

El recorte sí es propio, y **no** con `grim -g`, por dos razones: una sola captura
en lugar de dos —lo que se guarda es exactamente el instante que se vio— y porque
la geometría de `grim` está en coordenadas del layout de salidas, que no siempre
coinciden con las de la pantalla: en la máquina de desarrollo `-g "0,0 400x300"`
contesta «did not intersect with any outputs».

El portapapeles va por **`wl-copy`**, que es el que sabe declarar `image/png` en
Wayland. El de Tauri maneja texto, y lo que hace útil una captura es poder pegarla
como imagen.

## Tres cosas que costaron encontrar

**El layer-shell tiene que iniciarse antes de que la ventana se mapee.** Con
`visible: true`, `init_layer_shell` aborta con «assertion '!gtk_widget_get_mapped'
failed» y cada llamada siguiente avisa «GtkWindow is not a layer surface». El
resultado es una ventana de 800×600 con decoración en el medio de la pantalla en
lugar de una superficie que tapa todo. La ventana arranca oculta y se muestra
después.

**`wl-copy` se demoniza y hereda la salida estándar.** Con los descriptores
heredados, quien lo llamó se queda esperando que el pipe se cierre — y no se cierra
mientras el portapapeles tenga la imagen. Desde una terminal parece que la
herramienta se colgó; desde un atajo, que nunca terminó. Su salida va a `null`.

**`assetProtocol` está desactivado por omisión**, así que `convertFileSrc` queda
bloqueado por la política de contenido y la imagen no carga. Y eso no se ve: la
ventana es transparente, así que se veía el escritorio **vivo** debajo y la
selección parecía funcionar mientras en realidad se elegía sobre una pantalla que
seguía moviéndose. Ahora la imagen se verifica antes de usarla como fondo, y si no
carga se dice.

## Dependencias

`grim` para capturar, `wl-clipboard` para el portapapeles, `gtk-layer-shell` para
la superficie que tapa todo, y `libnotify` para el aviso al guardar.

## Lo que falta

- **Capturar una ventana** eligiéndola con el puntero. Necesita
  `zwlr_foreign_toplevel_manager` para saber dónde está cada una.
- **Anotar**: flechas, recuadros, difuminar una zona antes de compartir.
- **Retardo** antes de capturar, para poder abrir un menú.

## Licencia

GPL-3.0-or-later.
