# vasak-shot

Las capturas de pantalla de VasakOS.

## Cómo se usa

| Cómo | Qué hace |
|---|---|
| `Impr Pant` | Abre el selector: arrastrá una zona, o apretá Intro para toda la pantalla |
| `Mayús+Impr Pant` | Guarda toda la pantalla directo, sin interfaz |
| `vasak-shot` | Lo mismo que la tecla: abre el selector |
| `vasak-shot --pantalla` | Guarda y copia toda la pantalla, imprime la ruta y sale |

**Soltar el botón entrega la captura.** El gesto es uno solo: se arrastra sobre lo
que se quiere y al levantar el dedo ya está. Qué es «entregar» lo decide la
preferencia de abajo —sin tocar nada, guardar y copiar—, y con «esperar» soltar
no entrega: congela la selección y deja decidir con los botones. Un clic sin
arrastrar no entrega nada.

**Un clic sobre una ventana la captura entera.** Al pasar el puntero se resalta
la que está debajo; el clic la elige con su borde exacto, sin encuadrarla a ojo.

El resto del selector sigue ahí para lo que no es un arrastre: **Intro** guarda y
copia —y sin arrastrar nada captura la pantalla entera, que es el camino más corto
para el caso más común—, **Ctrl+C** copia sin guardar, **Esc** cancela.

### Corregir lo elegido

Con la preferencia en «esperar», la selección queda ahí y se puede acomodar antes
de entregarla:

| Cómo | Qué hace |
|---|---|
| Los ocho tiradores | Mueven ese borde. Los de esquina, los dos que tocan |
| Arrastrar adentro | Corre la selección entera sin cambiarle el tamaño |
| Flechas | Corren la selección un píxel; con **Mayús**, diez |
| **Ctrl**+flechas | Mueven el borde de abajo a la derecha, o sea redimensionan |

Cruzar un borde más allá del opuesto **da vuelta** la selección en lugar de
dejarla en cero, y nada se puede sacar de la pantalla.

Mientras se elige o se corrige aparece una **lupa** con la captura ampliada y las
coordenadas: es lo que permite parar en el píxel que se quiere y no en el de al
lado. Se da vuelta sola contra los bordes, que es justo donde más se la necesita.

## Preferencias

La rueda dentada de la barra abre dos cosas:

- **Qué hace soltar el botón**: guardar y copiar (lo de siempre), sólo guardar,
  sólo copiar, o **esperar** y decidir con los botones. La última es la que va a
  hacer falta cuando se pueda anotar la captura o ajustar la selección, porque las
  dos cosas pasan después de soltar.
- **En qué carpeta se guarda.** Se escribe a mano: el selector es una superficie
  de capa que tapa todo, y un diálogo de sistema lanzado desde ahí aparece detrás
  o no aparece. Abajo del campo está siempre a la vista dónde van a ir a parar las
  capturas ahora mismo.

Las preferencias viven en `~/.config/vasak-shot/preferencias.json`. Un archivo que
no está o que está roto **no** es un error: quedan los valores de siempre y la
captura sigue su camino.

Sin elegir carpeta, las capturas van a `~/Imágenes/ScreenShots`, o el equivalente
en el idioma de la instalación: la carpeta madre la elige `user-dirs.dirs`, y no es
«Pictures» en todas las máquinas. **Si ya había capturas en la carpeta anterior
—`Capturas`— se sigue guardando ahí**: cambiar el nombre por omisión no puede
partir en dos lo que alguien ya tenía guardado.

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

## Dónde está cada ventana

Se lo pregunta al **IPC de wayfire**, que es el único que lo sabe.
`zwlr_foreign_toplevel_manager` —que es lo que este archivo suponía que hacía
falta— informa títulos y estados, **no rectángulos**. El socket de wayfire no
pasa por `permisos-globales`: es un socket Unix anunciado por una variable de
entorno, no un global de Wayland, y esta aplicación ya lo usaba para saber dónde
está el puntero.

Dos cosas que hay que saber para que las coordenadas den:

- **La geometría de una vista es relativa a su salida, no al layout.** Medido en
  una sesión de dos monitores apilados: una ventana a pantalla completa en el de
  abajo —que empieza en `y = 1080`— contesta `y = 0`. Hay que sumarle el origen
  de su salida, que sale de `list-outputs`.
- **Las ventanas de los otros escritorios están corridas un ancho de pantalla**,
  así que recortar contra la salida las deja afuera sin tener que preguntar en
  qué espacio de trabajo está cada una.

Se pregunta **al arrancar**, junto con la captura y por la misma razón: lo que se
señala tiene que coincidir con lo que la imagen congelada muestra.

Sin wayfire la lista queda vacía, no se resalta nada y queda el arrastre.

## Lo que falta

- **Anotar**: flechas, recuadros, difuminar una zona antes de compartir.
- **Retardo** antes de capturar, para poder abrir un menú.

## Licencia

GPL-3.0-or-later.
