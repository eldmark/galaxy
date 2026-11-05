# 01. Requisitos y mapa de puntos

## Requisitos eliminatorios (si falta uno, 0 puntos)

| Requisito | Cómo se cumple | Hito |
|---|---|---|
| Pipeline de software propio; raylib solo ventana/entrada/mostrar framebuffer | Módulo `raster` propio; sin `draw_model`, `Camera3D`, shaders GLSL | M1 |
| Sol, nave modelada por nosotros, ≥1 planeta | Nave y objetos modelados en Blender y exportados a OBJ | M2 |
| Nave controlable | `ship` con empuje y giro | M3 |
| Planeta orbita y rota según tiempo transcurrido (no cuadros) | `time_s` acumulado con reloj real; órbita = f(t) analítica | M2 |
| Acercarse y alejarse sin pantalla de carga | Un solo mundo, LOD continuo | M5 |
| Orientar la nave y ver el entorno al acercarse | Cámara de nave con mirada libre | M3 |
| Matrices de modelo/vista/proyección + prueba de profundidad | `math::Mat4`, z-buffer por tile | M1 |
| ≥1 shader de fragmentos propio | Shaders de planeta en `shading` | M2 |
| Entrega: código, recursos, instrucciones, video del recorrido completo | README + `scripts/videos.sh` | M10 |

El video debe mostrar: vuelo espacial, acercamiento, vuelta alrededor de un planeta y alejamiento. El guion está en `05-hitos-y-pruebas.md`.

## Puntos objetivo

Hay 200 disponibles y la nota máxima es 100. Meta: **~150 puntos** para tener margen si algo no se acepta en la demostración. Cada característica debe poder verse y explicarse.

### Planetas (50 de 50)

| Planeta | Identidad | Origen |
|---|---|---|
| Verdalia | Continentes, océano, nubes, costas con Conway, luna real en órbita | `lab6` `rocky` |
| Siroco | Gigante gaseoso con bandas, tormenta y anillo como geometría | `lab6` `giant` |
| Nix | Planeta glacial con vetas cristalinas y cristales de hielo | `lab6` `nix` |
| Fornax | Volcánico con grietas de lava, bump y parallax | `lab6` `fornax` |
| Mutante | Mundo pixelado que cambia de vaca a cerdo (HighLife) | `lab6` `pixel` |

Sol: `lab6` `star` (Ígnea). No cuenta como planeta pero sí como shader.

### Superficie (35 de 35)

| Característica | Pts | Plan |
|---|---|---|
| Tipos de modelo propio | 20 | Árbol (Verdalia), roca/espira de obsidiana (Fornax), cristal de hielo (Nix), edificio/monumento pixel (Mutante). 4 tipos |
| Distribución procedural con semilla | 10 | `scatter` con hash determinista por chunk; semilla en configuración y `--seed` |
| Orientación sobre superficie curva mientras rota | 5 | Las instancias viven en el espacio local del planeta; la normal sale del gradiente del terreno |

### Renderizado (52 de 60)

| Característica | Pts | Plan |
|---|---|---|
| 4 shaders distintos | 24 | Rocoso, gaseoso, glacial, volcánico (más sol y pixel como respaldo) |
| Shader animado | 8 | Lava de Fornax, tormenta de Siroco, granulación del sol |
| Mapeo de normales | 8 | Normal map tangente generado por ruido, muestreado con TBN en el cube-sphere |
| Deformación por ruido | 10 | Desplazamiento radial del terreno |
| Generación/subdivisión procedural de geometría | 10 | Quadtree que genera mallas de chunk al vuelo e icosfera subdividida para lejos |

Nota: deformación y subdivisión son logros distintos. Se documenta cada uno por separado (`terrain::displace` y `terrain::build_chunk`/`icosphere::subdivide`).

### Navegación (30 de 30)

| Característica | Pts | Plan |
|---|---|---|
| Control local para rodear un planeta | 8 | Modo ORBITA/LOCAL en el marco del planeta |
| Aceleración, frenado y giro progresivos con paso fijo | 8 | Integración a paso fijo de 1/120 s con acumulador |
| Gravedad de los planetas | 10 | Aceleración GM/r² dentro de la esfera de influencia |
| Colisión básica | 4 | Esfera de la nave contra altura del terreno |

### Rendimiento (15 de 15)

| Característica | Pts | Plan |
|---|---|---|
| Descarte fuera de vista | 7 | Frustum culling por objeto/chunk + horizonte del planeta |
| Niveles de detalle | 8 | LOD de chunks, de instancias y de octavas del shader |

Ambos exigen **contadores visibles** (triángulos procesados/descartados) y un modo de comparación (tecla para apagar culling/LOD).

### Presentación (10 de 10)

Dirección artística coherente (paleta y siluetas) y controles claros (pantalla de ayuda `F1`, HUD con modo, altitud y velocidad).

### Total estimado

50 + 35 + 52 + 30 + 15 + 10 = **192**. Con un recorte táctico (p. ej. dejar mapeo de normales o un shader) siguen quedando >150. El orden de recorte está en `06-reutilizacion-y-riesgos.md`.

## Qué decir en la demostración

Cada integrante debe explicar una contribución concreta y poder cambiar un parámetro en vivo. Dejar estos parámetros accesibles por consola/archivo `config.toml` o teclas: radio del planeta, velocidad orbital, semilla de distribución, intensidad del bump, umbral de LOD.
