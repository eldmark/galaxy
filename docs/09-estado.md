# 09. Estado del proyecto

Última actualización: 2026-11-05. Entrega: 20/11/2026.

## Hecho (fase rúbrica-100, integrado en `main`)

| Área | Estado | Dónde |
|---|---|---|
| Pipeline de software (M/V/P, z-buffer 1/w, tiles con rayon, sombreado diferido) | listo, 16 pruebas | `src/raster/` |
| Frustum culling + contadores | listo | `raster::render_scene`, HUD F3/F4 |
| LOD de esferas por tamaño en pantalla (6 niveles, histéresis) | listo | `geometry::lod`, `app::Scene` |
| Subdivisión procedural (icosfera) y deformación por ruido | listo (la deformación aún no se usa en los planetas) | `geometry::sphere` |
| Shaders propios: sol, Verdalia, Siroco, Mutante, Nix, Fornax, anillo, nave, cielo | listo; 5 animados | `src/shading/` |
| Órbitas y rotación por tiempo, gravedad, colisión, modos Cruise/Orbit/Descent/Landed | listo | `src/world/`, `src/ship/` |
| Aceleración y giro progresivos con paso fijo 120 Hz | listo, probado con distintos dt | `ship::stepper`, `ship::physics` |
| Tour automático y aterrizaje automático con botones | listo (aterriza y despega en Fornax, Verdalia, Mutante, Nix) | `ship::autopilot`, `app.rs` |
| HUD propio (fuente bitmap, botones, estadísticas, ayuda) | listo | `src/hud.rs` |
| Ventana e entrada (raylib solo en `platform.rs`) | listo | `src/platform.rs` |
| Nave modelada por nosotros (script paramétrico) + 3 props | listo | `assets/` |

## Medición (release, 16 hilos, 960x540, recorrido del Tour)

`cargo run --release -- --bench --frames 600`

| Configuración | ms medio | triángulos entrada | triángulos dibujados | draw calls descartadas |
|---|---|---|---|---|
| culling + LOD | 8.7 | 2 000 | 832 | 3 |
| sin culling, sin LOD | 10.9 | 124 572 | 17 771 | 0 |

Nota: el cuello de botella ahora es el sombreado de los planetas grandes (~6.5 ms), no la geometría. El fondo estelar bajó de 27 ms a 1.2 ms al pasarlo a un cubemap precalculado. Falta medir en un equipo de 4 núcleos.

## Verificación

- `cargo test`: 74 pruebas pasan, 3 ignoradas (benchmarks y prueba con ventana).
- Cuadros del Tour revisados a mano (`--record`): alineación, órbita, descenso y posada en Verdalia con el HUD correcto.
- Ventana interactiva abre y corre sin errores (8 s con `--tour`). **Pendiente**: probar los controles manuales con teclado y ratón.

## Pendiente para cubrir la rúbrica con margen

1. Probar controles manuales y ajustar la sensación (sensibilidad, asistencia, frenado).
2. Animar el tren de aterrizaje (la nave es una sola malla; hay que separar los grupos `gear_*` al cargar el OBJ).
3. Usar la deformación por ruido en al menos un planeta (para que cuente como logro visible) y mostrar la comparación LOD.
4. Selección de destino con teclas 1-6 (HUD ya las documenta; no está conectado).
5. Efecto de cámara libre (mirada) separado del giro de la nave.
6. README de entrega: controles, lista de características a evaluar y dónde verlas, uso de IA y recursos.
7. Video del recorrido completo (`--record` + ffmpeg, o grabación de pantalla).

## Siguiente fase (si el núcleo está estable, ~14/11)

Terreno cube-sphere con LOD, objetos en superficie con distribución por semilla, mapa de normales, luego Mutante/Dunara/Nublar con vegetación propia (`03`, `07`).
