# Galaxy: plan del proyecto final

Sistema solar explorable en Rust con **renderizador por software**. Se vuela desde el espacio, se orbita un planeta, se desciende y se **aterriza** sin pantalla de carga, y existe un modo de recorrido automático (estilo No Man's Sky).

Enunciado original: `PROYECTO_FINAL GRAFICAS 2026.md` (en esta carpeta).

## Documentos

| # | Archivo | Contenido |
|---|---|---|
| 00 | `00-indice.md` | Este índice, visión, decisiones y supuestos |
| 01 | `01-requisitos-y-puntos.md` | Requisitos eliminatorios y mapa de puntos objetivo |
| 02 | `02-arquitectura.md` | Módulos, pipeline, escala del mundo, multihilo |
| 03 | `03-planetas-y-superficie.md` | Cube-sphere con LOD, ruido, shaders, objetos en superficie |
| 04 | `04-navegacion-aterrizaje-autopiloto.md` | Modos de vuelo, gravedad, aterrizaje, tour automático |
| 05 | `05-hitos-y-pruebas.md` | Hitos con criterios de aceptación, pruebas, video, entrega |
| 06 | `06-reutilizacion-y-riesgos.md` | Qué se copia de `cargar-modelo` y `lab6`, riesgos |
| 07 | `07-assets-y-temas.md` | Planetas extra, assets inspirados en Star Wars/Marvel/Jurassic Park/Dune, vegetación por planeta |
| 10 | `10-guion-demo.md` | Guion de la demostración en vivo y preguntas probables del profesor |

## Visión en una frase

Un solo mundo continuo (sin escenas): el mismo bucle dibuja el sol, planetas pequeños con órbita, una nave y, al bajar, la superficie de un planeta con relieve, objetos y atmósfera. La cámara pasa de "escala sistema solar" a "escala suelo" cambiando solo de **marco de referencia y nivel de detalle**.

## Decisiones y supuestos (confirmar con el equipo)

1. **Todo en CPU.** El enunciado prohíbe shaders de GPU, cámara 3D, modelos, materiales y física de raylib; incumplirlo da 0. Raylib solo abre la ventana, lee entradas y sube el framebuffer a una textura 2D para mostrarlo (igual que en `lab6`). "Usar más poder" significa **hilos de CPU** (rayon, por tiles), no GPU.
2. **Aterrizar es un extra.** El enunciado dice que no se requiere. Se construye sobre la base obligatoria (hito M4 ya cumple todo lo eliminatorio) y no la pone en riesgo.
3. **"Planetas que tengan órbita"**: todos los planetas orbitan el sol. Son aterrizables todos menos Siroco (gigante gaseoso, sin suelo: se puede rozar la capa de nubes y la presión empuja hacia arriba) y el sol.
4. **Piloto automático (decidido)**: un **botón** (HUD clicable y tecla `T`) inicia un **tour automático** que viaja de planeta en planeta, orbita, desciende con **animación de aterrizaje**, espera y despega. En el suelo **no hay recorrido automático**: la nave posada solo se mueve por control del jugador.
5. **Planetas pequeños** (estilo Mario Galaxy / Outer Wilds): radios de 2 a 6 km, órbitas cercanas, viajes de 30 a 60 s entre planetas.
6. **Trabajo individual (decidido).** Una sola persona: el alcance se prioriza con P0/P1/P2 (ver `05` y `07`). Los puntos de la lista se sostienen con P0; lo demás es ampliación.
7. **Resolución 960x540 (decidido).**
8. **Ocho planetas** (5 de `lab6` + Dunara, Nublar y Egrin) con **vegetación propia por planeta**. Ver `07-assets-y-temas.md`. Solo 5 planetas puntúan; los 3 nuevos son de segunda prioridad.

## Resumen del alcance

| Capa | Qué incluye |
|---|---|
| Obligatorio | Sol, nave propia, ≥1 planeta con órbita y rotación por tiempo real, acercamiento sin cortes, modelos con M/V/P y z-buffer, shader propio, video |
| Núcleo de puntos | 5 planetas, 4 shaders + animado, deformación, subdivisión, LOD, culling, gravedad, colisión, control local, aceleración progresiva |
| Superficie | Terreno cube-sphere con LOD, árboles/rocas/cristales/edificios procedurales, mapa de normales |
| Extra "No Man's Sky" | Descenso atmosférico, aterrizaje, despegue, tour automático con botón y animación de aterrizaje, vegetación independiente por planeta |
| Ambientación | Planetas y assets inspirados en Star Wars, Marvel, Jurassic Park y Dune (originales, no copias) |
