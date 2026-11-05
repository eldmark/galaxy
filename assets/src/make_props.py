"""Genera props de superficie: tree.obj, obsidian.obj, ice_crystal.obj (origen en la base, +Y arriba)."""
import os, sys, math
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from meshlib import *

def apex(x, y, z, n):
    return [(x, y, z)]*n

def tree():
    m = Mesh(); n = 9
    m.add("trunk", loft([circ_y(0.0, 0.55, 6, 0, 0), circ_y(1.2, 0.38, 6, 0.05, 0), circ_y(3.5, 0.26, 6, 0.0, 0.05)]), (0.30, 0.20, 0.12), 0.03)
    greens = [(0.12, 0.34, 0.14), (0.16, 0.42, 0.16), (0.20, 0.48, 0.18), (0.24, 0.54, 0.20), (0.30, 0.60, 0.22), (0.36, 0.66, 0.26)]
    for i in range(6):
        y0 = 2.0 + i*1.15; r = 2.6 - i*0.4; rot = i*0.35
        m.add("canopy", loft([circ_y(y0, r, n, 0, 0, rot), circ_y(y0+0.55, r*0.55, n, 0, 0, rot), apex(0, y0+1.9, 0, n)]), greens[i], 0.05)
    return m

def obsidian():
    m = Mesh(); n = 6
    def spire(bx, bz, h, r, lean, rot, col):
        x, z = bx, bz
        rings = [circ_y(0.0, r, n, x, z, rot), circ_y(h*0.3, r*0.85, n, x+lean*0.3, z, rot+0.2),
                 circ_y(h*0.75, r*0.5, n, x+lean*0.8, z, rot+0.4), apex(x+lean, h, z, n)]
        m.add("spire", loft(rings), col, 0.03)
    spire(0.0, 0.0, 5.0, 0.95, 0.4, 0.1, (0.07, 0.05, 0.11))
    spire(1.1, 0.4, 3.2, 0.6, 0.5, 0.5, (0.09, 0.06, 0.14))
    spire(-0.9, 0.7, 2.6, 0.55, -0.4, 0.9, (0.06, 0.04, 0.10))
    spire(0.3, -1.0, 1.8, 0.45, 0.2, 0.3, (0.12, 0.07, 0.15))
    m.add("shard", loft([circ_y(0.0, 0.5, n, -0.6, -0.9, 0.2), circ_y(0.5, 0.35, n, -0.6, -0.9, 0.2), apex(-0.5, 1.2, -0.9, n)]), (0.35, 0.10, 0.08), 0.03)
    return m

def ice():
    m = Mesh(); n = 6
    def cr(bx, bz, h, r, lean, rot, col):
        rings = [circ_y(0.0, r*0.7, n, bx, bz, rot), circ_y(h*0.15, r, n, bx+lean*0.1, bz, rot),
                 circ_y(h*0.7, r*0.9, n, bx+lean*0.7, bz, rot), apex(bx+lean, h, bz, n)]
        m.add("crystal", loft(rings), col, 0.04)
    cr(0.0, 0.0, 4.0, 0.7, 0.3, 0.0, (0.62, 0.86, 0.95))
    cr(0.9, 0.3, 2.6, 0.5, 0.7, 0.4, (0.70, 0.90, 0.98))
    cr(-0.8, 0.5, 2.0, 0.45, -0.6, 0.8, (0.55, 0.80, 0.92))
    cr(0.2, -0.9, 1.5, 0.35, 0.3, 0.2, (0.80, 0.94, 1.0))
    return m

if __name__ == "__main__":
    out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "models")
    for fn, mk in (("tree.obj", tree), ("obsidian.obj", obsidian), ("ice_crystal.obj", ice)):
        m = mk(); m.write(os.path.join(out, fn), fn[:-4])
        print(fn, len(m.F), "tris")
