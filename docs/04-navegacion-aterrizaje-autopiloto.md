# 04. Navegación, aterrizaje y autopiloto

El enunciado señala que la nave debe viajar por el sistema y también cerca de la superficie, y que eso **cambia cómo se controla**. Se resuelve con una máquina de estados de navegación; cada modo cambia el marco de referencia, las limitaciones de velocidad y el mapeo de controles. Todas las transiciones son mezclas de ~1.5 s dentro del mismo mundo.

## Modos

```
        altitud < 3R                altitud < 0.5R              contacto suave
CRUCERO ───────────────▶ ORBITA ───────────────▶ DESCENSO ───────────────▶ POSADA
   ▲  ◀───────────────── (LOCAL)  ◀──────────────  (ATMOSFERA) ◀─────────────  │
   │      altitud > 3.5R           altitud > 0.7R       despegue (Espacio)      │
   └──────────────── AUTOPILOTO (TOUR / ATERRIZAR AUTO) se activa con el botón desde CRUCERO, ORBITA o DESCENSO
```

Los umbrales usan histéresis (entrada < salida) para que no oscile entre modos.

| Modo | Marco | Velocidad máx. | Controles |
|---|---|---|---|
| CRUCERO | Sistema | hasta ~2 km/s con boost | Empuje adelante/atrás, strafe, cabeceo/guiñada/alabeo con inercia, boost |
| ORBITA (local) | Planeta orbital | proporcional a la altitud | Mantener altitud, orbitar izquierda/derecha, subir/bajar radial, mirar al planeta |
| DESCENSO | Planeta superficial | `v_max = k · altitud` | "Arriba" = radial; horizonte estabilizado; asistencia de frenado |
| POSADA | Planeta superficial | 0 | Cámara libre, despegue |
| AUTOPILOTO | cualquiera | según fase | Cualquier entrada de mando lo interrumpe (tecla `T` lo reanuda) |

### CRUCERO (8 pts de aceleración progresiva)

- Estado: posición/velocidad lineal (`f64`), orientación (cuaternión) y velocidad angular.
- Control: la entrada fija un **objetivo** de aceleración; la nave lo alcanza con una constante de tiempo (`a += (a_obj - a) * (1 - e^(-dt/τ))`). Igual para el giro.
- Asistencia de vuelo: sin entrada, un amortiguamiento suave reduce la velocidad respecto del marco de referencia más cercano; `X` la desactiva (vuelo newtoniano puro).
- Paso fijo de 1/120 s con acumulador (`02-arquitectura.md`): mismo comportamiento a 20 o 144 FPS. Test: simular 5 s con distintos `dt` de render y comparar posición final.

### ORBITA / control local (8 pts)

Al entrar en la esfera de influencia el marco pasa a ser el del planeta. Con ello:

- La nave **ya se mueve con el planeta**: no hay que perseguirlo a su velocidad orbital.
- La entrada se reinterpreta en coordenadas del planeta: A/D recorren un círculo a altitud constante, W/S sube/baja radialmente, la cámara puede fijarse al horizonte o al centro del planeta.
- Un anillo de altitud (HUD) sugiere la distancia al suelo, que permite "dar la vuelta" fácilmente, como pide el video.

### Gravedad (10 pts)

```
a_i = G·M_i / (r² + ε²)   dirigida al centro del cuerpo i, solo si r < r_influencia_i
```

- Valores elegidos para jugabilidad (no físicos) y `ε` evita singularidad. El sol no atrae a la nave en cuanto a gameplay (o con valor muy bajo) para que no se pierda caída libre hacia él.
- En ORBITA y DESCENSO hay una **asistencia**: se cancela parcialmente para que no obligue a empujar siempre contra el planeta. Se puede apagar con una tecla para demostrar el efecto de la gravedad sobre la trayectoria (HUD muestra el vector).
- Demostración: apagar motores a 2R y observar la nave curvar su trayectoria.

### Colisión básica (4 pts)

- Nave como esfera de radio ~6 m. Chequeo: `|p| - radio_nave < height(dir(p))`.
- Respuesta: se empuja a la altura de contacto y se elimina la componente normal de la velocidad (rebote amortiguado). Mismo `height()` que el terreno visible, así no hay discrepancias.
- El sol no tiene suelo: su fotosfera es letal. Siroco usa la capa superior de nubes como superficie jugable para aterrizar y explorar.

## DESCENSO y aterrizaje

1. **Entrada** a ~0.5R: el marco pasa a superficie (la nave hereda la velocidad de rotación del suelo).
2. **Horizonte estabilizado**: la orientación se mezcla con la base local (arriba = normal esférica) para que la nave no ruede al descender. Giro libre sigue funcionando para mirar alrededor.
3. **Límite de velocidad dependiente de altitud** y *flare* automático al acercarse al suelo.
4. **Contacto suave** cuando `altitud < h_tren` y `v_vertical < 6 m/s` y inclinación < 25°: el estado pasa a POSADA. Si no, es rebote por colisión.
5. **POSADA**: la nave se fija al marco superficial (queda estacionaria sobre el suelo y gira con el planeta), se alinea a la normal con suavizado y se anima el tren. Despegue con `Espacio`: empuje vertical hasta salir de la atmósfera.
6. Sin carga ni escena: el LOD del terreno ya generó los chunks cercanos durante el descenso (la caché los precalienta mirando en la dirección de la nave).

### Detalles

- **Seguimiento del terreno**: durante el descenso se usa `height()` directo para el aviso de proximidad (HUD: altitud real al suelo, no al centro).
- **Sin vibración**: origen flotante y posiciones locales (ver `02-arquitectura.md`).
- **Efectos de entrada** (opcional): tinte rojizo del cielo con la velocidad, partículas de polvo cerca del suelo (puntos en el framebuffer).

## AUTOPILOTO

El **Tour de observación** se elige en la portada, después de ver su tutorial. La nave viaja, orbita y aterriza sola. Es una máquina de estados que alimenta los mismos controles de vuelo y comparte física, colisión y límites. No hay combate ni trofeos en este modo. El Juego es la otra opción de la portada y permite aterrizar automáticamente con `L`.

### Portada

- Los botones de Juego y Tour los dibuja `hud.rs` en el framebuffer; el ratón solo entrega la posición y el clic.
- Al elegir Tour se muestra su tutorial y luego el HUD indica destino, fase y ETA.
- Botón secundario `ATERRIZAR AUTO`: si la nave está en ORBITA o DESCENSO sobre un planeta aterrizable, ejecuta solo la secuencia de descenso y posada del planeta actual.

### Animación de aterrizaje

Secuencia determinista, controlada por una curva de tiempo `t ∈ [0,1]` en la fase DESCENDER/POSAR:

1. **Alineación** (1.5 s): la nave gira hasta tener el morro hacia el horizonte y "arriba" = normal.
2. **Descenso en espiral**: altitud sigue `alt(t) = alt0 · (1 - ease_in_out(t))^1.5` con un giro suave alrededor del punto de aterrizaje.
3. **Frenado** (últimos 60 m): velocidad vertical decae a ~2 m/s; el morro se levanta unos grados (flare).
4. **Tren de aterrizaje**: animación de despliegue (escala/rotación de partes de la malla de la nave, sin huesos).
5. **Polvo/chispas**: puntos o quads emisivos en el contacto, con color por planeta (arena, nieve, ceniza).
6. **Toque**: ligera compresión de la altura de la cámara y un temblor corto; la nave pasa a POSADA.
7. **Despegue** (Tour y `Espacio`): tren se repliega y empuje vertical con la misma curva invertida.

El destino de aterrizaje se elige con `scatter` (claro de terreno plano cerca de un punto de interés) o la plataforma de Verdalia.

### Tour (viaje automático entre planetas)

```
Elegir siguiente destino (ruta fija o aleatoria con semilla, evitando repetir)
 ├─ ALINEAR   gira hacia el punto de intercepción (planeta predicho a t + ETA)
 ├─ TRANSITO  empuje con perfil trapezoidal (acelera / crucero / frena); corrección continua
 ├─ APROXIMAR entra a la esfera de influencia, iguala velocidad con el marco del planeta
 ├─ ORBITAR   1 vuelta a ~1.0R de altitud, cámara cinematográfica
 ├─ DESCENDER espiral de descenso hacia un punto de interés (plataforma, bosque, lava)
 ├─ POSAR     toque suave, espera 5-8 s (cámara orbitando la nave)
 ├─ DESPEGAR  ascenso vertical hasta ~1R
 └─ siguiente destino
```

- **Intercepción**: la órbita es analítica, así que se predice `pos_planeta(t + ETA)` iterando 3-4 veces: cálculo barato y estable.
- Control tipo PD: el autopiloto genera `entrada ∈ [-1,1]` que pasa por el mismo suavizado que las teclas.
- Cualquier tecla de mando lo interrumpe y devuelve el control sin saltos (la nave conserva su estado).
- HUD: destino, ETA, fase actual.
- El **video de entrega** puede producirse con el Tour, y con él se garantiza el recorrido exigido (vuelo, acercamiento, vuelta, alejamiento).

### Sin recorrido automático en el suelo

Decisión: el Tour termina cada visita con el despegue. Mientras la nave está posada o volando bajo, el movimiento es 100 % manual; el relleno visual de esa parte lo da la vegetación independiente por planeta (`07-assets-y-temas.md`).

## Cámaras

| Cámara | Uso |
|---|---|
| Cabina / seguimiento | Predeterminada; seguimiento con retardo suave |
| Libre alrededor de la nave | Mirar el entorno sin cambiar el rumbo (clic derecho + arrastrar) |
| Cinemática | Autopiloto: órbita lenta alrededor de la nave o del planeta |

El enunciado exige poder orientar la nave y ver el entorno durante el acercamiento: la cámara libre cubre ese punto en todos los modos.

## Controles propuestos

| Tecla | Acción |
|---|---|
| W/S, A/D, Q/E | Adelante/atrás, lateral, abajo/arriba |
| Flechas | Cabeceo y guiñada; `Z/C` alabeo |
| Botón derecho + arrastrar | Mirada libre de la cámara (no cambia el rumbo; vuelve al soltar) |
| `M` o botón central | El ratón timonea la nave (alterna) |
| Shift / Ctrl | Boost / freno |
| `G` | Gravedad asistida sí/no |
| `X` | Asistencia de vuelo sí/no |
| `1`/`2` en la portada | Elegir Juego o Tour de observación |
| `L` o botón `ATERRIZAR AUTO` | Aterrizaje automático en el planeta actual |
| `B` | Bajar tras aterrizar, abordar cerca de la nave y despegar con ayuda |
| `1`-`6` | Viaje automático al cuerpo (1 = sol): alinear, tránsito, aproximar y una vuelta de órbita |
| `F1` ayuda, `F3` estadísticas, `F4` culling, `F5` LOD, `N` normal map |
| `P` | Captura PNG |

## Actualización (flight): sin tope en crucero, sol, órbitas y aterrizaje asistido

- **Crucero sin tope**: mantener el empuje sigue subiendo la velocidad (solo hay un límite numérico de 200 km/s). El boost sostenido sube el multiplicador de aceleración de 1.5 a 8 con constante de 6 s (`Ship::boost_charge`); el Tour usa 1.5 fijo. En ORBITA y DESCENSO se conserva `v_max = k·altitud`; el exceso al entrar se recorta con exceso/1.5 s y tope de 4000 m/s² (`LOCAL_DECEL_MAX`).
- **Colisión continua**: si un paso mueve más de 30 m se recorre el segmento con muestras de ~6 m (`physics::sweep_collisions`); la zona letal del sol usa intersección segmento-esfera.
- **Gravedad**: el sol atrae (g superficie 30 m/s², influencia 3R, atenuación desde 0.9). Dentro de un marco local solo cuenta el planeta del marco. Sin asistencia no hay co-rotación fuerte, así que una órbita libre a 1.5R conserva su radio (±0.1 % en 5 min).
- **El sol mata**: < 1.5R destruye; 1.5R-2.3R sube la temperatura (HUD, tinte), con daño de casco solo por encima de 55 % de calor. Impacto normal > 120 m/s destruye; 12-120 m/s rebota con daño. Reaparición a los 4 s (K o Supr manual).
- **Aterrizaje asistido**: en DESCENSO, bajo 250 m, bajando a < 60 m/s y < 80 m/s horizontal, sin mandos, arranca solo; cualquier mando lo cancela y lo bloquea 3 s.
- Tecla `O`: líneas de órbita (`src/orbits.rs`). Efectos en `src/particles.rs` y `src/fx.rs`. Modos `--orbits-shot`, `--death-shot`, `--landing-shot` (`src/demos.rs`).
