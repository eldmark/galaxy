# 02. Arquitectura

## Dependencias

```toml
[dependencies]
raylib = "5.5.1"   # ventana, entrada, subir framebuffer a textura 2D, nada más
rayon  = "1"       # paralelismo de CPU por tiles (permitido: no sustituye etapas evaluadas)
```

No se usa ninguna crate de ruido, matemáticas ni física: el ruido viene de `lab6/shaders.rs` y las matemáticas son propias (`Vec3`, `Mat4`, cuaterniones). Los perfiles `release` con `opt-level = 3`, `lto = "thin"`, `codegen-units = 1`.

## Estructura de módulos

```
src/
  main.rs            CLI (--shot, --bench, --seed, --tour), bucle principal
  app.rs             estado global, paso fijo, cambio de modos
  math/              Vec3 (f32 y f64), Mat4, Quat, frustum, hash/ruido
  platform.rs        única capa que toca raylib (ventana, teclas, textura)
  raster/
    framebuffer.rs   color + profundidad (1/z) por tile
    pipeline.rs      vértices -> primitivas -> clipping -> binning por tile
    tile.rs          rasterizado baricéntrico, top-left, interpolación perspectiva
    stats.rs         contadores (triángulos entrada/descartados/dibujados, chunks)
  geometry/
    obj.rs           cargador OBJ (v, vt, vn) copiado de lab6 y ampliado
    icosphere.rs     subdivisión procedural (lejos y para el sol)
    cubesphere.rs    mapeo cara/u/v <-> dirección, vecindad
    ring.rs          anillo de Siroco como malla
  terrain/
    height.rs        altura(dir, planeta) con fbm (deformación por ruido)
    quadtree.rs      selección de chunks por error en pantalla
    chunk.rs         malla 17x17 con faldones y normal map por chunk
    cache.rs         caché de chunks (reutiliza y descarta), generación en hilos
  scatter.rs         instancias procedurales por chunk con semilla
  shading/
    mod.rs           trait PlanetShader { fn shade(&FragIn) -> Rgb }
    sun.rs rocky.rs giant.rs ice.rs volcanic.rs pixel.rs ship.rs
    sky.rs           atmósfera y fondo estelar
    life.rs          Conway/HighLife (de lab6)
  world/
    system.rs        cuerpos, elementos orbitales, rotación, luna
    frames.rs        marcos de referencia, origen flotante
  ship/
    physics.rs       integración, empuje, gravedad, colisión
    controls.rs      mapeo de entradas por modo
    modes.rs         máquina de estados de navegación
    autopilot.rs     tour y crucero de superficie
  hud.rs             texto propio (fuente bitmap) dibujado al framebuffer
assets/models/       nave, árbol, roca, cristal, edificio (.obj)
scripts/             videos.sh, publicar (de lab6)
```

Regla: solo `platform.rs` importa `raylib`. Facilita probar el renderizador sin ventana y demuestra el cumplimiento del requisito.

## Pipeline por cuadro

```
tiempo real -> pasos fijos de simulación (1/120 s, acumulador) -> estado del mundo
1. Actualizar órbitas y rotaciones: posición = f(t) analítica
2. Elegir marco de referencia y calcular matriz de vista (origen flotante)
3. Recolectar dibujables:
     cuerpos lejanos (icosfera), chunks del quadtree, instancias, nave, anillo
   aplicando frustum culling + cull de horizonte + LOD (contando todo en stats)
4. Etapa de vértices (paralela): modelo->vista->proyección, normal, UV
5. Recorte contra el plano cercano (near) y binning de triángulos por tile de 64x64
6. Rasterizado por tile (paralelo, sin candados): baricéntrico + 1/z, guarda
   visibilidad (id de triángulo + baricéntricas) por píxel
7. Sombreado diferido: solo los píxeles visibles llaman a su shader de fragmentos
8. Pasada de cielo/atmósfera y estrellas en píxeles sin geometría
9. HUD en el framebuffer, subir a textura de raylib y presentar
```

El **sombreado diferido** (paso 6-7) es clave: los shaders de `lab6` son caros (varios `noise` por píxel) y aquí el sobredibujo del terreno sería costoso. Guardando visibilidad primero, cada píxel se sombrea una sola vez.

### Resolución y rendimiento

- Framebuffer interno 960x540 (configurable con `--scale`), escalado a la ventana.
- Tiles de 64x64 → ~135 tiles repartidos entre hilos. Con 16 hilos disponibles en la máquina de desarrollo hay margen; el objetivo es **≥30 FPS** con la ventana a 960x540 en un equipo de 4 núcleos.
- Coordenadas en `f32` para el pipeline, `f64` para posiciones de mundo (ver abajo).
- Perfil de referencia: `perf` + `flamegraph`, como en `cargar-modelo`.

## Escala y precisión (el problema central de aterrizar sin cortes)

El mundo usa metros. Valores de referencia:

| Elemento | Tamaño |
|---|---|
| Nave | ~12 m |
| Planeta | radio 2 a 6 km |
| Sol | radio ~8 km |
| Órbitas | 20 a 120 km |
| Plano cercano / lejano | 0.5 m / 500 km |

Un `f32` pierde precisión a 100 km del origen (milímetros a metros), lo que provoca vibración en los vértices a nivel de suelo. Solución:

1. **Posiciones de mundo en `f64`** (centradas en el sol).
2. **Origen flotante**: antes de cada cuadro se resta la posición de la cámara en `f64` y solo entonces se baja a `f32`. Todo el pipeline trabaja en coordenadas relativas a la cámara.
3. La profundidad se guarda como `1/z` (lineal en pantalla, buena precisión cerca) y no `z`.
4. Las instancias y chunks guardan posición local al planeta (`f32` pequeña) y la matriz del planeta se compone en `f64`.

## Marcos de referencia

`frames.rs` expone tres marcos y permite mezclar entre ellos en ~1.5 s para evitar saltos:

- **Sistema**: inercial, centrado en el sol.
- **Planeta (orbital)**: se traslada con el planeta, no rota. Se usa al rodear el planeta.
- **Planeta (superficial)**: se traslada y rota con el planeta. Se usa al descender y al estar posado.

Así la nave no es "dejada atrás" por la órbita ni por la rotación mientras se aproxima.

## Tiempo

- `sim_time` avanza con `Instant` real (con tope por cuadro), nunca con el contador de cuadros.
- La física usa **paso fijo de 1/120 s** con acumulador; el render interpola. Esto da respuesta consistente en equipos rápidos y lentos (criterio de 8 pts).
- `--time-scale` acelera órbitas para probar y para el video.

## Estadísticas obligatorias

`raster/stats.rs` cuenta por cuadro y las muestra en el HUD (tecla `F3`):

```
cuerpos/chunks/instancias: total, descartados por frustum, por horizonte, por LOD
triángulos: entrada, tras culling, recortados, dibujados
píxeles sombreados, tiempo por etapa (ms)
```

Teclas de comparación para la demostración: `F4` apaga culling, `F5` apaga LOD (fuerza máximo detalle). El HUD debe mostrar el cambio de conteo y de ms.
