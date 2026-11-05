"""Genera assets/models/ship.obj y ship_low.obj (nave utilitaria 'Tordo'), modelado parametrico propio."""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from meshlib import *

HULL = (0.55, 0.52, 0.47); HULL_D = (0.38, 0.36, 0.33)
ORANGE = (0.86, 0.42, 0.10); GLASS = (0.10, 0.17, 0.38)
DARK = (0.06, 0.06, 0.07); EMIT = (1.0, 0.92, 0.65); METAL = (0.30, 0.30, 0.32)

def build(high=True):
    m = Mesh(); n = 8 if high else 4
    cr = 0.45 if high else 0
    ns = 8 if high else 5            # lados de cilindros
    hull = [ring(-5.9, 0.55, 0.40, -0.25, 0.15 if high else 0, n=n), ring(-4.0, 1.8, 0.95, -0.1, cr, n=n),
            ring(-1.2, 2.7, 1.30, 0.0, cr, n=n), ring(3.0, 2.8, 1.30, 0.0, cr, n=n),
            ring(4.9, 2.3, 1.10, 0.0, cr*0.8, n=n)]
    if not high: hull = [hull[0], hull[2], hull[4]]
    m.add("hull", loft(hull), HULL)
    # cabina acristalada al frente
    cab = [ring(-5.1, 0.6, 0.15, 0.55, n=4), ring(-4.2, 1.2, 0.5, 1.15, n=4),
           ring(-2.6, 1.45, 0.65, 1.55, n=4), ring(-1.3, 1.5, 0.4, 1.3, n=4)]
    if not high: cab = [cab[0], cab[2], cab[3]]
    m.add("cockpit", loft(cab), GLASS, 0.02)
    m.add("hull", box(-1.6, 1.6, 1.2, 1.4, -1.5, -0.9), ORANGE)   # marco/banda de cabina
    # alas cortas (cuna)
    for s in (-1, 1):
        r = [(2.4, 0.35, -1.8), (2.4, 0.35, 4.6), (2.4, -0.35, 4.6), (2.4, -0.35, -1.8)]
        t = [(6.3, 0.18, 2.2), (6.3, 0.18, 4.6), (6.3, -0.12, 4.6), (6.3, -0.12, 2.2)]
        m.add("hull", loft([[(s*x, y, z) for x, y, z in r], [(s*x, y, z) for x, y, z in t]]), HULL_D)
        xa, xb = sorted((s*6.3, s*6.9))
        m.add("hull", box(xa, xb, -0.14, 0.2, 2.2, 4.6), ORANGE)        # punta naranja
        if high:
            xa, xb = sorted((s*6.4, s*6.6))
            m.add("hull", box(xa, xb, 0.1, 0.9, 3.4, 4.7), ORANGE)      # aleta vertical
    # motores dobles
    for s in (-1, 1):
        x = s*1.65
        nac = [circ(1.0, 0.85, ns, x, 0.1), circ(3.0, 1.0, ns, x, 0.1), circ(5.4, 0.95, ns, x, 0.1)]
        if not high: nac = [nac[0], nac[2]]
        m.add("hull", loft(nac), HULL_D)
        m.add("hull", loft([circ(5.2, 1.0, ns, x, 0.1), circ(5.5, 1.0, ns, x, 0.1)]), ORANGE)
        m.add("nozzle", loft([circ(5.5, 0.9, ns, x, 0.1), circ(6.3, 0.72, ns, x, 0.1)], caps=False), DARK, 0.01)
        m.add("nozzle", loft([circ(6.15, 0.64, ns, x, 0.1), circ(6.2, 0.64, ns, x, 0.1)]), EMIT, 0.0)  # cara emisiva
    m.add("hull", box(-0.2, 0.2, 1.0, 1.6, 2.6, 4.8), HULL_D)   # aleta dorsal
    if high:
        for s in (-1, 1):
            xa, xb = sorted((s*2.7, s*3.3))
            m.add("hull", box(xa, xb, -0.6, 0.5, -1.0, 2.4), HULL_D)          # vainas de carga
            xa, xb = sorted((s*3.3, s*3.36))
            m.add("hull", box(xa, xb, -0.3, 0.2, -0.6, 1.0), ORANGE)
            m.add("hull", box(s*1.0-0.4, s*1.0+0.4, 1.28, 1.45, 0.3, 2.6), HULL_D)  # panel dorsal
        for s in (-1, 1):
            for z in (1.5, 2.3, 3.1, 3.9):     # aletas de refrigeracion de las naceles
                m.add("hull", loft([circ(z, 1.06, ns, s*1.65, 0.1), circ(z+0.18, 1.06, ns, s*1.65, 0.1)]), METAL)
            m.add("hull", box(s*1.65-0.12, s*1.65+0.12, 1.0, 1.9, 4.0, 5.3), ORANGE)   # estabilizador
            m.add("hull", box(s*0.5-0.3, s*0.5+0.3, -0.9, -0.3, -5.2, -4.4), HULL_D)  # toma frontal
        # domo sensor ventral (esferoide)
        dome = [circ_y(-1.3-0.5*math.sin(a), 0.6*math.cos(a), 8, 0.0, -1.6) for a in (0.0, 0.5, 1.0, 1.3)]
        m.add("hull", loft(dome), METAL)
        # parches de desgaste (placas oscuras finas)
        for i, (x, z, w, d) in enumerate([(-1.0, -3.0, .8, .7), (1.2, -2.5, .6, .9), (-0.6, 0.5, .9, .6), (0.9, 2.0, .7, .8),
                                           (-1.8, 3.0, .6, .5), (1.7, 4.0, .6, .6), (-2.3, -1.0, .5, .9), (2.3, 0.2, .5, .7)]):
            y = 1.2 if abs(x) < 2.0 else 0.9
            m.add("hull", box(x-w/2, x+w/2, y, y+0.07, z-d/2, z+d/2), HULL_D)
        m.add("hull", box(-1.2, 1.2, -1.6, -1.25, -3.0, 2.0), METAL)          # panza
        m.add("hull", box(-0.05, 0.05, 1.3, 2.2, -0.8, -0.7), METAL)          # antena
        m.add("hull", box(-0.9, 0.9, -0.12, 0.05, -5.0, -4.6), ORANGE)        # barra de luz
    # tren de aterrizaje: grupos separados, pose desplegada, pivote en (px,-1.0,pz)
    def gear(name, px, pz, pad_w):
        m.add(name, box(px-0.28, px+0.28, -1.5, -0.8, pz-0.35, pz+0.35), HULL_D)
        k = 5
        m.add(name, loft([circ_y(-1.0, 0.13, k, px, pz), circ_y(-3.0, 0.11, k, px, pz)]), METAL)
        m.add(name, box(px-pad_w/2, px+pad_w/2, -3.3, -3.0, pz-0.55, pz+0.55), ORANGE)
        return (px, -1.0, pz)
    pivots = {"gear_left": gear("gear_left", -2.0, 2.0, 0.7), "gear_right": gear("gear_right", 2.0, 2.0, 0.7),
              "gear_front": gear("gear_front", 0.0, -3.2, 0.55)}
    return m, pivots

if __name__ == "__main__":
    out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "models")
    for high, fn in ((True, "ship.obj"), (False, "ship_low.obj")):
        m, piv = build(high)
        m.write(os.path.join(out, fn), "nave Tordo " + ("alta" if high else "LOD bajo"))
        print(fn, len(m.F), "tris", len(m.V), "verts")
    print(piv)
