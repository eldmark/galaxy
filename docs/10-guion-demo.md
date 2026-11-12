# 10. Guion para la demostración

La portada se abre con `cargo run --release`; `1` elige Juego y `2` elige Tour de observación. Cada modo muestra un tutorial antes de comenzar. `--tour` omite la portada para ir directo al recorrido automático. Conviene tener `F3` abierto y el proyecto ya compilado. Reserva unos siete minutos para mostrar las partes de la rúbrica y deja los cambios de código para el final.

| Tiempo | Qué mostrar | Qué explicar |
|---|---|---|
| 0:00 | `F1`, HUD y nave | El rasterizador, los shaders y el HUD trabajan en la CPU. Raylib presenta la textura y lee entradas. |
| 0:45 | Viaje con `1`-`6`, sistema y órbitas con `O` | El sol y los cinco planetas tienen órbitas y rotación en función del tiempo, sin integrar su posición cuadro por cuadro. |
| 1:30 | Verdalia, Fornax, Nix y Siroco | Cada cuerpo usa un shader propio. El tiempo de simulación anima nubes, lava, tormenta y sol. |
| 2:15 | Acercamiento a una superficie, `N` | La esfera deformada pasa a chunks cube-sphere. El mapa de normales añade detalle a la iluminación sin añadir geometría. |
| 3:00 | Objetos con `--seed 1` y `--seed 7`, o una vista de `--surface-shot` | Árboles, espiras, cristales y edificios se colocan de forma reproducible. Se orientan con la normal del relieve y giran junto con el planeta. |
| 3:45 | `F3`, alternar `F4` y `F5` | `F4` cambia el descarte por frustum. `F5` cambia el detalle de esferas, terreno, objetos y shaders. Comparar triángulos, hojas e instancias en el panel. |
| 4:45 | `W`, `Shift`, `Ctrl`, `G`, `X` | La nave acelera de forma progresiva con simulación a paso fijo. `G` cambia la gravedad y `X` la asistencia. |
| 5:30 | `L` y aterrizaje; luego `B` al volver a la nave | La nave puede bajar, posar el tren y despegar con ayuda. Para mostrar el Tour, reinicia y elige `2` en la portada. |
| 6:15 | `F6` y `F3` | La escala interna baja si el tiempo de cuadro se mantiene alto, y el HUD conserva la resolución nativa. Mostrar el benchmark medido en el README. |

Para mostrar el juego pirata, elige Juego en la portada. Cerca de Verdalia, pulsa `B` para bajar, camina hacia el trofeo indicado en el HUD y recógelo con `E`. El contador cambia a 1/5. Cada planeta tiene uno y, al juntar los cinco, aparece la pantalla de victoria. Las patrullas se acercan en tres modelos distintos, incluso en el planeta, y el número máximo aumenta con los trofeos. Enseña la mirilla y el radar; apunta con el botón derecho o las flechas y dispara con `Espacio`. Muestra el tutorial de controles con `F1`. `Shift` acelera en vuelo local y permite correr a pie. Muestra el agujero negro solo si queda tiempo: se activa al alejarse hacia el borde del sistema.

## Cambio sencillo en vivo

En `src/world/mod.rs`, `solar_system`, cambiar el radio de Verdalia y volver a compilar. El planeta, su superficie y su zona de influencia cambian porque la misma definición alimenta la escena y la física. Otra opción es cambiar `MAX_ERROR_PX` en `src/geometry/lod.rs`: al subir el umbral, se eligen esferas menos densas y `F3` muestra menos triángulos.

## Preguntas probables

**¿Por qué el z-buffer guarda `1/w`?** Porque se interpola linealmente en pantalla con las coordenadas baricéntricas; un valor mayor está más cerca y cero representa el fondo (`src/raster/mod.rs`).

**¿Qué evita que la física dependa de los FPS?** `FixedStepper` acumula el tiempo y ejecuta pasos de 1/120 s, con un máximo de ocho por cuadro (`src/ship/stepper.rs`).

**¿Cómo detectas un impacto rápido?** La colisión de `src/ship/physics.rs` barre el movimiento de la nave entre la posición anterior y la nueva contra la superficie. Así no depende solo de que la posición final esté bajo el suelo. La velocidad de contacto determina si aterriza o se destruye.

**¿Por qué el terreno usa chunks?** El quadtree divide solo las zonas que ocupan suficientes píxeles. Cada chunk tiene malla propia y un faldón para ocultar grietas; si una hoja aún no se generó, se muestra un ancestro ya disponible (`src/terrain/`).

**¿Por qué los objetos siguen la rotación del planeta?** Sus posiciones se generan en coordenadas locales del cuerpo. `Scene::render` aplica a los objetos la misma matriz de traslación y rotación que al terreno, y su orientación inicial sale de la normal del relieve (`src/scatter/`, `src/app.rs`).

**¿Qué hacen las teclas F?** `F1` abre ayuda; `F3` muestra contadores; `F4` alterna el frustum culling del rasterizador, mientras el quadtree del terreno sigue seleccionando chunks visibles; `F5` alterna LOD; `F6` alterna resolución automática y 100 %. `--scale` fija la escala para medirla sin que cambie.

**¿Qué hace el culling y qué hace el LOD?** El culling evita procesar llamadas fuera de la cámara. El LOD usa mallas y detalle más baratos cuando el objeto ocupa pocos píxeles. `F3` permite ver el efecto de cada uno.

**¿Qué hace la IA?** El README declara la asistencia usada. Quien presente debe poder explicar el pipeline, la física y un shader con el código abierto, y ajustar cualquier afirmación de contribución a su trabajo real.

El cierre debe mostrar el Tour o una captura del aterrizaje. Antes de entregar, prueba los controles a mano y graba el video final.
