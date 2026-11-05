# 10. Guion de la demostración en vivo

Duración objetivo: 6 a 7 minutos, en el orden de la rúbrica. Todo lo que se afirma aquí se verificó en el código (`src/`); lo que aún no existe está marcado como en desarrollo y no se debe mostrar como logrado.

Preparación antes de empezar: `cargo run --release` ya compilado, ventana a 960x540 o mayor, `F3` abierto, una terminal con `cargo run --release -- --bench --frames 600` ya ejecutado para tener los números a la mano, y el editor abierto en `src/world/mod.rs` (`solar_system`) para cambiar un parámetro en vivo.

## Orden de la demostración

| Min | Qué mostrar | Qué decir (1-2 frases) |
|---|---|---|
| 0:00 | Arranque, HUD, `F1` con los controles | "Es un renderizador por software: raylib solo abre la ventana y muestra la imagen. Las matrices, el z-buffer, el rasterizado y los shaders están escritos por mí en la CPU." |
| 0:30 | Planetas y shaders: volar hacia Verdalia, luego Fornax y Siroco (o `--shot` del sistema) | "Cada planeta tiene su propio shader de fragmentos (`src/shading/`). Los adapté del laboratorio lab6 y los porté a este pipeline." |
| 1:15 | Shader animado: mirar Siroco o el sol unos segundos | "Los shaders reciben el tiempo de simulación como uniforme, no el número de cuadro; por eso la tormenta, la granulación y la lava se mueven siempre a la misma velocidad." |
| 1:45 | Subdivisión: alejarse y acercarse a un planeta con `F3` abierto, ver `TRI ENTRADA` | "Las esferas son icosferas subdivididas por código (`icosphere(n)`, 20·4ⁿ triángulos). Se generan los niveles 0 a 5 al arrancar." |
| 2:15 | LOD: `F5` apagado y encendido frente a un planeta | "El LOD elige la malla según el error de silueta en píxeles, con histéresis para que no parpadee. Con `F5` apagado todos usan el nivel máximo y los triángulos suben." |
| 3:00 | Culling: mirar a otro lado, ver `CULL` en `DRAW CALLS`; `F4` apagado y encendido | "Antes de procesar vértices pruebo la esfera de cada objeto contra los seis planos del frustum. Lo que está fuera no cuesta nada." |
| 3:45 | Aceleración progresiva: mantener `W`, soltar, usar `Shift` y `Ctrl`, mirando `VEL` | "La entrada fija una aceleración objetivo y la nave la alcanza con una constante de tiempo. La simulación corre a paso fijo de 1/120 s con acumulador, así que responde igual a 20 o a 144 FPS." |
| 4:30 | Gravedad y colisión: acercarse a un planeta, `X` apagada, `G` encendida y apagada, dejarse caer | "La atracción es GM/(r²+ε²) dentro de una esfera de influencia de 8 radios. La colisión trata la nave como una esfera de 6 m contra la superficie del planeta; si llega suave, se posa." |
| 5:15 | Control local: entrar a ORBITA (el `MODO` cambia al bajar de 3 radios) y rodear el planeta; `L` para aterrizar | "Al entrar en la esfera de influencia, la nave hereda el movimiento orbital del planeta, así que no hay que perseguirlo. Eso permite rodearlo con control fino." |
| 5:45 | Tour: `T` o el botón, un par de fases (APROXIMAR, ORBITAR, descenso) | "Es una máquina de estados que genera las mismas entradas que el jugador; cualquier tecla la interrumpe." |
| 6:15 | Presentación y rendimiento: `F3`, tabla de `--bench` | "Con culling y LOD el pipeline procesa 2 000 triángulos en lugar de 124 572. El tiempo total cambia poco porque el cuello de botella es el sombreado por píxel." |
| 6:45 | Cambio de un parámetro en vivo (ver abajo) | Explicar qué se cambió y qué se espera ver antes de recompilar. |

No mostrar como logrado (en desarrollo): deformación por ruido visible en planetas, terreno con relieve, objetos de superficie, mapa de normales, luna de Verdalia, mirada libre con el ratón, selección de destino con `1`-`6`. Si el profesor lo pregunta: la función `displace` existe y está probada, pero hoy se llama con altura cero.

## Parámetros para cambiar en vivo (ensayar antes)

1. **Radio de un planeta.** `src/world/mod.rs`, `solar_system`: cambiar `3500.0` (radio de Verdalia) a `5000.0`. Al recompilar es más grande, tiene más gravedad superficial (`gm = 30·r²`) y su esfera de influencia crece (8 radios).
2. **Velocidad orbital.** Misma tabla: `orbit_period` de Verdalia, `800.0` a `200.0`. Orbita cuatro veces más rápido; como la posición es función de `t`, no hay nada más que arreglar.
3. **Umbral de LOD.** `MAX_ERROR_PX` en `src/geometry/lod.rs`, de `2.0` a `10.0`: mallas visiblemente más facetadas a la misma distancia y menos triángulos en `F3`.
4. **Intensidad de un shader.** `src/shading/rocky.rs`, umbral de nubes `smooth(0.54, 0.63, ...)` a `smooth(0.40, 0.50, ...)`: más nubes. O en `shader_for` (`src/shading/mod.rs`) crear `VolcanicShader { bump: false, parallax: false }` y comparar Fornax de cerca.

No hay semilla de distribución todavía (no hay scatter de objetos).

## Preguntas probables del profesor

**¿Por qué guardas 1/w en el z-buffer y no z?**
Después de la proyección, 1/w sí es lineal en el espacio de pantalla, así que se interpola con las mismas baricéntricas de pantalla sin corrección de perspectiva. Además mayor valor significa más cerca y 0 significa infinito, que es el valor con el que se limpia el buffer (`Framebuffer::depth`, `src/raster/mod.rs`). Z/w no es lineal en pantalla.

**¿Por qué paso fijo?**
Para que la simulación dependa del tiempo y no de los cuadros. `FixedStepper` (`src/ship/stepper.rs`) acumula el tiempo real y ejecuta pasos de 1/120 s, con tope de 8 por cuadro para no entrar en una espiral. La prueba `fixed_step_is_frame_rate_independent` simula 5 s con 20, 60 y 144 FPS y compara la posición final.

**¿Qué hace el culling?**
Hay dos descartes. Primero, el frustum culling por llamada de dibujo: se transforma el centro de la esfera envolvente y se compara con los seis planos del frustum (`frustum_planes` y `sphere_visible` en `src/raster/pipeline.rs`); si queda fuera, no se procesa ningún vértice. Segundo, el descarte de caras traseras por triángulo. Los contadores salen en `F3` y `F4` apaga el primero.

**¿Cómo funciona el LOD?**
Cada planeta tiene seis mallas (icosferas con 0 a 5 subdivisiones). Para cada nivel se mide el ángulo máximo de arista y se calcula cuánto se hundiría la silueta (la sagita) en píxeles a la distancia actual: `R·(1−cos(θ/2))·H/(2d·tan(fov/2))`. Se escoge el nivel más barato con error de 2 px o menos (`MAX_ERROR_PX`). La histéresis de 25 % (`LOD_MARGIN` en `src/app.rs`) evita saltos entre niveles. Además el shader recibe `detail`, que reduce octavas de ruido en planetas lejanos.

**¿Cómo se calcula la gravedad?**
`Body::gravity_at` (`src/world/mod.rs`): aceleración `GM/(r²+ε²)` dirigida al centro, con `ε` de 5 % del radio para evitar la singularidad. Solo actúa dentro de una esfera de influencia de 8 radios y cae suavemente a cero en el último 25 % (smoothstep) para que no haya saltos. Los valores de GM son de jugabilidad (gravedad superficial cercana a 30 m/s²), no físicos. En ORBITA y DESCENSO una asistencia cancela parte de la atracción; se apaga con `X`, y `G` apaga la gravedad completa.

**¿Cómo evitas que los planetas se escapen o se muevan raro?**
Los planetas no se integran: su posición es una función analítica del tiempo (`Body::position(t)`, órbita circular inclinada), así que no acumulan error ni se desvían. Lo que sí se "escapa" es la nave respecto del planeta que se mueve, y eso se resuelve con el marco de referencia: en ORBITA la velocidad se mide respecto al planeta, y cerca del suelo se mezcla con la rotación del suelo (`corotation_blend`), y una vez posada la nave queda fija al marco del planeta (`step_landed`).

**¿Por qué origen flotante?**
Con distancias de cientos de miles de metros, `f32` pierde precisión. Se resta la posición de la cámara en `f64` y solo el resultado relativo pasa a `f32` (`body_model_matrix`, `CameraRig::relative`). La vista se multiplica con el modelo antes de proyectar para evitar coordenadas de mundo enormes.

**¿Qué es el sombreado diferido en tu pipeline?**
Primero se rasteriza por tiles guardando, por píxel, qué triángulo gana el z-test y sus baricéntricas; solo después se ejecuta el shader de fragmentos una vez por píxel visible. Los shaders caros (ruido, células de lava) no se calculan en píxeles tapados.

**¿Cómo usas varios hilos?**
Con `rayon`: la pantalla se divide en tiles de 64x64 que se procesan en paralelo (`TILE` en `src/raster/pipeline.rs`), y el cubemap del cielo también se precalcula en paralelo. Cada tile tiene su propio buffer, así que no hay bloqueos.

**¿Por qué no hay huecos entre triángulos vecinos?**
El rasterizador usa punto fijo 28.4 con aritmética entera y la regla top-left, de modo que una arista compartida se asigna a un solo triángulo.

**¿Cómo se hacen los planetas animados con Conway?**
Verdalia y Mutante usan una línea de tiempo precalculada de un autómata (`src/shading/life.rs`): Conway para las costas de Verdalia y HighLife para el cambio de vaca a cerdo de Mutante. Es función pura del tiempo de simulación.

**¿Qué hace la IA y qué hiciste tú?**
Ver la sección de asistencia de IA del README: el código se generó con Claude Code bajo mi dirección. Prepararse para explicar el pipeline (`src/raster/pipeline.rs`), la física (`src/ship/physics.rs`) y al menos un shader con detalle.

## Cierre

Resumir: pipeline propio con culling y LOD medibles, cinco planetas con shaders propios, todos animados, más el sol, nave con física a paso fijo, gravedad, colisión y Tour automático. Mencionar con honestidad lo que sigue en desarrollo (terreno, deformación visible, objetos de superficie).
