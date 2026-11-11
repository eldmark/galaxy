# 09. Estado del proyecto

Actualizado el 11/11/2026. Entrega: 20/11/2026.

## Lo que ya funciona

Galaxy dibuja el sistema en CPU con matrices propias, recorte, z-buffer 1/w, tiles paralelos y sombreado diferido. Raylib solo aparece en `src/platform.rs` para la ventana, las entradas y la textura final. Hay un sol y cinco planetas con posición orbital y rotación calculadas a partir del tiempo de simulación. Los shaders de los cuerpos están en `src/shading/`.

La nave usa física a paso fijo de 1/120 s, gravedad, colisión contra el relieve y modos de crucero, órbita, descenso y posada. Puede viajar con las teclas `1`-`6`, seguir el Tour con `T`, aterrizar de forma asistida con `L` y reaparecer con `K` o `Supr`. El sol tiene zona letal y hay muerte por impacto a más de 120 m/s. La mirada libre usa el botón derecho y `M` activa el timoneo con el ratón.

Cerca del suelo se dibuja terreno cube-sphere por chunks y quadtree (`src/terrain/`). Los objetos de `src/scatter/` se reparten con una semilla reproducible, se orientan según la normal del relieve y comparten la transformación del planeta. El mapa de normales se alterna con `N`. Hay atmósfera, líneas de órbita con `O` y tren de aterrizaje animado.

Esta continuación añadió escala interna automática en cuatro pasos, `F6` para alternar auto/100 % y `--scale` para fijarla. El HUD sigue a 960x540, muestra la escala y amplía `F3` con terreno y objetos. `F5` afecta esferas, terreno, objetos y detalle de shader; `F4` afecta el culling por frustum del rasterizador.

Para repartir el costo de la entrada al terreno se preparan hasta dos chunks por cuadro desde una cámara predicha 0.7 s hacia adelante, y la esfera se mantiene hasta que los ancestros visibles están listos. El quadtree guarda la geometría fija de sus nodos para no volver a muestrear el relieve en cada cuadro. El rasterizador procesa las mallas pequeñas sin crear tareas paralelas por cada llamada.

## Verificación

- `cargo test`: 182 pruebas pasan y 4 se ignoran en el worktree de continuación.
- El benchmark completo de 6300 cuadros de `main` alcanzó 13.88 ms medios y 53.15 ms de pico con culling, LOD y 16 hilos; la ejecución se interrumpió antes de imprimir la segunda configuración. En esta rama, con escala 1.0, fueron 12.47 ms medios y 45.94 ms de pico. Sin culling ni LOD, esta rama dio 14.15 ms medios y 57.67 ms de pico. La etapa de selección del quadtree estuvo en 0.19 ms durante el pico de esa última configuración gracias a la caché geométrica.
- En 600 cuadros, con culling y LOD, la escala 0.7 bajó la media de 15.25 a 6.33 ms con 16 hilos, de 23.45 a 12.28 ms con cuatro y de 42.80 a 23.13 ms con dos. La tabla completa está en el README. Ese tramo no recorre todo el descenso y no sirve para afirmar un pico del Tour completo.
- En dos hilos y escala 0.55 la media de esos 600 cuadros fue 19.38 ms. Con cuatro hilos y escala 0.85 fue 22.76 ms. El controlador evita volver a una escala que acaba de superar el umbral alto hasta que haya margen suficiente.
- Se revisaron cuadros PNG del Tour con `F3` abierto, tanto en espacio como en descenso. El panel cabe en 960x540 y deja libres los botones. `scripts/video.sh` convirtió una secuencia corta a MP4 con `ffmpeg`.
- Tras corregir `--record` para renderizar todos los cuadros aunque solo guarde uno cada K, un cuadro espaciado del descenso mostró 260 hojas de terreno y 87 instancias de objetos; antes ese modo dejaba las cachés incompletas en capturas muy separadas.

La meta de pico menor que 25 ms a escala 1.0 sigue pendiente: el mayor cuadro medido fue 45.94 ms y no generó terreno ni objetos; el sombreado ocupó 36.49 ms. La atmósfera promedió 2.33 ms por cuadro en el Tour completo a 16 hilos. Conviene revisar esos picos en la máquina de presentación y usar la escala automática si hacen que el vuelo se sienta entrecortado.

En el mismo Tour de 6300 cuadros a 16 hilos y escala 0.7 se midieron 8.01 ms medios y 44.21 ms de pico con culling y LOD; el pico fue el primer cuadro. Con ambos apagados se midieron 9.25 ms medios y 34.09 ms de pico. El benchmark no incluye el costo de reescalar ni de dibujar el HUD.

## Pendiente antes de entregar

Hay que probar manualmente los controles en la ventana real y grabar el video final. El integrante debe completar `[NOMBRE]`, la contribución y el enlace del video en el README. La luna de Verdalia y los planetas Dunara, Nublar y Egrin siguen como ampliaciones posibles. Las capturas del Tour y el ajuste visual del polvo y la explosión del sol necesitan una última revisión humana.
