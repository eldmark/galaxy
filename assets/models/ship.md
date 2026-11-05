# Nave del jugador "Tordo" (ship.obj / ship_low.obj)

Modelada por nosotros con script paramétrico (`assets/src/make_ship.py` + `meshlib.py`), sin Blender.
Unidades: metros. Avance = -Z (morro), +Y arriba, +X derecha. Origen ~ centro de masa.
Formato: `v x y z r g b`, `vn` por cara, grupos `o`, triángulos antihorarios.
Largo 12.2 m (z -5.9..6.3), envergadura 13.8 m. Con tren desplegado y: -3.3..2.2.

| Archivo | Tris |
|---|---|
| ship.obj | 1308 |
| ship_low.obj (LOD ~34 %) | 440 |

Grupos: `hull` (casco, alas, motores, detalles), `cockpit` (cristal azul oscuro),
`nozzle` (toberas casi negras + disco emisivo claro, color ~(1,0.92,0.65)),
`gear_left`, `gear_right`, `gear_front` (tren, partes separadas, mismas en LOD bajo).

## Tren de aterrizaje (modelado en pose DESPLEGADA, coordenadas de nave)

| Grupo | Pivote (x,y,z) | Eje de plegado | Ángulo |
|---|---|---|---|
| gear_left | (-2.0, -1.0, 2.0) | +X (paralelo al eje lateral) | desplegada 0°, plegada -90° |
| gear_right | (2.0, -1.0, 2.0) | +X | 0° / -90° |
| gear_front | (0.0, -1.0, -3.2) | +X | 0° / -90° |

Rotación alrededor de +X por θ: `y' = y cosθ - z sinθ; z' = y sinθ + z cosθ`. Con θ = -90° la pata
(que cuelga hacia -Y) queda tumbada hacia +Z (hacia la cola), pegada a la panza. Para animar:
trasladar a -pivote, rotar, volver a +pivote (interpolar θ de 0 a -90°). También se puede escalar
la pata en Y respecto al pivote. Punto de contacto con el suelo desplegada: y = -3.3.

## Props de superficie (origen en la base, +Y arriba, colores de vértice)

| Archivo | Tris | Tamaño |
|---|---|---|
| tree.obj | 252 | 9.65 m alto, 5 m ancho (grupos trunk, canopy) |
| obsidian.obj | 168 | 5 m alto (spire, shard) |
| ice_crystal.obj | 144 | 4 m alto |

Regenerar: `python3 assets/src/make_ship.py`, `python3 assets/src/make_props.py`;
validar: `python3 assets/src/validate.py`; vista previa:
`python3 assets/src/preview.py assets/models/ship.obj assets/models/ship_preview.png ship`.
