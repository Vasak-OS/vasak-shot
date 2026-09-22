# vasak-shot

Las capturas de pantalla de VasakOS.

## Cómo se usa

| Cómo | Qué hace |
|---|---|
| `Impr Pant` | Abre el selector: arrastrá una zona, o apretá Intro para toda la pantalla |
| `Mayús+Impr Pant` | Guarda toda la pantalla directo, sin interfaz |
| `vasak-shot` | Lo mismo que la tecla: abre el selector |
| `vasak-shot --pantalla` | Guarda y copia todas las pantallas juntas, imprime la ruta y sale |
| `vasak-shot --salida DP-1` | Lo mismo con una sola, la que se llame así |
| `vasak-shot --retardo 5` | Espera cinco segundos y recién ahí captura |

**Soltar el botón entrega la captura.** El gesto es uno solo: se arrastra sobre lo
que se quiere y al levantar el dedo ya está. Qué es «entregar» lo decide la
preferencia de abajo —sin tocar nada, guardar y copiar—, y con «esperar» soltar
no entrega: congela la selección y deja decidir con los botones.

**Un clic sobre una ventana la captura entera.** Al pasar el puntero se resalta
la que está debajo; el clic la elige con su borde exacto, sin encuadrarla a ojo,
y entrega como cualquier otra selección. Un clic **fuera** de toda ventana elige
**la pantalla entera**, que es lo que el resaltado venía mostrando: el recuadro
y el nombre del monitor aparecen antes del clic, así que no hay nada que se
guarde sin haberse anunciado. Antes ese clic no hacía nada, justamente porque no
había forma de saber qué iba a pasar.

Lo que gana bajo el puntero va de lo más chico a lo más grande: región
arrastrada, después ventana, después pantalla.

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

Los tiradores aparecen con la barra de anotación en «Ajustar»: con una
herramienta tomada, arrastrar adentro dibuja.

Cruzar un borde más allá del opuesto **da vuelta** la selección en lugar de
dejarla en cero, y nada se puede sacar de la pantalla.

Mientras se elige o se corrige aparece una **lupa** con la captura ampliada y las
coordenadas: es lo que permite parar en el píxel que se quiere y no en el de al
lado. Se da vuelta sola contra los bordes, que es justo donde más se la necesita.

## Anotar

Con la selección hecha aparece la barra: diez herramientas —recuadro, elipse,
línea, flecha, lápiz, resaltador, texto, pasos numerados, difuminar y pixelar—,
el color, el grosor y **deshacer** (`Ctrl+Z`, y `Ctrl+Mayús+Z` para rehacer).
El recuadro y la elipse van rellenos o al aire.

El color, el grosor y el relleno se recuerdan **por herramienta**: elegir rojo
para la flecha no cambia el del resaltador.

El primer botón de la barra suelta la herramienta y vuelve al modo en el que se
ajusta la selección, que es donde están los tiradores.

**Las anotaciones son datos hasta el final.** Lo que se dibuja es una lista de
formas con su color y su grosor, no píxeles: de ahí sale deshacer, y de ahí sale
que el archivo se componga una sola vez.

### Tapar lo que no se comparte

Difuminar y pixelar no dibujan encima: **transforman los píxeles de abajo**. Eso
trae dos cosas que importan.

**El orden se respeta.** Una flecha dibujada antes de un difuminado que la cruza
queda difuminada; dibujada después, queda nítida encima.

**Y lo tapado queda tapado de verdad.** Las intensidades por omisión están
elegidas mirando un renglón de terminal —que es lo que de verdad se comparte— y
no una cara en una foto: un difuminado flojo sobre texto grande se lee igual, y
un mosaico chico sobre texto monoespaciado se puede revertir. Hay pruebas que lo
comprueban midiendo el contraste que queda.

## Por qué el archivo lo compone el selector y no Rust

Lo que se guarda es **el mismo mapa de bits que se ve**: el canvas de la
anotación se arma en el tamaño real de la captura y ése es el archivo.

El plan era al revés —mandar la lista de anotaciones y pintarlas en Rust, para no
cruzar megabytes por el IPC— y no se sostiene, por dos razones que aparecen al
escribirlo:

- El canvas suaviza los bordes de todo lo que dibuja y un dibujante de píxeles a
  mano no. Cada recuadro y cada flecha saldrían distintos de como se vieron.
- El texto necesita una fuente y sus métricas. Hacer coincidir la tipografía del
  navegador con la de un rasterizador de Rust es una pelea que no se gana.

Y tapar una zona **exige** que la vista previa y el archivo coincidan: la única
manera de que coincidan siempre es que sean la misma composición.

El costo es el que se quería evitar, acotado: el PNG cruza el IPC por el canal
**crudo** de Tauri —el cuerpo entero del pedido son los bytes, no un JSON con una
lista de números— y sólo cuando hay algo dibujado. Sin anotaciones sigue viajando
la región y nada más, que son cuatro números.

## Preferencias

La rueda dentada de la barra abre dos cosas:

- **Qué hace soltar el botón**: guardar y copiar (lo de siempre), sólo guardar,
  sólo copiar, o **esperar** y decidir con los botones. La última es la que hace
  falta para anotar la captura o ajustar la selección, porque las dos cosas pasan
  después de soltar.
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
Medido en esta máquina, en release, con `cargo run --release --example tiempos`:

```
escribir el PNG    46 ms   1920x1080
recortar            6 ms   300x200
```

**Lo que ese ejemplo no mide es la conversación con el compositor**, y no por
olvido: el permiso de captura es por ejecutable y lo tiene `/usr/bin/vasak-shot`,
así que un binario compilado en un árbol de trabajo falla antes de pedir el primer
buffer. Los números de acá son los de los pasos que sí se pueden medir en
cualquier lado — y son los que importan para el argumento, porque ocurren enteros
*después* de que el instante ya está congelado.

Que sean decenas de milisegundos y no unidades tampoco cambia nada: la ventana
tarda mil o dos mil. Y de paso la selección se hace sobre una imagen quieta en
lugar de sobre una pantalla que sigue cambiando debajo.

(Los números que había acá antes —136 ms de captura, 295 ms de recorte— eran los
de `grim` y de una compilación de depuración. Un README con números que nadie
puede reproducir es peor que uno sin números: suenan verosímiles y se toman
decisiones con ellos.)

## Por qué los píxeles los toma esta aplicación y no `grim`

**No porque `grim` esté mal**: hace esto bien y de ahí salió la forma de hacerlo.
El motivo es de permisos. El escritorio limita qué programas pueden pedirle al
compositor los protocolos que ven la sesión —`permisos-globales`, de
`vasak-wayfire-plugins`— y esa lista es **por ejecutable**. Con `grim` adentro,
cualquier programa capturaba la pantalla entera con dos líneas:

```sh
#!/usr/bin/env bash
grim "$1"
```

Medido: capturó 290 950 bytes sin estar en ninguna lista. El permiso de compartir
pantalla se saltaba llamando a la herramienta que sí lo tenía. Que los píxeles los
tome esta aplicación es lo que dejó sacar a `grim` de la lista sin que el
escritorio pierda las capturas — y capturar a mano desde una terminal dejó de
andar, a propósito.

Así que `zwlr_screencopy_manager_v1` se habla acá: copia **una salida** por vez a
un buffer de memoria compartida, y componer es cosa nuestra. La posición y el
tamaño lógicos salen de `xdg_output` y no de `wl_output`, cuyos números están en
píxeles del dispositivo y no contemplan la escala.

El recorte también es propio, y por dos razones que valen igual: una sola captura
en lugar de dos —lo que se guarda es exactamente el instante que se vio— y porque
la geometría del layout de salidas no coincide con las coordenadas de la pantalla
en la que se está eligiendo.

El portapapeles sí es prestado: va por **`wl-copy`**, que es el que sabe declarar
`image/png` en Wayland. El de Tauri maneja texto, y lo que hace útil una captura
es poder pegarla como imagen.

### Cuando los monitores tienen escalas distintas

El lienzo se compone en la escala **mayor** de todas las pantallas: bajar todo a
la menor tiraría píxeles que existen. Eso deja a las de menor escala estiradas
adentro de la imagen compuesta, así que de ahí no salen los recortes: de cada
pantalla que hubo que estirar se guardan aparte **sus píxeles tal como llegaron**,
y una selección que cabe entera en una de ellas se recorta de ésos. Sin eso, una
captura de 400×300 en el monitor al 100 % se guardaba como un archivo de 600×400
interpolado, con el selector diciendo 400×300.

De las que no se estiraron no se guarda nada: el lienzo ya tiene sus píxeles de
verdad, y una segunda copia serían decenas de megabytes repetidos. Con un solo
monitor —el caso de todos los días— no se guarda ninguno.

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

`wl-clipboard` para el portapapeles, `gtk-layer-shell` para la superficie que tapa
todo, y `libnotify` para el aviso al guardar y para la cuenta regresiva del
retardo.

Para capturar no hace falta ninguna: el crate de Wayland trae su propia
implementación del protocolo y **no** enlaza `libwayland-client` — comprobado con
`readelf -d` sobre el binario.

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

## Elegir la pantalla, y esperar antes de disparar

Con más de un monitor, la barra de abajo los lista por su nombre de conector y
sus medidas. Elegir uno lo entrega entero, **sin mover el puntero hasta él**: la
captura ya los contiene a todos, así que la otra pantalla no hay que ir a
buscarla. Por línea de órdenes es `--salida NOMBRE`, y un nombre que no existe
falla diciendo cuáles hay en lugar de guardar cualquier cosa.

El **retardo** es para lo que se cierra al perder el foco: un menú abierto, un
desplegable, un globo de ayuda. Elegir 3, 5 o 10 segundos **cierra el selector**
y vuelve a capturar cuando se cumplen — durante la espera no hay nada de esta
aplicación en pantalla, que es el punto. Quien captura de nuevo es otro proceso,
que arranca, espera y recién ahí toma los píxeles: el orden de siempre, píxeles
antes que ventana.

La cuenta regresiva va en una notificación, que es lo único que puede aparecer
sin robar el foco. **Se apaga un segundo antes del disparo**, porque si no
saldría adentro de la foto.

## Lo que falta

- **Subir la captura** a algún lado y dar el enlace, para compartirla sin adjuntar
  el archivo.
- **El aviso de guardado no lleva a ninguna parte**: dice dónde quedó, pero no
  abre ni la carpeta ni el archivo.

## Licencia

GPL-3.0-or-later.
