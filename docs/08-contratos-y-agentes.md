# 08. Contratos entre módulos y reparto de agentes (fase rúbrica-100)

Entrega: **20/11/2026**. Objetivo de esta fase: cubrir los puntos de la rúbrica sin terreno (planetas, shaders, navegación, culling, LOD, presentación ≈ 137 pts). Terreno cube-sphere, vegetación y assets P1/P2 vienen después (`03`, `07`).

## Reglas

- Raylib solo en `platform.rs`. Nada de dibujo 3D, modelos, cámara 3D ni shaders GPU de raylib.
- Cada agente es dueño de **sus archivos** y no edita los de otros. Los tipos compartidos (`math.rs`, y las definiciones de tipos en `raster/mod.rs`, `geometry/mod.rs`, `world/mod.rs`, `ship/mod.rs`, `platform.rs`, `hud.rs`) son el contrato: se pueden **añadir** campos/funciones, nunca cambiar firmas existentes; si hace falta, se reporta.
- Los stubs `todo!("nombre")` marcan lo que le toca a cada agente. Cada uno trabaja en su worktree/rama y deja `cargo test` en verde.
- Convenciones: mano derecha, cámara mira -Z, +Y arriba, `Mat4` row-major con vectores columna, profundidad guardada como `1/w`, colores de shader en 0..1, tiempo en segundos de simulación.

## Agentes

| Agente | Archivos propios | Entrega |
|---|---|---|
| `raster` | `src/raster/**` (puede crear submódulos), `src/png.rs` | `render_scene`: vértices, clipping near, frustum culling por esfera, backface culling, tiles + rayon, z-buffer 1/w, estadísticas; guardar PNG; pruebas |
| `shading` | `src/shading/**` | 6 shaders portados de `lab6` (sol, Verdalia, Siroco, Mutante, Nix, Fornax) + `life`, campo de estrellas/cielo, `detail` como LOD de octavas; pruebas |
| `geometry` | `src/geometry/**` | OBJ con normales/uv/color, icosfera, `displace`, niveles de LOD de esfera, anillo de Siroco; pruebas |
| `nav` | `src/world/**`, `src/ship/**` | Órbitas/rotación por tiempo, paso fijo, aceleración progresiva, gravedad, colisión, modo local de órbita, cámara de nave, piloto automático (tour); pruebas |
| `ui` | `src/platform.rs`, `src/hud.rs` | Ventana raylib, entrada, presentar framebuffer, fuente bitmap, botón, HUD, ayuda |
| `assets` | `assets/**` | `ship.obj` propio (y `.blend`/script fuente); más adelante árbol, roca, cristal |

El coordinador integra en `main.rs` (bucle, CLI `--shot`, `--bench`, `--tour`).
