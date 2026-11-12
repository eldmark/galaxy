# 07. Plan de creación de assets y temas

Inspiración: **Star Wars, Marvel, Jurassic Park y Dune**. Es inspiración de ambientación y siluetas, no copia: el enunciado exige modelos *creados o modificados sustancialmente por nosotros*, y además no conviene usar nombres ni diseños protegidos tal cual.

Reglas:

- Los modelos del juego se generan desde cero con vectores y triángulos en `src/bin/generate_assets.rs`. Nada de descargar modelos de las franquicias.
- Nombres propios dentro del proyecto (Dunara, Nublar, Egrin...), referencias en el README como "inspirado en".
- El código fuente está en `src/bin/generate_assets.rs` y los `.obj` generados en `assets/models/`.
- Si se usa generación asistida o librerías de assets, se declara (lo pide el enunciado) y se modifica de forma sustancial.

## Sistema de 8 planetas

Los 5 de `lab6` ya existen. Se agregan 3 con identidad propia. Por puntos bastan 5 planetas (tope de 50), así que los 3 extra son de **segunda prioridad**: aportan composición visual y variedad de vegetación, no puntos obligatorios.

| Planeta | Inspiración | Identidad visual | Shader (distinto) | Prioridad |
|---|---|---|---|---|
| Verdalia | Endor / Naboo (SW) | Bosque verde, océano, luna | Rocoso con nubes y Conway | P0 |
| Fornax | Mustafar (SW) | Lava y obsidiana | Células + bump + parallax | P0 |
| Nix | Hoth (SW) | Hielo azul, cristales | Vetas cristalinas | P0 |
| Siroco | Gigantes gaseosos | Bandas, tormenta, anillo | Bandas + remolino | P0 |
| Mutante | Estilo propio | Pixel art, vaca/cerdo | HighLife | P1 |
| **Dunara** | Arrakis (Dune) | Desierto de dunas, gusano gigante, cielo ocre | **Dunas**: ondas de viento (senos modulados por ruido), rizado fino con normal map, rocas oscuras | P1 |
| **Nublar** | Isla Nublar (Jurassic Park) | Selva densa, niebla, helechos, dinosaurios | **Selva**: capas de verdes por altura y humedad, ríos por ruido de crestas, niebla por altitud | P1 |
| **Egrin** | Ego, el planeta viviente (Marvel) | Planeta "orgánico" con venas luminosas | **Venas**: Voronoi en cresta + pulso animado cian/dorado, relieve que "respira" | P2 |

Cada uno se coloca en `world/system.rs` con su órbita (Dunara ~45 km, Nublar ~65 km, Egrin ~130 km, ajustables). La luna de Verdalia, ya prevista, se mantiene.

Los tres planetas nuevos requieren: un `height()` propio (parámetros de ruido), un shader nuevo, una paleta de cielo y su juego de vegetación/props.

## Assets por categoría

Presupuestos de triángulos (se ven a pocos metros, así que cada uno tiene versión baja de ~25 %):

| Asset | Tris (alto/bajo) | Planeta | Prioridad |
|---|---|---|---|
| **Nave del jugador** | 1500 / 400 | - | P0 |
| Plataforma/pista de aterrizaje | 600 | Verdalia | P0 |
| Árbol de copa facetada | 500 / 120 | Verdalia | P0 |
| Roca de obsidiana + espira | 300 / 80 | Fornax | P0 |
| Cristal de hielo | 120 / 40 | Nix | P0 |
| Edificio/monumento pixelado | 200 / 60 | Mutante | P1 |
| Duna-roca / afloramiento | 250 / 70 | Dunara | P1 |
| Helecho/cícada gigante | 400 / 100 | Nublar | P1 |
| Dinosaurio estilizado (estático o balanceo) | 1200 / 300 | Nublar | P2 |
| Gusano de arena (cuerpo segmentado animado) | 1500 | Dunara | P2 |
| Estación orbital tipo esfera con trinchera | 2000 | cerca de Siroco | P2 |
| Nave secundaria (carguero) | 800 | tráfico ambiental | P2 |
| Venas/raíces emisivas | 300 | Egrin | P2 |

Con P0 se cumplen los requisitos de superficie (4 tipos propios). P1 y P2 son ampliaciones.

### Estilo de modelado

- Low-poly facetado con colores planos y pocas texturas: encaja con un rasterizador por software y se ve intencional.
- Silueta legible a distancia (la nave, torres, árboles deben leerse como forma sin detalle).
- Atributos mínimos por vértice: posición, normal, color; UV solo si hace falta (cristales, nave).
- Origen en la base del objeto, +Y hacia arriba (para colocar en superficie con la normal del terreno).
- Escala en metros: nave ~12 m, árbol 6 a 10 m, cristal 3 a 8 m, dinosaurio ~8 m.

### Referencias de dirección artística (no copias)

| Franquicia | Se toma | Cómo se interpreta |
|---|---|---|
| Star Wars | Nave utilitaria desgastada, plataformas de aterrizaje, estación esférica | Casco ancho con alas cortas y motor doble; paleta gris/naranja |
| Marvel | Planeta cósmico y de colores saturados, energía luminosa | Egrin con venas emisivas; cielos violeta |
| Jurassic Park | Selva húmeda, helechos, silueta de dinosaurio | Nublar con vegetación densa y niebla |
| Dune | Desierto abierto, sombras largas, gusano colosal | Dunara con dunas, luz ocre, rizado de viento |

## Vegetación independiente por planeta

Cada planeta tiene su **tabla de especies** con reglas de colocación; el algoritmo de `scatter` es el mismo pero los parámetros son propios.

```rust
struct Species { model: ModelId, density: f32, min_alt: f32, max_alt: f32,
                 max_slope: f32, noise_freq: f32, noise_threshold: f32,
                 scale: (f32, f32), lod_distances: [f32; 2], billboard: bool }
```

| Planeta | Especies |
|---|---|
| Verdalia | Árbol facetado (bosques en manchas por ruido), arbusto bajo, hierba en cruces |
| Nublar | Helecho gigante, cícada, árbol alto de tronco liso, enredaderas bajas; niebla densa |
| Dunara | Casi sin vegetación: matas secas dispersas, rocas, cactus esporádicos; sombras largas |
| Nix | Cristales de hielo en grupos, "arbustos" de escarcha |
| Fornax | Espiras de obsidiana, roca; sin vida (contraste) |
| Mutante | Edificios/arbolitos cúbicos en cuadrícula de pixel |
| Egrin | Raíces/venas luminosas y hongos que pulsan con el tiempo |

### Niveles de detalle de vegetación

1. 0 a 40 m: modelo alto.
2. 40 a 150 m: modelo bajo.
3. 150 a 400 m: billboard (dos quads cruzados con color plano) o punto sombreado.
4. Más lejos: no se dibuja; el color del terreno ya sugiere bosque (mancha verde en el shader).

Esto mantiene el coste acotado, es el LOD de instancias (puntúa en rendimiento) y se muestra con contadores.

### Hierba y detalle denso

Para suelo "con vegetación" sin explotar triángulos: **hierba en cruces** de 2 triángulos, solo en un radio de ~60 m alrededor de la nave, generada con el mismo hash determinista por celda. Solo Verdalia y Nublar la usan al inicio.

### Animación barata

- Árboles y helechos: balanceo en la etapa de vértices (`offset = sin(t + fase) * altura²`), sin huesos. Cuenta como vertex shader en el pipeline propio.
- Gusano de arena y dinosaurio: ondulación del eje del cuerpo en la misma etapa. Si falta tiempo, quedan estáticos.

## Flujo de trabajo de assets (en solitario)

Los modelos actuales se generan con `cargo run --bin generate_assets`.

1. Definir anillos, cajas y puntas con coordenadas en metros.
2. Formar los triángulos, orientar sus caras hacia afuera y calcular las normales.
3. Asignar color a cada vértice y escribir los grupos del `.obj`.
4. Generar el LOD de la nave en el mismo programa; los LOD simplificados de objetos se construyen en `src/scatter/models.rs`.
5. Ejecutar `cargo run --bin generate_assets` y revisar la nave con `--shot-ship` y los objetos con `--surface-shot`.

Variación sin modelos extra: escala 0.8 a 1.3, giro aleatorio y pequeñas inclinaciones por instancia; un tipo sigue contando como uno solo.

### Calendario de assets (orden)

| Fase | Assets | Hito asociado |
|---|---|---|
| A1 | Nave, plataforma, árbol, roca de obsidiana, cristal de hielo (P0) | M2 a M7 |
| A2 | Edificio pixelado, afloramiento de Dunara, helecho de Nublar (P1) | M7, M10 |
| A3 | Dinosaurio, gusano, estación, carguero, venas (P2) | Solo si sobra tiempo |

El visor `--viewmodel` se agrega en M1 porque sirve para probar cada asset apenas se exporta.

## Cobertura de puntos de los assets

- 4 tipos de modelo propio (20 pts): árbol, roca de obsidiana, cristal, edificio pixelado.
- Con P1/P2 se agregan tipos extra que **no suman puntos** (tope 20) pero justifican la composición visual y respaldan la presentación (hasta 5 pts).
- Cada asset debe poder explicarse: cómo se modeló, polígonos, cómo se coloca y cómo cambia con el LOD.
