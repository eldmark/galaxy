# 03. Planetas y superficie

El reto: `lab6` dibuja una esfera fija de `sphere.obj`. Para aterrizar hace falta detalle de metros en un planeta de kilómetros. Una malla fija no alcanza, así que cada planeta tiene **dos representaciones** que se intercambian por distancia sin que se note.

## Representaciones por distancia

| Distancia a la superficie | Representación | Triángulos aprox. |
|---|---|---|
| > 4 radios | Icosfera subdividida (nivel 3-4) con el mismo shader y la silueta deformada | 1k a 5k |
| 4 radios a ~0.3 radios | Quadtree cube-sphere con pocos niveles | 5k a 40k |
| < 0.3 radios | Quadtree profundo + instancias + normal map | 40k a 150k |

La transición lejos→cerca usa el **mismo `height()`**, así la silueta coincide y el cambio es invisible. Para evitar parpadeo se dibuja el chunk nuevo en cuanto está listo y se retira la icosfera cuando todos los chunks de la cara visible existen.

## Terreno en cube-sphere

### Mapeo

Un cubo con 6 caras, cada una un quadtree. Un punto `(cara, u, v)` con `u,v ∈ [-1,1]` se proyecta a la esfera con la corrección `x' = x·sqrt(1 - y²/2 - z²/2 + y²z²/3)` para evitar celdas deformadas en las esquinas. Ventajas: sin singularidad en los polos (a diferencia de `uv()` de `lab6`), UV naturales para mapas de normales y vecindad sencilla.

### Altura (deformación por ruido)

```
height(dir, planeta) = radio * (1 + amplitud * fbm_ridged(dir * frecuencia, semilla_planeta))
```

- Una sola función pura, determinista y barata: la usan la malla, el normal map, la colisión y el scatter.
- La normal se obtiene por **diferencias centrales** de `height` (o gradiente analítico del ruido) para tener la normal exacta aun a LOD bajo.
- Cada planeta define amplitud, frecuencia, octavas y nivel del mar (Verdalia aplana bajo el nivel del mar; Fornax usa crestas; Nix mesetas suaves; Mutante cuantiza la altura en escalones de 1/16).

### Selección de chunks (generación procedural de geometría)

```
visitar(nodo):
  si fuera del frustum o tras el horizonte -> descartar (stats)
  error_px = tamaño_geométrico_nodo * foco / distancia
  si error_px > UMBRAL y nivel < MAX -> visitar los 4 hijos
  si no -> dibujar chunk(nodo)
```

- Cada chunk es una malla de **17x17 vértices** generada al vuelo (subdivisión procedural de una cara del cubo) y desplazada con `height`.
- Se agregan **faldones** (borde que baja ~2 % del tamaño del chunk) para tapar grietas entre niveles distintos; es más simple que armonizar bordes y suficiente para una demostración.
- **Horizonte**: un chunk se descarta si está bajo el plano tangente que pasa por la cámara y es tangente al planeta.
- **Caché**: los chunks se guardan en un mapa por clave `(cara, nivel, x, y)`; se generan en hilos de `rayon` sin bloquear el cuadro; mientras se genera se muestra el padre. Límite de memoria (p. ej. 512 chunks, descarta los menos recientes).
- `MAX` nivel ≈ 8 → vértices a ~1 m en un planeta de 3 km. UMBRAL ≈ 6 px, ajustable en vivo.

### Mapa de normales (8 pts)

- Se genera una textura tangente **tileable de 256x256** por planeta con ruido (relieve fino: grava, hielo agrietado, basalto).
- En el fragment se calcula el marco TBN a partir de las caras del cube-sphere (T = ∂p/∂u, B = ∂p/∂v, N = normal del terreno), se muestrea con filtrado bilineal y se perturba la normal antes de la iluminación.
- Se compara con/sin mapa de normales con una tecla (`N`).
- El bump analítico y el parallax de Fornax en `lab6` se mantienen como mejora específica de ese planeta.

## Cuerpos del sistema

| Cuerpo | Radio | Órbita | Rota | Extras |
|---|---|---|---|---|
| Ígnea (sol) | 8 km | fija en el centro | lenta | emisivo, corona, no aterrizable |
| Verdalia | 3.5 km | 35 km | 1 vuelta/90 s | luna **real** (radio 0.9 km) orbitando; nubes en capa propia; aterrizable |
| Siroco | 6 km | 80 km | rápida | anillo como malla (anillo plano, prueba z); aterrizaje sobre una capa de nubes jugable |
| Mutante | 2 km | 55 km | 1 vuelta/60 s | shader pixel; terreno cuantizado; aterrizable |
| Nix | 2.5 km | 105 km | lenta | hielo, cristales; aterrizable |
| Fornax | 3 km | 22 km | 1 vuelta/70 s | lava animada, bump+parallax; aterrizable |
| Dunara | 3 km | 45 km | 1 vuelta/80 s | dunas por ondas de viento, gusano gigante (P2); aterrizable |
| Nublar | 3.5 km | 65 km | 1 vuelta/85 s | selva densa, niebla, dinosaurios (P2); aterrizable |
| Egrin | 2.5 km | 130 km | 1 vuelta/100 s | venas luminosas pulsantes, relieve "respira"; aterrizable |

Dunara, Nublar y Egrin son de segunda prioridad (`07-assets-y-temas.md`); la altura de cada uno usa su propia configuración de `height()`: dunas = suma de senos alineados al viento + ruido; selva = colinas suaves con ríos por ruido de crestas; Egrin = Voronoi con canales.

Los valores son punto de partida; viven en una tabla en `world/system.rs` y se ajustan jugando. Posición de órbita circular inclinada: `pos(t) = R_orb * (cos(ωt + φ), sin(i)·sin(ωt+φ), sin(ωt+φ))`, siempre función del tiempo.

## Shaders (`shading/`)

Se portan de `lab6` a un trait con `f32`:

```rust
pub struct FragIn { pub p_local: Vec3, pub normal: Vec3, pub uv: (f32, f32),
                    pub view_dir: Vec3, pub light_dir: Vec3, pub time: f32,
                    pub altitude: f32, pub detail: f32 /* 0..1, controla octavas */ }
pub trait PlanetShader: Sync { fn shade(&self, f: &FragIn) -> Rgb; }
```

- `detail` baja el número de octavas de `fbm` con la distancia: es parte del **LOD** de sombreado y ahorra tiempo en pantalla completa.
- Los shaders que dependen del tablero de vida (`life.rs`) lo reciben como recurso de solo lectura que se actualiza en el paso de simulación, no por píxel.
- Atmósfera (`sky.rs`): para cada píxel sin geometría o con geometría lejana, se interseca el rayo con una esfera de atmósfera y se mezcla un color de dispersión simple. Al descender, el fondo pasa de negro con estrellas al azul del cielo según la altitud. Sin cambio de escena: es la misma función evaluada con altitud distinta.

## Objetos en superficie

### Modelos (4 tipos propios)

Se generan en Rust (`src/bin/generate_assets.rs`) como `assets/models/*.obj`; el LOD simplificado se construye en `src/scatter/models.rs`:

| Tipo | Planeta | Notas |
|---|---|---|
| Árbol | Verdalia | tronco + copa facetada; variación por escala/rotación (sigue contando como un solo tipo) |
| Espira de obsidiana / roca | Fornax | silueta angular, emisivo en la base |
| Cristal de hielo | Nix | prisma alargado con brillo especular |
| Edificio / monumento pixelado | Mutante | cubos apilados, referencia a la cara de vaca |

Además, nave propia (`ship.obj`) y una plataforma de aterrizaje en Verdalia como punto de interés. El catálogo completo, con presupuestos de triángulos, prioridades y el flujo de modelado, está en `07-assets-y-temas.md`.

### Vegetación independiente por planeta

Cada planeta define su propia **tabla de especies** (`Species`: modelo, densidad, altitud, pendiente máxima, ruido, escala, distancias de LOD). El algoritmo de distribución es único; los parámetros no. Verdalia tiene bosques, Nublar selva con helechos, Dunara casi nada, Nix cristales, Fornax espiras, Egrin raíces luminosas. LOD de vegetación: modelo alto → bajo → billboard → nada, y hierba en cruces cerca de la nave. Detalle en `07`.

### Distribución procedural (10 pts)

```
Por cada chunk hoja (nivel >= N_SCATTER):
  rng = hash(semilla_global, planeta, cara, nivel, x, y)
  rejilla de celdas dentro del chunk con jitter dado por rng
  para cada celda: dir -> densidad = bioma(dir) * fbm(dir)
                    si noise < densidad y pendiente < máx y altura sobre mar:
                       instancia (posición local, escala, giro_vertical)
```

- Misma semilla → mismas posiciones, independientemente del orden en que se generen los chunks (el hash depende de la clave del chunk, no de un estado global).
- Se prueba con un test que genera dos veces y compara, y con `--seed` distinto que cambia el resultado.
- Máximo de instancias por chunk y distancia máxima de dibujo (LOD de instancias: modelo alto → bajo → descartar).

### Posición y orientación sobre la superficie curva (5 pts)

- Posición: `dir * height(dir)` en espacio local del planeta.
- Orientación: eje Y local = normal del terreno (del gradiente de `height`), giro aleatorio alrededor de esa normal.
- Como la instancia está en espacio local del planeta, la matriz de mundo es `M_planeta(t) * M_local`: **rota y orbita con el planeta**. Se demuestra con `--time-scale 50` y un árbol visible de lado.
