# 09. Estado del proyecto

Actualizado el 12/11/2026. Entrega: 20/11/2026.

## Lo que ya funciona

Galaxy dibuja el sistema en CPU con matrices propias, recorte, z-buffer 1/w, tiles paralelos y sombreado diferido. Raylib solo aparece en `src/platform.rs` para la ventana, las entradas y la textura final. Hay un sol y cinco planetas con posición orbital y rotación calculadas a partir del tiempo de simulación. Los shaders de los cuerpos están en `src/shading/`.

La nave usa física a paso fijo de 1/120 s, gravedad, colisión contra el relieve y modos de crucero, órbita, descenso y posada. En el menú se elige Juego o Tour de observación y se muestra el tutorial correspondiente. En Juego se puede viajar con las teclas `1`-`6`, aterrizar de forma asistida con `L` y reaparecer con `K` o `Supr`. El sol tiene zona letal y hay muerte por impacto a más de 120 m/s. La mirada libre usa el botón derecho y `M` activa el timoneo con el ratón.

En Juego, `B` inicia un descenso asistido y baja al jugador cuando la nave se posa; junto a ella permite abordar y otra pulsación inicia un despegue asistido. `WASD` mueve al pirata y `E` recoge el trofeo cercano. Los cinco planetas son aterrizables, incluido Siroco sobre una capa de nubes jugable. El HUD muestra el avance de 0/5 a 5/5 y al reunirlos aparece la pantalla de victoria, desde donde se puede reiniciar con `Enter`. Hay tres modelos propios de patrulla: exploradora, interceptora y pesada, con distinta vida, velocidad y cadencia. El límite simultáneo sube de dos a cinco conforme avanza la partida. Las patrullas persiguen y disparan incluso cerca de la superficie. El radar indica su posición relativa y una mirilla central marca la dirección del disparo. `Espacio` dispara desde la nave o a pie. `Shift` da más velocidad en órbita y descenso, y un agujero negro atrae y destruye la nave en el borde del sistema. El tutorial se puede consultar y el juego se pausa con `F1`. El Tour omite combate y trofeos. Estas mecánicas están en `src/pirates.rs` y `src/app.rs`.

Cerca del suelo se dibuja terreno cube-sphere por chunks y quadtree (`src/terrain/`). Los objetos de `src/scatter/` se reparten con una semilla reproducible, se orientan según la normal del relieve y comparten la transformación del planeta. El mapa de normales se alterna con `N`. Hay atmósfera, líneas de órbita con `O` y tren de aterrizaje animado.

Esta continuación añadió escala interna automática en cuatro pasos, `F6` para alternar auto/100 % y `--scale` para fijarla. El HUD sigue a 960x540, muestra la escala y amplía `F3` con terreno y objetos. `F5` afecta esferas, terreno, objetos y detalle de shader; `F4` afecta el culling por frustum del rasterizador.

Para repartir el costo de la entrada al terreno se preparan hasta dos chunks por cuadro desde una cámara predicha 0.7 s hacia adelante, y la esfera se mantiene hasta que los ancestros visibles están listos. El quadtree guarda la geometría fija de sus nodos para no volver a muestrear el relieve en cada cuadro. El rasterizador procesa las mallas pequeñas sin crear tareas paralelas por cada llamada.

## Verificación

- `cargo test`: 194 pruebas pasan y 4 se ignoran en el proyecto (192 del juego y 2 del generador de modelos).
- El benchmark completo de 6300 cuadros de `main` alcanzó 13.88 ms medios y 53.15 ms de pico con culling, LOD y 16 hilos; la ejecución se interrumpió antes de imprimir la segunda configuración. En esta rama, con escala 1.0, fueron 12.47 ms medios y 45.94 ms de pico. Sin culling ni LOD, esta rama dio 14.15 ms medios y 57.67 ms de pico. La etapa de selección del quadtree estuvo en 0.19 ms durante el pico de esa última configuración gracias a la caché geométrica.
- En 600 cuadros, con culling y LOD, la escala 0.7 bajó la media de 15.25 a 6.33 ms con 16 hilos, de 23.45 a 12.28 ms con cuatro y de 42.80 a 23.13 ms con dos. La tabla completa está en el README. Ese tramo no recorre todo el descenso y no sirve para afirmar un pico del Tour completo.
- En dos hilos y escala 0.55 la media de esos 600 cuadros fue 19.38 ms. Con cuatro hilos y escala 0.85 fue 22.76 ms. El controlador evita volver a una escala que acaba de superar el umbral alto hasta que haya margen suficiente.
- Se revisaron cuadros PNG del Tour con `F3` abierto, tanto en espacio como en descenso. El panel cabe en 960x540 y deja libres los botones. `scripts/video.sh` convirtió una secuencia corta a MP4 con `ffmpeg`.
- Tras corregir `--record` para renderizar todos los cuadros aunque solo guarde uno cada K, un cuadro espaciado del descenso mostró 260 hojas de terreno y 87 instancias de objetos; antes ese modo dejaba las cachés incompletas en capturas muy separadas.

La meta de pico menor que 25 ms a escala 1.0 sigue pendiente: el mayor cuadro medido fue 45.94 ms y no generó terreno ni objetos; el sombreado ocupó 36.49 ms. La atmósfera promedió 2.33 ms por cuadro en el Tour completo a 16 hilos. Conviene revisar esos picos en la máquina de presentación y usar la escala automática si hacen que el vuelo se sienta entrecortado.

En el mismo Tour de 6300 cuadros a 16 hilos y escala 0.7 se midieron 8.01 ms medios y 44.21 ms de pico con culling y LOD; el pico fue el primer cuadro. Con ambos apagados se midieron 9.25 ms medios y 34.09 ms de pico. El benchmark no incluye el costo de reescalar ni de dibujar el HUD.

## Pendiente antes de entregar

Hay que probar manualmente los controles en la ventana real y grabar el video final. El integrante debe completar `[NOMBRE]`, la contribución y el enlace del video en el README. La luna de Verdalia y los planetas Dunara, Nublar y Egrin siguen como ampliaciones posibles. Las capturas del Tour y el ajuste visual del polvo y la explosión del sol necesitan una última revisión humana.

## Actualización 18/11: pausa y rendimiento en suelo

- **Pausa (`Esc`)**: congela la simulación y el combate, muestra la tabla de controles y los botones `CONTINUAR` y `SALIR DEL JUEGO` (`hud::draw_pause`, bucle en `app::run`). En la portada y en la pantalla de victoria `Esc` sale del programa. `--pause-shot out.png` la captura sin ventana.
- **Etapa de vértices en paralelo por llamada de dibujo** (`raster/pipeline.rs`): el terreno llega como cientos de mallas pequeñas y antes todo el trabajo caía en un hilo. Tour completo de 13.2 a ~10.8 ms por cuadro; el resultado sigue siendo idéntico con 1 y con 16 hilos.
- **Shaders más baratos sin cambiar la imagen** (`src/shading/`: ruido, cachés de esquinas, poda exacta de `cell_surface`, mapa de normales): sombreado a 40 m -40 % en Fornax, -30 % en Verdalia, -25 % en Nix, -20 % en Mutante. Diferencia de imagen de 0/255 de media.
- **Escala dinámica apunta a 60 FPS** (`resolution.rs`): baja con más de 16 ms sostenidos (de dos en dos niveles si pasa de 28 ms) y sube con holgura.
- **Bancos nuevos**: `--bench` imprime una línea solo con los cuadros de suelo; `--surface-shot ... --bench-ground [--scale S]` mide el cuadro a ras de suelo por planeta.

Tiempo por cuadro a 40 m de altitud, 16 hilos, 960x540 (la máquina estaba compartida, hay ruido de unos ±10 %):

| Planeta | Escala 100 % | Escala 70 % | Escala 55 % |
|---|---|---|---|
| Verdalia | 25.4 ms | 14.9 ms | 12.6 ms |
| Fornax | 43.6 ms | 24.0 ms | 15.2 ms |
| Nix | 18.8 ms | 10.0 ms | 7.1 ms |
| Mutante | 17.6 ms | 8.4 ms | 5.7 ms |
| Siroco | 12.1 ms | 7.2 ms | 4.5 ms |

Con la escala automática todos entran en 16.7 ms; Fornax es el más pesado y necesita llegar al 55 %. A 100 % solo Siroco cumple.
