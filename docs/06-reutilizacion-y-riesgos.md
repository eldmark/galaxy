# 06. Reutilización y riesgos

## Qué se aprovecha de los proyectos anteriores

### `../cargar-modelo` (pipeline de la clase)

| Archivo | Uso en galaxy |
|---|---|
| `obj.rs` | Base del cargador; se amplía con `vt` y `vn` |
| `triangle.rs` (baricéntrico, top-left) | Idea del rasterizado; se reescribe por tiles con `1/z` |
| `vertex.rs`, `fragment.rs`, `primitive.rs`, `rasterizer.rs`, `shaders.rs` | Separación de etapas: vértices → primitivas → fragmentos → framebuffer. Se conserva la estructura conceptual |
| `framebuffer.rs` | Se sustituye por framebuffer propio con profundidad |
| `--benchmark-triangles`, flamegraph | Se reutiliza el método de medición |

### `../lab6` (atlas de mundos)

| Archivo | Uso en galaxy |
|---|---|
| `shaders.rs` (`noise`, `fbm`, `star`, `rocky`, `giant`, `nix`, `fornax`, `pixel`, `bumped_normal`, `parallax_position`) | Núcleo de los shaders; se pasan de `f64` y `usize` por mundo a `f32` y `trait PlanetShader`, y se añade el parámetro `detail` |
| `life.rs` | Conway y HighLife sin cambios |
| `decorations.rs` (luna y anillo por píxel) | **No se reutiliza como técnica**: la luna pasa a ser un cuerpo real que orbita y el anillo una malla. Sí se reutilizan sus colores y funciones de patrón |
| `view_renderer.rs` (`Camera`, `project`, estrellas) | Reemplazado por matrices MVP reales; el campo de estrellas se conserva |
| `renderer.rs` | Referencia del baricéntrico con z-buffer; se sustituye |
| `png.rs` | Capturas y volcado de cuadros sin ventana |
| `viewer.rs` | Patrón de ventana + textura del framebuffer; se limita a `platform.rs` |
| `scripts/videos.sh`, `publish-github.sh` | Sin cambios para el video y la publicación |
| `public/sphere.obj`, `README` | Base de documentación y de la esfera inicial para M1 |

Limitación de `lab6`: los vértices se proyectan con un único `focal = size` y la cámara solo tiene yaw/pitch (sin roll). Para volar hace falta cámara completa con cuaternión, plano cercano y recorte. Eso es trabajo nuevo (M1).

## Riesgos y mitigaciones

| Riesgo | Efecto | Mitigación |
|---|---|---|
| **Uso de GPU o funciones 3D de raylib** | 0 puntos | `platform.rs` es el único módulo que importa raylib; revisión en cada PR |
| Rendimiento: shaders con `noise` por píxel a pantalla completa | FPS bajos al aterrizar | Sombreado diferido, `detail` (octavas), tiles en paralelo, resolución interna menor (`--scale 0.5`) |
| Precisión `f32` lejos del origen | Vibración de la geometría en el suelo | Posiciones `f64`, origen flotante, `1/z` |
| Grietas entre chunks de distinto nivel | Agujeros en el terreno | Faldones; restringir a diferencia máxima de 1 nivel entre vecinos si hace falta |
| Parpadeo (pop-in) al cambiar LOD | Distracción visual | Histéresis en el umbral; mostrar al padre mientras se genera el hijo; mezcla de color opcional |
| Generación de chunks bloquea el cuadro | Tirones al descender | Hilos de fondo con rayon, caché, presupuesto de N chunks por cuadro, precalentado en la dirección de movimiento |
| Aterrizaje frustrante o inestable | Difícil mostrar el logro | Asistencia de frenado, horizonte estabilizado, umbrales generosos, Tour que aterriza solo |
| Desfase de marcos de referencia al cambiar de modo | Saltos de cámara | Mezcla de 1.5 s; las pruebas comprueban que posición y velocidad en mundo son continuas en la transición |
| Múltiples características sin tiempo | Puntos no obtenidos | Recorte por prioridad (abajo) |
| Modelos "no propios" | Puntos de superficie no válidos | Conservar el generador propio en Rust y los `.obj` que produce |
| Autoría: no poder explicar el código | Pérdida de puntos | Cada persona documenta su módulo; ensayo de cambios en vivo |

## Orden de recorte si falta tiempo

De lo primero a lo último que se sacrifica:

1. Planetas y assets P2 (Egrin, dinosaurio, gusano, estación, carguero).
2. Atmósfera con dispersión (se deja cielo plano degradado) y hierba en cruces.
3. Dunara y Nublar (quedan los 5 planetas de `lab6`).
4. Parallax/bump específico de Fornax (se mantiene el normal map general).
5. Mutante y Nix aterrizables (quedan Verdalia y Fornax; siguen contando como planetas).
6. Luna de Verdalia como cuerpo real; tour completo (se conserva solo `ATERRIZAR AUTO` y el aterrizaje manual).
7. **No recortar**: M0-M4 (eliminatorios), M6 (terreno), gravedad y colisión, culling y LOD con contadores.

Con todo hasta el punto 6 recortado quedan ~140 puntos teóricos, aún por encima de 100.

## Preguntas abiertas

Resueltas: trabajo individual, Tour con botón y animación de aterrizaje, sin recorrido automático en el suelo, resolución 960x540, 3 planetas extra.

Pendientes:
- Fecha de entrega (define hasta qué capa P0/P1/P2 se llega).
- Riesgo nuevo por trabajo individual: el modelado compite en tiempo con el código. Mitigación: assets P0 primero, estilo facetado y presupuesto de triángulos fijo (`07`).
- Opcional: bitácora de planetas visitados (estilo No Man's Sky), sin impacto en puntos.
