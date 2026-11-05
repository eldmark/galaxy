# 05. Hitos, pruebas y entrega

Orden pensado para que **cada hito deje el programa ejecutable** y para cumplir lo eliminatorio lo antes posible. El enunciado recomienda: primero un planeta y una nave que se vean y se muevan, luego un acercamiento estable, y a partir de ahí ampliar.

## Hitos

| Hito | Entregable | Criterio de aceptación | Puntos que habilita |
|---|---|---|---|
| **M0** Proyecto base | `cargo new`, dependencias, `platform.rs`, bucle con reloj real, captura PNG | Ventana negra a 60 FPS; `--shot` genera PNG sin ventana | - |
| **M1** Pipeline 3D | `Mat4`, cámara, proyección, z-buffer `1/z`, OBJ con normales, esfera iluminada | Esfera girando con perspectiva correcta y sin artefactos de profundidad; test de matrices | requisito |
| **M2** Sistema mínimo | Sol, Verdalia con órbita y rotación por tiempo, nave en Blender, shader de Verdalia, estrellas | La órbita avanza igual a distinto FPS (`--time-scale`); shader propio visible | requisito, 10-20 |
| **M3** Nave y vuelo | Física a paso fijo, controles, cámara con mirada libre | Respuesta idéntica con `--fps-cap 20` y `144`; acercarse y alejarse libremente | 8 aceleración |
| **M4** Rendimiento base | Tiles + rayon, sombreado diferido, frustum culling, contadores | ≥30 FPS en 960x540; HUD muestra conteos; **cumple todos los requisitos eliminatorios** | 7 culling |
| **M5** Cuerpos y LOD lejano | Los 5 planetas + luna, icosfera subdividida con niveles | Se distinguen a simple vista; LOD de icosfera con contadores | 50 planetas, 10 subdivisión |
| **M6** Terreno cube-sphere | `height`, quadtree, chunks, faldones, caché, horizonte | Descenso continuo de 1R a 20 m sin saltos ni grietas visibles; sin tirones | 10 deformación, 8 LOD |
| **M7** Superficie viva | Modelos, `scatter`, normal map, atmósfera | Misma semilla → mismas posiciones; instancias correctas al rotar | 20 modelos, 10+5 scatter, 8 normales |
| **M8** Navegación avanzada | Gravedad, colisión, modos ORBITA/DESCENSO/POSADA | Se puede orbitar, descender, tocar suelo y despegar sin transición | 10 gravedad, 4 colisión, 8 control local |
| **M9** Autopiloto | Botón de Tour y de aterrizaje automático, animación de aterrizaje y despegue (tren, flare, polvo) | Clic en el botón: recorre 5 planetas sin intervención, aterriza con animación y despega, sin atravesar terreno; cualquier mando lo interrumpe | extra / video |
| **M10** Contenido P1 y entrega | Mutante, Dunara, Nublar con vegetación propia; shaders restantes, HUD, ayuda, README, video, lista de puntos | Todo verificado; ejecutable desde cero con `cargo run --release` | 24 shaders + 8 animado + 10 presentación |
| **M11** Contenido P2 (opcional) | Egrin, dinosaurio, gusano, estación, carguero | Cada uno aislado detrás de una bandera; no afecta el rendimiento base | presentación |

Notas de orden:

- Los hitos M5 a M8 se pueden repartir en paralelo entre integrantes una vez terminado M4.
- Si el tiempo se acorta, **M6 es el hito crítico** (el aterrizaje depende de él). M9 depende de M8.

## Trabajo individual: prioridades

Todo lo hace una persona, así que se trabaja por capas de prioridad y no se pasa a la siguiente hasta que la anterior está estable:

| Capa | Contenido | Hitos |
|---|---|---|
| **P0** (núcleo, ya supera 100 pts) | Pipeline, sistema con 4-5 planetas, vuelo, culling, terreno con LOD, 4 modelos, scatter, gravedad, colisión, botón de Tour con aterrizaje animado | M0 a M9 |
| **P1** | Mutante, Dunara y Nublar con vegetación propia, atmósfera, hierba | M10 |
| **P2** | Egrin, dinosaurio, gusano, estación, carguero, tráfico | M11 (solo si sobra) |

Ritmo recomendado: terminar M4 pronto (cumple lo eliminatorio) y dejar un ejecutable siempre funcional. Cada hito termina con captura y commit. El documento `07-assets-y-temas.md` agenda los assets por fase para no bloquearse modelando.

## Pruebas

### Automáticas (`cargo test`)

- `math`: multiplicación de matrices, inversa, cuaterniones.
- `raster`: triángulo conocido cubre píxeles esperados; regla top-left sin huecos ni doble pintado entre triángulos vecinos; z-buffer elige el más cercano; recorte por plano cercano.
- `geometry`: cube-sphere es continua en aristas (puntos de caras vecinas coinciden); icosfera tiene 20·4ⁿ caras y vértices en la esfera unidad.
- `terrain`: `height` determinista; normal por diferencias ≈ analítica; chunks vecinos coinciden en el borde ± faldón.
- `scatter`: misma semilla → misma lista; semilla distinta → lista distinta; instancias con pendiente > máx no existen.
- `ship`: integración con distintos `dt` converge a la misma posición; gravedad reduce la distancia; colisión nunca deja la nave bajo el suelo.
- `autopilot`: intercepción predice dentro de un margen la posición de llegada del planeta.
- `world`: la posición orbital depende solo del tiempo (no de la cuenta de cuadros).

### Visuales (`--shot`)

Captura reproducible por configuración fija (`--shot out.png --body fornax --alt 300 --seed 7 --time 4`). Se guardan en `screenshots/` y se pegan al README. Sirve para comparar antes/después sin abrir ventana.

### Rendimiento

- `--bench`: ruta fija de cámara (alejado → acercamiento → rasante), imprime FPS medio/mínimo, triángulos por cuadro y ms por etapa.
- Tabla de comparación para la demostración: **sin culling/sin LOD** vs **con** (triángulos y ms). Se guarda en el README.
- `perf record` + flamegraph en M4 y M6 para decidir qué optimizar (como en `cargar-modelo`).

## Guion del video (entrega)

Duración 2-3 min. Se genera con el Tour (`--tour --record frames/`, equivalente a pulsar el botón) y `scripts/videos.sh` (de `lab6`). Resolución de grabación 960x540.

1. Sol y nave en el espacio, HUD visible, controles breves (0:00 a 0:15).
2. Crucero hacia Verdalia, se ve la órbita y la rotación con aceleración progresiva (0:15 a 0:40).
3. Acercamiento; paso a ORBITA y vuelta completa al planeta (0:40 a 1:10).
4. Descenso continuo hasta la superficie, árboles y normal map, aterrizaje (1:10 a 1:50).
5. Despegue y alejamiento; vista rápida de los otros planetas y Siroco con su anillo (1:50 a 2:30).
6. Pantalla de estadísticas: culling/LOD activados vs desactivados (2:30 a 2:50).

## Entrega

README con:

- Cómo compilar y ejecutar (`cargo run --release`, requisitos de CMake y compilador C para raylib).
- Tabla de controles (la de `04-navegacion-aterrizaje-autopiloto.md`).
- Integrantes y contribución concreta de cada uno.
- **Lista de características del menú que se solicitan evaluar y dónde verlas** (tecla/ubicación/archivo), basada en la tabla de `01-requisitos-y-puntos.md`.
- Recursos externos y asistencia de IA declarados (se pide en el enunciado).
- Parámetros editables para la demostración en vivo (`config.toml` o teclas).

Cada integrante debe poder explicar su contribución y modificar un parámetro en vivo; hay que ensayar cambiar radio, velocidad orbital, semilla y umbral de LOD.
