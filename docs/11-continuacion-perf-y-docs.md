# 11. Instrucciones para continuar el trabajo de los agentes `perf` y `docs`

Documento para un agente nuevo. Dos agentes (`perf` y `docs`) se detuvieron a medias, probablemente por falta de tokens. **No los retomes ni versiones lo que dejaron**: empieza desde `main` y usa lo de abajo. Entrega del proyecto: **20/11/2026** (esta nota se escribió el 11/11, quedan unos 9 días).

## 0. Reglas de trabajo

- Trabaja en un worktree nuevo desde `main` (`git worktree add` o la herramienta de aislamiento). **No uses ni fusiones** las ramas `worktree-agent-ae75a0c0804597e97` (perf) ni `worktree-agent-a0043a057db8f04b6` (docs): quedan intactas como referencia. No las borres hasta que el coordinador lo diga.
- Raylib solo se usa en `src/platform.rs` (ventana, entrada, subir el framebuffer). Nada de dibujo 3D, modelos, cámara ni shaders de GPU de raylib: incumplirlo da 0 en el proyecto.
- Usa un `CARGO_TARGET_DIR` propio (p. ej. `/tmp/galaxy-target-<tu-nombre>`); un directorio de build compartido ya devolvió binarios viejos a otros agentes. No dependas de rutas del scratchpad de sesiones anteriores.
- No leas `docs/PROYECTO_FINAL GRAFICAS 2026.md` completo (1.9 MB por imágenes incrustadas). Si necesitas el enunciado: `awk 'length($0)<2000' "docs/PROYECTO_FINAL GRAFICAS 2026.md"`.
- No cambies firmas públicas sin actualizar todos los sitios que las usan (incluidas las pruebas). Todas las pruebas deben seguir en verde.
- Commits en español, cortos, en tu rama. El coordinador fusiona.
- **Verifica antes de afirmar**: en README y docs solo escribe lo que hayas comprobado leyendo el código o ejecutando el comando.

## 1. Estado de partida

- `main` en `0fd06b3`. En esa versión (medido el 05/11, 16 hilos, 960x540, `--bench --frames 6300`): **176 pruebas pasan, 4 ignoradas**, Tour completo con **10.9 ms medios por cuadro y pico de ~52 ms**; sin culling ni LOD 12.4 ms. Vuelve a medir tú mismo como línea base antes de cambiar nada.
- Integrado: pipeline por software con tiles y sombreado diferido, shaders de 6 cuerpos, relieve por planeta, terreno cube-sphere con LOD (`src/terrain/`), objetos de superficie con semilla (`src/scatter/`, `--seed N`), mapa de normales (tecla `N`), atmósfera (`src/atmosphere.rs`), tren de aterrizaje animado, Tour y aterrizaje automáticos con botones, viaje a destino (`1`-`6`), mirada libre (botón derecho), vuelo sin tope con boost progresivo, órbitas libres y líneas de órbita (`O`), el sol mata (zona letal 1.5R, calor hasta 2.3R), muerte por impacto >120 m/s, reaparición (`K`/`Supr`), aterrizaje asistido con polvo.
- Lee primero: `docs/09-estado.md` (desactualizado: termina en la segunda tanda), `docs/04-navegacion-aterrizaje-autopiloto.md` (sección final de vuelo), `src/app.rs`, `src/hud.rs` (`HELP_ROWS` tiene 21 filas), `README.md`.

## 2. Qué dejó cada agente detenido

### `perf` (worktree `.claude/worktrees/agent-ae75a0c0804597e97`)
**No dejó nada**: sin commits ni cambios sin commitear. Toda la tarea 3.A está por hacer desde cero.

### `docs` (worktree `.claude/worktrees/agent-a0043a057db8f04b6`)
- **Los textos no se actualizaron**: `README.md`, `docs/09-estado.md`, `docs/10-guion-demo.md` y `docs/00-indice.md` siguen como estaban en `main`.
- Solo dejó capturas **sin commitear** en su `screenshots/` (28 MB en total, cada PNG pesa ~2 MB porque `src/png.rs` guarda sin comprimir):
  - Nuevas: `atmosfera-1500m.png`, `atmosfera-espacio.png`, `con-normalmap.png`, `sin-normalmap.png`, `nave-tren-plegado.png`, `orbitas.png`, `superficie-fornax.png`, `superficie-mutante.png`, `superficie-nix.png`, `superficie-semilla7.png`, `superficie-verdalia.png`, `terreno-lod.png`. Modificó `sistema.png`.
  - Borró `tour-aproximacion.png`, `tour-crucero.png` y `tour-orbita.png`, pero el README de `main` **todavía los enlaza** (líneas ~7 y ~170).
- Puedes reutilizar esas capturas **copiándolas** (`cp`) al `screenshots/` de tu worktree después de revisarlas con Read (se generaron con el código de `0fd06b3`; si cambias el aspecto en la tarea A, regenera las afectadas). No fusiones su rama.

## 3. Tareas

Orden sugerido: **A** (código) primero y **B** (documentación) al final, porque B debe describir lo que A deje. Si hay dos agentes, el de B puede adelantar todo lo que no dependa de A y marcar `en desarrollo` lo demás, y hacer una pasada final.

### A. Rendimiento, resolución dinámica y HUD (src/)

Objetivo: que sea fluido en equipos de distinta velocidad (el profesor puede tener menos núcleos). **Mide antes y después**: `cargo run --release -- --bench --frames 6300`; para simular máquinas lentas, `RAYON_NUM_THREADS=4` y `=2`.

1. **Picos de cuadro.** El máximo (~52 ms) ocurre al descender: generación de chunks del terreno y horneado de mallas de objetos. Meta: pico < ~25 ms con 16 hilos sin cambiar la apariencia. Ideas: precalentar chunks con una cámara **predicha** (posición + velocidad × ~0.7 s) usando un presupuesto pequeño por cuadro además del normal; repartir generación (terreno y objetos) en varios cuadros con presupuestos; evitar trabajo duplicado. Puntos de partida: `Terrain::frame` y `budget` en `src/terrain/mod.rs`, `src/scatter/mod.rs` (presupuesto de horneado 4 por cuadro), bloque de chunks en `Scene::render` (`src/app.rs`).
2. **Resolución dinámica.** Renderiza el mundo en un `Framebuffer` interno con escala `s ∈ {1.0, 0.85, 0.7, 0.55}` de 960x540 y reescálalo (bilineal o vecino, propio y paralelo, sin raylib) al framebuffer de 960x540 sobre el que se dibuja el HUD y que se presenta (`Platform::present` ignora un framebuffer de tamaño distinto al de `new()`). Controlador con histéresis sobre el promedio móvil del tiempo de cuadro: baja si > 22 ms sostenido ~0.5 s, sube si < 13 ms sostenido ~2 s. Tecla `F6` alterna automático/100 %, `--scale S` lo fija (también en `--bench`), y el HUD muestra la escala y el modo. `atmosphere::apply`, `SkyBox` y los efectos de `src/fx.rs`/`src/orbits.rs`/`src/particles.rs` deben trabajar con `fb.width/height`, no con constantes. Agrega `F6` a `UiInput` (`src/platform.rs`) y a `HELP_ROWS`. Pruebas: escalado correcto (colores, bordes, 1x1 sin pánico), el controlador sube y baja con histéresis sin oscilar.
3. **Costos.** `atmosphere::apply` mide 2-4 ms (meta ≤ ~2): por ejemplo evalúa el cielo a media resolución en píxeles de fondo e interpola. A ras de suelo el shader de Verdalia/Fornax domina: el uniforme `detail` vale 1.0 en todos los chunks; calcula un `detail` por chunk según distancia y cobertura (los shaders ya reducen octavas con `detail` y con `FragIn.distance`) sin cambios visibles apreciables. Reporta ms por etapa antes y después.
4. **HUD / panel F3.** Añade estadísticas de **terreno** (`Terrain.stats`: chunks dibujados, hojas, nivel mín/máx, triángulos, fallback, generados, `ms_select`, `ms_generate`), de **objetos** (`Scene::scatter_stats`: instancias totales/mostradas/descartadas, triángulos con y sin LOD) y la escala dinámica. Debe caber en 960x540 sin tapar los botones (dos columnas o más compacto). Actualiza todos los constructores de `HudState` y sus pruebas.
5. **F4/F5 para la demostración de la rúbrica.** Con culling (F4) y LOD (F5) apagados los contadores deben cambiar visiblemente. Hoy `F5` afecta el LOD de esferas y el `detail`; comprueba si también debe afectar el terreno (p. ej. `px_threshold` mínimo) y los objetos (modelos completos). Haz que lo haga si es sencillo y documenta con precisión qué afecta cada tecla; agrega una prueba.

Aceptación de A: pico < ~25 ms con 16 hilos; con `RAYON_NUM_THREADS=4` el modo automático mantiene un tiempo de cuadro razonable bajando la escala; `cargo test` verde; capturas `--record` del Tour con F3 abierto revisadas con Read (3-4 cuadros); tabla final de `--bench` con 16, 4 y 2 hilos y con `--scale 1.0` y `0.7` en el reporte.

### B. Documentación y entrega (README, docs/, scripts/, screenshots/)

1. **`README.md`**: actualizar a lo que existe de verdad. Todo lo de la sección 1 (viaje `1`-`6`, mirada libre, `M`, `N`, `O`, `K`, vuelo sin tope, el sol letal, aterrizaje asistido, terreno, objetos y `--seed`, atmósfera, tren animado, `F6`/`--scale` si A lo dejó) y quitar las notas "en desarrollo" que ya no apliquen (hoy dicen, entre otras, que faltan objetos de superficie y mapa de normales). Tabla de controles tomada de `HELP_ROWS` de `src/hud.rs`; opciones CLI de `cargo run --release -- --help` (hay modos `--shot`, `--shot-ship`, `--terrain-shot`, `--surface-shot`, `--atmos-shot`, `--orbits-shot`, `--death-shot`, `--landing-shot`, `--record`, `--bench`, `--seed`, y las que añada A). Corrige los enlaces a las capturas borradas.
2. **Tabla "Características del menú que se solicitan evaluar"**: re-evalúa con estado real y dónde verlo/cómo demostrarlo en vivo (tecla + contador de F3): planetas (5), shaders (4+) y animado, mapeo de normales (`N`), deformación por ruido (`src/relief.rs` + `geometry::displace`), subdivisión procedural (`geometry::icosphere` y chunks del terreno), 4 modelos propios y distribución con semilla, orientación sobre superficie curva mientras el planeta rota, control local, aceleración progresiva con paso fijo, gravedad, colisión, culling, LOD (esferas, terreno, objetos), presentación.
3. **`docs/09-estado.md`**: reescribir al estado actual (tras A y la tanda previa) incluyendo mediciones nuevas. **`docs/10-guion-demo.md`**: actualizar el guion y las preguntas probables con respuesta basada en el código (colisión continua por barrido, por qué el terreno usa chunks, cómo se garantiza que los objetos roten con el planeta, qué hace cada F-tecla). **`docs/00-indice.md`**: añadir las filas de `09`, `10` y este documento si procede.
4. **Capturas** (`screenshots/`): reutiliza las nuevas del worktree de `docs` (copiándolas) tras revisarlas con Read; regenera las que A cambie. Pesan ~2 MB cada una: opcional pero recomendable comprimirlas (`oxipng`/`optipng`/`pngquant` si existen, o añadir compresión a `src/png.rs` con DEFLATE propio) para no inflar el repositorio. Deja solo las que el README enlaza.
5. **Marcadores pendientes**: `[CAPTURA: ...]`, `[VIDEO: ...]`, `[NOMBRE]` y `[PENDIENTE: ...]` en el README. Rellena los de captura; los de `[NOMBRE]`, `[VIDEO]` y contribuciones **son del usuario**: déjalos marcados.
6. `scripts/video.sh` y `scripts/record-demo.sh` ya existen y funcionan (probados con ffmpeg real, h264 960x540 60 fps). Comprueba que sigan siendo válidos con los modos nuevos.

## 4. Lo que solo puede hacer el usuario (no lo marques como hecho)

- **Probar a mano con teclado y ratón** la ventana real. Ningún agente pudo: `1`-`6`, `M` (captura el cursor y bloquea los botones del HUD hasta volver a pulsar `M`), mirada libre con botón derecho, `N`, `O`, `K`, `F1`/`F3`/`F4`/`F5`/`F6`, sensación del boost sin tope, que `P` guarde capturas.
- Rellenar `[NOMBRE]`, contribuciones y el enlace del video; grabar el video final (`scripts/record-demo.sh` genera ~5 GB de PNG en `frames/`, ignorado por git, o usar grabación de pantalla).
- Decidir si le gusta el tuneo visual pendiente (ver 5).

## 5. Notas y riesgos conocidos

- La explosión del sol se ve poco sobre el fondo brillante del sol, y el polvo del aterrizaje se ve como manchas borrosas (`src/fx.rs`, `src/particles.rs`): retoque opcional.
- `atmosphere::apply` y el suelo de noche: el suelo se ve negro del lado sin luz.
- La variable de entorno `GALAXY_NO_SCATTER=1` desactiva los objetos de superficie (sirve para medir con el bench).
- `Stats` se acumula (`+=`): reinícialo con `Stats::default()` al inicio de cada cuadro.
- Hay unos pocos warnings de imports sin usar; no importan.
- `Platform::present` ignora framebuffers con tamaño distinto al del `new()`: respétalo al introducir la escala interna.
- Planes posteriores (no son de esta tarea): Dunara, Nublar y Egrin, y vegetación propia por planeta (`docs/07-assets-y-temas.md`).

## 6. Entrega del agente

Reporta: qué hiciste y qué no, tabla de rendimiento antes/después (16, 4 y 2 hilos; escala 1.0 y 0.7), archivos tocados, cambios de contrato, estado de `cargo test` (número de pruebas) y qué quedó pendiente. Checklist final del coordinador antes del 20/11: `cargo test`, `cargo run --release -- --help`, `--bench`, README sin marcadores del agente, scripts de video, `git status` limpio.
