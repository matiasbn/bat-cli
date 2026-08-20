# Despliegue automático de dependencias EVM a Miro

Plan de diseño para `feature/evm-miro-auto-deploy`.
Fuente: OpenAPI oficial de Miro (`miroapp/api-clients` → `spec.json`, v2.0) + docs de la plataforma.

---

## 1. Qué encontré en la API (y qué está mal hoy)

### 1.1 El bug que obliga a mover las capturas a mano

`api::create_image_from_device` (`src/batbelt/miro/image.rs`) sube el PNG **sin la parte `data`**:

```rust
let form = Form::new().part("resource", some_file);   // ← falta "data"
```

El schema `UploadFileFromDevice` acepta dos partes:

| parte      | tipo                 | contenido                                            |
|------------|----------------------|------------------------------------------------------|
| `resource` | binary (≤ 6 MB)      | el archivo                                            |
| `data`     | `application/json`   | `title`, `altText`, `position`, `geometry`, `parent` |

Ejemplo oficial (`x-readme` del endpoint):

```bash
curl -X POST "https://api.miro.com/v2/boards/{board_id}/images" \
  -H "Authorization: Bearer $TOKEN" \
  -F 'data={"position":{"x":3000,"y":3000}};type=application/json' \
  -F "resource=@image.png;type=image/png"
```

Es decir: **la imagen se puede crear ya posicionada, dimensionada y dentro del frame en una sola llamada**.
Hoy hacemos POST (cae en 0,0 sin padre) + PATCH de posición → 2 llamadas Level 2 por captura, y el tamaño
queda en el que salga de silicon. Bonus: el `mime_str` está en `text/plain` en vez de `image/png`.

### 1.2 Sistema de coordenadas (lo crítico para el layout)

- Items sueltos en el board: `x`/`y` = **centro del item**, relativo al **centro del board**.
- Items con `parent.id` (dentro de un frame): `x`/`y` = **centro del item**, relativo a la
  **esquina superior izquierda del frame**.
- Conversión frame → board: `x_board = (frame.x - frame.width/2) + x_local`.

O sea, dentro del frame trabajamos en un sistema tipo pantalla (0,0 arriba-izquierda), que es
exactamente lo que necesita el algoritmo de layout. Todo el cálculo se hace en coordenadas locales.

### 1.3 Tamaño de la imagen: `FixedRatioGeometry`

`geometry` de una imagen acepta `width` **o** `height`, nunca ambos (mantiene el aspect ratio).
Entonces: renderizamos el PNG, leemos sus píxeles localmente, y mandamos sólo `width` →
**sabemos el bounding box exacto en board units antes de subir nada**. Esa es la pieza que hoy falta
para poder posicionar automáticamente.

`image::image_dimensions(path)` (crate `image`, ya es dependencia) devuelve `(w,h)` sin decodificar
el PNG completo.

### 1.4 Conectores

`POST /v2/boards/{board_id}/connectors`, `ConnectorCreationData`:

```jsonc
{
  "startItem": { "id": "...", "snapTo": "bottom" },   // o "position": {"x":"50%","y":"100%"}
  "endItem":   { "id": "...", "snapTo": "top" },
  "shape": "straight" | "elbowed" | "curved",          // default: curved
  "style": {
    "strokeColor": "#2d9bf0", "strokeWidth": "2.0",
    "strokeStyle": "normal" | "dotted" | "dashed",
    "startStrokeCap": "none", "endStrokeCap": "stealth",
    "color": "#1a1a1a", "fontSize": "14", "textOrientation": "aligned"
  },
  "captions": [{ "content": "<p>calls</p>", "position": "50%" }]   // máx 20
}
```

Dos hallazgos importantes:

1. **Los frames NO se pueden usar como extremo de un conector** (literal en el spec:
   *"Note that Frames are not supported at the moment"*). Conectamos siempre imagen→imagen. No es
   problema para este feature.
2. `position` es un **offset relativo en porcentaje** (`"0%"`–`"100%"`, 0,0 = arriba-izquierda del
   item), no coordenadas absolutas. Admite **cualquier punto del item**, también interiores — no
   está restringido a los bordes ni a los 4 puntos de `snapTo`. El `ConnectorOptions` actual pasa
   strings tal cual, lo que sólo funciona si el caller ya manda porcentajes — hoy nadie lo usa
   (todo va con `None` → `snapTo: auto`).

### 1.5 Rate limiting (hoy no existe manejo)

| Nivel   | Créditos/llamada | Req/min |
|---------|------------------|---------|
| Level 1 | 50               | 2000    |
| Level 2 | 100              | 1000    |
| Level 3 | 500              | 200     |
| Level 4 | 2000             | 50      |

Presupuesto global: **100.000 créditos/min**, por combinación usuario+app. Crear frame, imagen y
conector son todos **Level 2** (bulk: Level 2 *por item*). Headers: `X-RateLimit-Limit`,
`X-RateLimit-Remaining`, `X-RateLimit-Reset` (epoch). En 429 hay que hacer backoff exponencial.

Un entrypoint con 25 dependencias = 1 frame + 25 imágenes + ~30 conectores ≈ 56 llamadas ≈ 5.600
créditos. Cabe cómodo, pero desplegando `--all` sobre un repo grande hay que respetar el límite.

### 1.6 Bulk create (opcional, buena optimización)

`POST /v2/boards/{board_id}/items/bulk` con `multipart/form-data`:
- `resources`: hasta **20** archivos binarios
- `data`: un archivo JSON con un array de 20 objetos, **en el mismo orden** que `resources`

Es transaccional (si falla uno, no se crea ninguno). Mismo costo en créditos, pero 1 roundtrip en
vez de 20. **A verificar empíricamente**: la doc del formato JSON confirma `position` y `geometry`
por item, pero no es explícita sobre `parent`.

### 1.7 Límites duros a respetar

- Imagen: **≤ 6 MB** y **≤ 8192 px** por lado.
- Frame: mínimo 100×100. Sin máximo documentado (a verificar con el PoC).
- Board: hasta 100.000 objetos, pero el rendimiento se degrada sobre ~1.000–5.000.

---

## 2. Qué cambia en el diseño actual

### 2.1 Los frames de hoy quedan chicos

```rust
pub const MIRO_FRAME_WIDTH: u64  = 5600;
pub const MIRO_FRAME_HEIGHT: u64 = 2600;
pub const MIRO_BOARD_COLUMNS: i64 = 5;
pub const MIRO_INITIAL_X: i64 = 4800;
```

Un frame fijo de 5600×2600 con capturas a font 16–28 obliga a que todo quede minúsculo o se salga.
**Propuesta**: el frame se dimensiona *después* de calcular el layout — `frame.width = bbox.width +
2·PADDING`, `frame.height = bbox.height + 2·PADDING + TITLE_BAND`. Con anchos objetivo de
1200–1600 por captura y 4–6 niveles, un frame típico queda en ~8.000–20.000 de ancho.

La grilla de 5 columnas fijas también se va: los frames se colocan en filas empaquetando por ancho
real, leyendo primero los frames existentes del board para no solapar.

Las constantes viejas se conservan sólo para el flujo SVM/code-overhaul legacy, marcadas como tales.

### 2.2 Módulos nuevos

```
src/batbelt/miro/
  client.rs        ← NUEVO: MiroClient (reqwest::Client compartido, retry/429, semáforo)
  image.rs         ← create con data{parent,position,geometry}; una sola llamada
  connector.rs     ← tipado: SnapTo, ConnectorShape, ConnectorStyle
  frame.rs         ← create con geometry calculada; grilla dinámica
  layout.rs        ← NUEVO: algoritmo puro, sin I/O, testeable
src/batbelt/evm/miro/
  auto_deploy.rs   ← NUEVO: orquestador por entrypoint
```

`layout.rs` no conoce Miro: recibe `Vec<Node{id, w, h, depth}>` + `Vec<Edge{from,to}>` y devuelve
posiciones + bbox. Eso permite testearlo con `cargo test` sin tocar la red.

---

## 3. Pipeline por entrypoint

```
metadata → grafo → PNGs locales → medir → escalar → layout → frame → imágenes → conectores → persistir
```

### 3.1 Anclaje del conector a la línea exacta de la llamada

Esta es la pieza que hace que el conector no apunte "a la captura" sino **a la línea**.

**Por qué se puede**: `ItemConnectionCreationData.position` es un `RelativeOffset` — porcentajes
sobre el item, `x=0%,y=0%` es la esquina superior izquierda, `x=100%,y=100%` la inferior derecha.
Al ser una **fracción**, es independiente de la escala que le demos a la imagen con
`geometry.width`. Sólo necesitamos saber en qué fracción de la altura del PNG cae la línea N.

**Métricas exactas de silicon 0.5.2** (verificadas leyendo `formatter.rs` y con un test que
compara contra renders reales, `src/batbelt/silicon.rs::line_geometry_test`):

- `create_drawables` dibuja la línea `i` (0-based) en `get_line_y(i)`
- `get_line_y(i) = i * line_height + code_pad + code_pad_top`
- `line_height = font.get_font_height() + line_pad`, con `line_pad = 2`
- `code_pad = 25`
- `code_pad_top = 50` sólo si hay title bar; construimos con `window_controls(false)` y sin
  `window_title` → **0**
- `ShadowAdder` desplaza toda la imagen en `pad_vert = 10`

De ahí, en píxeles del PNG final:

```
line_height  = FontCollection::new(&[("Hack", font_size)]).get_font_height() + 2
first_line_y = PAD (10) + CODE_PAD (25) = 35
y_center(i)  = first_line_y + i * line_height + line_height / 2
altura total = n_lines * line_height + 2*CODE_PAD + 2*PAD
```

`FontCollection` y `get_font_height()` son públicos en silicon, así que **la geometría se calcula
sin renderizar nada** — depende sólo del tamaño de fuente. Ya está implementado como
`silicon::line_geometry(font_size) -> LineGeometry`, con `line_center_y` y
`line_center_fraction`. El test valida, para fuentes 16/20/28, que 20 líneas extra suman
exactamente `20 * line_height` y que la altura absoluta coincide con la fórmula cerrada.

**Del número de línea del código al índice de línea de la imagen**: `create_screenshot` antepone
`"// <path>\n\n"` cuando `include_path` está activo, o sea 2 líneas. Entonces, para una llamada en
la línea `L` del archivo y una captura que empieza en `start_line`:

```
line_index = (L - start_line) + if include_path { 2 } else { 0 }
y_frac     = line_center_fraction(line_index, png_height_px)
```

**Los dos extremos del conector**:

| extremo | item | ancla | significado |
|---------|------|-------|-------------|
| `startItem` | captura del **caller** | `{ x: "<x_frac fin de línea>%", y: "<y_frac de la línea>%" }` | interior: justo después del `;` de la línea que llama |
| `endItem`   | captura del **callee** | `{ x: "0%", y: "<y_frac de la línea de la firma>%" }` | borde izquierdo, a la altura de la firma |

La firma del callee es la primera línea de su captura → `line_index = 2` con `include_path`.

**Un conector por call site, no por par de funciones.** Si `_deposit` llama a `MathLib.mulDiv` en
tres líneas distintas, se dibujan tres conectores que salen de tres alturas distintas y llegan al
mismo callee. El `caption` lleva el número de línea (`L142`) para que se lea sin hacer zoom.

**De dónde sale el número de línea de la llamada**: hoy `resolve_evm_function_deps` resuelve las
dependencias recorriendo el cuerpo línea por línea, así que ya tiene el índice — sólo hay que dejar
de descartarlo. Se propaga como `CallSite { callee_id, line }` en vez de sólo `callee_id`. Lo mismo
aplica al lado SVM, donde `CallResolver` ya visita cada call site con `syn` (que expone `span()`,
con línea si está activo `proc-macro2/span-locations`; si no, se localiza por búsqueda de texto
sobre el cuerpo ya recortado).

**Ancla interior: al final de la línea que hace la llamada.**

`RelativeOffset` admite cualquier punto del item, no sólo los bordes ni los 4 puntos de `snapTo`.
Así que el conector nace **dentro de la captura**, en la línea de la llamada. Anclarlo exactamente
sobre el nombre de la función haría que la flecha tape el código, así que el ancla va **justo
después del último carácter de la línea** — a la derecha del `;`, sobre fondo vacío. Se ve de qué
línea sale sin ambigüedad y no se pisa nada.

Para eso hace falta la coordenada X, que se calcula igual de exacto que la Y. De
`create_drawables` + `get_left_pad`:

```
line_number_chars = floor(log10(total_lines + line_offset)) + 1
left_pad          = CODE_PAD (25) + 2·LINE_NUMBER_PAD (6) + text_len("0" × line_number_chars)
x_end(línea)      = PAD (10) + left_pad + text_len(línea con tabs expandidos a 4 espacios)
```

`FontCollection::get_text_len` es público, y `create_figure` usa `tab_width(4)`, así que se replica
exacto. Implementado como `silicon::line_end_x(font_size, show_line_number, total_lines,
line_offset, line_text)`.

Está verificado **contra los píxeles reales**: el test `test_line_end_x_matches_rendered_png`
renderiza una figura, escanea las filas de píxeles de cada línea buscando el último píxel que no es
el fondo Dracula, y compara. `line_end_x` devuelve la posición de avance del cursor (donde iría el
siguiente carácter), así que queda entre 0 y un ancho de carácter por delante del último píxel con
tinta — exactamente donde queremos el ancla.

El ancla final del `startItem`:

```
x_frac = (line_end_x(...) + GAP) / png_width_px      // GAP ≈ 1 carácter
y_frac = line_center_fraction(line_index, png_height_px)
```

Nada de `highlight_lines`: las capturas quedan limpias, el origen de la flecha ya identifica la
línea.

**Casos borde**:

- *La llamada viene de un modifier*, no del cuerpo → el ancla es la línea del modifier en la firma
  (`function foo() external onlyOwner {`), que sí está en la captura.
- *La línea cae fuera del rango capturado* (herencia, truncado) → `line_center_fraction` clampea a
  `[0,1]`; si el clamp se activa, se hace fallback a `snapTo: "right"` / `"left"`.
- *Dos callees anclados a la misma línea* → se desplaza el `y` unos décimos de punto porcentual por
  índice, para que los conectores no nazcan exactamente superpuestos.
- *Fuentes distintas por profundidad* → `line_geometry` se calcula por nodo con su propio
  `font_size`; no hay una constante global.

### 3.2 Tamaño del frame y distribución de las capturas

Todo se calcula en coordenadas locales al frame (origen arriba-izquierda), y **antes** de crear
nada en Miro. Constantes propuestas:

```
PADDING_X = 250     GUTTER_X = 450     TITLE_BAND = 150
PADDING_Y = 200     GUTTER_Y = 120
```

Sea `K` el número de capas, `n_k` los nodos de la capa `k`, y cada nodo `i` con box `(w_i, h_i)`
ya escalado (Fase 3):

```
maxw_k   = max(w_i)  para i en la capa k
totalh_k = Σ h_i + GUTTER_Y · (n_k − 1)

bbox_w = Σ_k maxw_k + GUTTER_X · (K − 1)
bbox_h = max_k totalh_k

frame_w = bbox_w + 2 · PADDING_X
frame_h = bbox_h + 2 · PADDING_Y + TITLE_BAND
```

Posición de cada nodo (centro, local al frame):

```
x_layer_k = PADDING_X + Σ_{j<k} (maxw_j + GUTTER_X)
x_i       = x_layer_k + w_i / 2                      // alineado a la izquierda de su capa

y_start_k = PADDING_Y + TITLE_BAND + (bbox_h − totalh_k) / 2      // capa centrada verticalmente
y_i       = y_start_k + Σ_{j<i} (h_j + GUTTER_Y) + h_i / 2
```

Ejemplo con el PoC (`Router.zapIn`, 5 capas, anchos 1800/1500/1200/1200/1200, la capa más alta con
4 nodos de ~700):

```
bbox_w  = 6900 + 1800 = 8700        frame_w ≈ 9200
bbox_h  = 4·700 + 3·120 = 3160      frame_h ≈ 3710
```

**Cuando una capa es muy alta**: si `totalh_k` supera `MAX_LAYER_H` (por defecto 12000), la capa se
parte en sub-columnas dentro de su propia banda de X (se ensancha `maxw_k` a `n_sub · (maxw_k +
GUTTER_X/2)`). Evita frames de 40.000 de alto cuando una función llama a 30 helpers.

El padding garantiza además que **ningún hijo queda fuera del frame**, que es la condición que Miro
exige para poder redimensionar un frame con contenido.

### 3.3 Colocación de frames a escala

El caso real no son 10 frames: son **cientos de entrypoints**, cada uno con decenas de capturas.
Por eso la grilla de 5 columnas existía. Un packing geométrico con test de colisión contra todo el
board es exactamente el enfoque que no escala — hay que descartarlo.

**Miro no ayuda**: no hay endpoint de "espacio libre" en la REST API. Existe
`miro.board.findEmptySpace({x, y, width, height, offset})`, pero es del **Web SDK**, corre dentro
de una app en el browser. Para un CLI que habla REST no sirve.

La solución es no preguntar nunca dónde hay espacio: **reservar una región y llevar un asignador
propio, persistido**.

#### a) Región reservada, una sola vez

En el primer despliegue se escanea el board una única vez (`GET /items?type=frame`) y se calcula:

```
origin_y = max(frame.y + frame.height/2 sobre los frames existentes) + REGION_MARGIN (5000)
origin_x = min(frame.x − frame.width/2)   // alineado a la izquierda de lo que ya hay
```

Ese origen se guarda en `EvmBatMetadata.miro.auto_region`. **Nunca más se vuelve a escanear el
board completo.** Toda la salida automática vive por debajo del contenido manual, así que un frame
hecho a mano no puede colisionar con uno generado.

#### b) Asignador de estantes (shelf packing), O(1) por frame

Los frames tienen tamaños distintos, así que una grilla de celdas uniformes desperdicia muchísimo
espacio cuando un entrypoint tiene 3 dependencias y otro 60. El shelf packing resuelve eso sin
tests de colisión:

```
estado persistido: { origin_x, origin_y, cursor_x, cursor_y, row_height }

place(frame_w, frame_h) -> (x, y):
    if cursor_x + frame_w > ROW_MAX_W:          // se acabó la fila
        cursor_x  = 0
        cursor_y += row_height + GUTTER
        row_height = 0
    x = origin_x + cursor_x + frame_w / 2       // Miro quiere el centro
    y = origin_y + cursor_y + frame_h / 2
    cursor_x  += frame_w + GUTTER
    row_height = max(row_height, frame_h)
```

- **O(1) por frame, cero llamadas a la API, cero tests de colisión.** Escala igual con 10 que con
  10.000 frames.
- **Imposible que se solapen por construcción**: dentro de una fila el cursor avanza el ancho
  completo; entre filas se baja la altura del frame más alto de la fila anterior.
- `ROW_MAX_W` es el equivalente moderno de `MIRO_BOARD_COLUMNS = 5`: en vez de "5 columnas fijas"
  es "una fila mide hasta X de ancho". Por defecto `5 · 12000 = 60000`, configurable con
  `--row-width`. Frames chicos entran 8 por fila, uno gigante ocupa la fila entero.
- **Determinista**: los entrypoints se ordenan por `contrato::función` antes de asignar, así dos
  corridas producen el mismo board.

#### c) Incremental e idempotente

Cada entrypoint guarda su celda asignada (`frame_id`, `x`, `y`, `w`, `h`). Un re-deploy:

- si el frame no cambió de tamaño → reusa la celda, sólo actualiza imágenes y conectores;
- si creció y **cabe en el hueco de su fila** (queda espacio hasta el siguiente frame) → se
  redimensiona in situ;
- si no cabe → se reasigna al final del asignador y su celda vieja queda como hueco.

Los huecos se acumulan sólo si se re-despliega mucho; `miro evm-auto-deploy --compact` vuelve a
correr el asignador sobre todos los frames registrados y hace PATCH de posiciones. No es necesario
en el uso normal.

#### d) Verificación barata contra frames manuales

Si alguien creó un frame a mano *dentro* de la región (posible, aunque la región está lejos), antes
de cada lote se hace **un** `GET /items?type=frame` y se filtran sólo los frames cuyo centro cae
dentro del bbox de la región y que no están en el registro de metadata. Si hay alguno, se avisa y
se ofrece bajar `origin_y` por debajo de él. Es una llamada por lote, no por frame.

### 3.4 Límites cuando el repo es grande

Con 200 entrypoints × ~25 capturas se llega a ~5.000 imágenes + ~6.000 conectores + 200 frames
≈ **11.000 objetos**. Está muy por debajo del límite duro de Miro (100.000) pero muy por encima
del umbral donde el board se vuelve lento (~5.000). Y en rate limit: 11.000 llamadas Level 2 =
1.100.000 créditos ≈ **11 minutos** sólo de límite, sin contar latencia.

Mitigaciones, todas en el mismo diseño:

- **Un board por contrato o por módulo** (`--board <url>`), en vez de uno por repo. La región
  reservada y el asignador son por board, así que esto sale gratis.
- **Despliegue por lotes reanudable**: como el asignador y los ids viven en metadata, se puede
  correr `--all` en tandas y retomar sin duplicar nada.
- **`--max-depth` / `--max-nodes`** por entrypoint, con un aviso explícito de lo que se recortó
  (nunca truncado silencioso).
- **Colapso de hojas compartidas**: utilidades puras como `MathLib.mulDiv`, que aparecen como
  callee de media docena de funciones, se dibujan una sola vez por frame y reciben varias flechas,
  en vez de una copia por caller.

### Fase 1 — Grafo

De `EntryPointMetadata` + `FunctionDependency.callees` (EVM). BFS desde el entrypoint, guardando
**cada call site con su número de línea** (`CallSite { caller_id, callee_id, line }`), no sólo el
par caller→callee:

- dedupe por `metadata_id`
- los modifiers entran como nodos (ya se resuelven hoy en `resolve_evm_function_deps`)
- **detección de ciclos**: si un callee ya está en el camino actual, es un back-edge → no se
  re-expande, y la arista se dibuja `strokeStyle: "dashed"` (recursión visible pero sin romper el DAG)
- `--max-depth N` para acotar árboles gigantes
- se excluyen nodos de `lib/` (`ContractMetadata.external == true`) salvo `--include-external`

### Fase 2 — Render y medición

Silicon genera un PNG por nodo en un tmpdir. Para cada uno: `image::image_dimensions` → `(w_px, h_px)`.
Nada se sube todavía. Si el PNG excede 8192 px o 6 MB → se re-renderiza con `font_size` menor
(y si aún así no cabe, se trunca el cuerpo con un marcador `…`).

### Fase 3 — Escalado a board units

Ancho objetivo por profundidad:

| profundidad | ancho objetivo | font silicon |
|-------------|----------------|--------------|
| 0 (entrypoint) | 1800        | 32           |
| 1              | 1500        | 26           |
| ≥ 2            | 1200        | 22           |

`scale = target_w / w_px` → `h_board = h_px · scale`. Se envía sólo `geometry.width = target_w`
(ratio fijo). A partir de acá cada nodo tiene un box `(w, h)` exacto **antes** de subir.

### Fase 4 — Layout (Sugiyama simplificado, left → right)

1. **Capas**: `layer(n) = longest path` desde la raíz (no el más corto). Con longest-path, una
   función compartida por el nivel 1 y el 3 cae en el 3 → **ninguna flecha apunta hacia atrás**.
2. **Orden dentro de la capa**: heurística de baricentro, 4 pasadas alternando, para minimizar
   cruces. El baricentro se pondera por la **altura de la línea de llamada** dentro del caller
   (ver §3.1): si `_deposit` llama a `_takeEntryFee` en su línea 3 y a `_mint` en su línea 12,
   `_takeEntryFee` se ordena arriba de `_mint`. Así los conectores salen casi horizontales.
3. **X**: `x(layer) = Σ(ancho máximo de capas anteriores) + GUTTER_X·layer`, con
   `GUTTER_X = 450` (espacio para los codos de los conectores y sus captions).
4. **Y**: dentro de cada capa los boxes se apilan verticalmente con `GUTTER_Y = 120`; cada capa
   se centra respecto de la más alta.
5. **Salida**: posición del centro de cada nodo en coordenadas locales al frame + bbox total.

**Por qué left→right y no top→bottom** (esto cambió respecto del borrador inicial): como el
conector nace en la línea exacta de la llamada dentro del caller, el punto de salida natural es el
**borde derecho a esa altura**, y el de entrada es el **borde izquierdo del callee** a la altura de
su firma. Con un layout vertical, anclar a una línea específica obligaría a que el conector salga
por el costado y baje, cruzándose con las capturas vecinas. Con left→right el conector es casi
recto y la línea de origen se lee de inmediato.

El costo es un frame más ancho: ~1500 por capa × 6 capas ≈ 12.000 de ancho, contra ~8.000 de alto
si una capa llega a 10 nodos. Es un rectángulo razonable.

### Fase 5 — Frame

`POST /frames` con `geometry {width, height}` ya calculada, `data.title = "EP · Contract.function"`,
`style.fillColor` según profundidad máxima. Posición en el board: se listan los frames existentes
(`GET /items?type=frame`, ya implementado en `MiroFrame::get_frames_from_miro`) y se busca el primer
hueco en una grilla de filas con gutter de 1000.

### Fase 6 — Imágenes

Una llamada por nodo, con todo resuelto:

```jsonc
data = {
  "title": "Vault._deposit",
  "position": { "x": <local_x>, "y": <local_y> },   // centro, relativo al top-left del frame
  "geometry": { "width": <target_w> },
  "parent":   { "id": "<frame_id>" }
}
```

Concurrencia limitada (4–6 en vuelo) vía el semáforo del `MiroClient`. Opcionalmente en lotes de 20
con bulk create si la verificación de `parent` sale bien.

### Fase 7 — Conectores anclados a la línea de la llamada

Ver §3.1 para el cálculo. Para cada **call site** (no para cada par de funciones):

```jsonc
{
  "startItem": { "id": <img_caller>, "position": { "x": "68.2%", "y": "37.4%" } },
  "endItem":   { "id": <img_callee>, "position": { "x": "0%",   "y": "8.1%"  } },
  "shape": "elbowed",
  "style": { "strokeWidth": "3", "strokeColor": <color por profundidad>,
             "strokeStyle": <"dashed" si es back-edge>, "endStrokeCap": "stealth" },
  "captions": [{ "content": "<p>L142</p>", "position": "15%" }]
}
```

### Fase 8 — Persistencia e idempotencia

`MiroFrameRef` se extiende con `nodes: Vec<{function_metadata_id, image_id, x, y, w, h}>` y
`connector_ids: Vec<String>`. Con eso un re-deploy puede: reusar el frame, hacer PATCH de las
imágenes que se movieron, borrar conectores obsoletos y crear sólo lo nuevo, en vez de duplicar todo.

---

## 4. Comando

```
bat-cli miro evm-auto-deploy [--entry-point <name> | --all] [--max-depth N] [--dry-run]
```

`--dry-run` imprime la tabla del layout (nodo, capa, x, y, w, h, aristas) y **no llama a Miro**.
Es la forma de iterar el algoritmo sin gastar rate limit ni ensuciar el board.

---

## 5. Verificación con el PoC (`evm-deps-poc/`)

Caso de prueba: `Router.zapIn` — profundidad 5, diamante en `MathLib.mulDiv`, cross-contract vía
interfaz y modifiers heredados.

A verificar empíricamente contra un board real:

1. Coordenadas de hijo relativas al top-left del frame (confirmar signo y origen).
2. Precisión visual del ancla por porcentaje (`position`) en imágenes muy altas: verificar que el
   redondeo a 2 decimales de porcentaje cae dentro de la línea correcta. Con una captura de 200
   líneas, 1 línea ≈ 0.5% → 2 decimales sobran; con 2000 líneas habría que truncar la captura.
3. Si el bulk create acepta `parent`.
4. Cuál es el tamaño máximo real de un frame.
5. Si crear una imagen fuera del bbox del frame la desancla o falla.

Tests unitarios (sin red) sobre `layout.rs`: capas por longest-path, cero aristas hacia arriba,
cero solapes de boxes, bbox coherente, estabilidad del orden.

---

## 6. Orden de implementación

| # | PR | Contenido |
|---|----|-----------|
| 0 | `silicon::line_geometry` | ✅ hecho: geometría de línea + test contra renders reales |
| 1 | `miro/client.rs` | cliente compartido, rate limiter, retry con backoff en 429, `Result` en vez de `.unwrap()` |
| 2 | `miro/image.rs` | parte `data` con parent/position/geometry, mime `image/png`; `deploy_screenshot_to_miro_frame` pasa de 2 llamadas a 1 |
| 3 | `miro/layout.rs` | algoritmo puro + tests |
| 4 | `evm/miro/auto_deploy.rs` | orquestador + comando + `--dry-run` |
| 5 | `miro/connector.rs` | tipado `RelativeOffset`/`SnapTo`/`shape`/estilo, anclaje por línea, colores por profundidad, dashed en back-edges |
| 6 | metadata | `MiroFrameRef` extendido, re-deploy idempotente |
| 7 | limpieza | constantes de frame legacy acotadas al flujo SVM |

Los PRs 1–3 no cambian comportamiento visible (el 2 sí: arregla el posicionamiento), así que se
pueden mergear antes de que el layout esté listo.

## 7. Riesgos

- **`parent` en bulk create**: si no lo soporta, quedan llamadas individuales (mismo costo en créditos, más latencia).
- **Frames enormes**: si Miro tiene un máximo no documentado, hay que fallback a varios frames por entrypoint o reducir anchos objetivo.
- **Funciones gigantes**: PNG > 8192 px → re-render con font menor o truncado.
- **Grafos muy anchos**: una capa con 40 nodos genera un frame de ~60.000 de ancho; mitigar con
  `--max-depth`, colapso de hojas repetidas (libs puras como `MathLib.mulDiv` pueden dibujarse una
  vez por capa en vez de una vez por caller) o wrap en varias filas por capa.
