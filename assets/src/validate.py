"""Valida mallas OBJ: indices, NaN, area de triangulos, dimensiones, presupuesto, morro en -Z, orientacion."""
import os, sys, math
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from meshlib import sub, cross, dot

def load(path):
    V = []; N = []; F = []; objs = []
    for ln in open(path):
        p = ln.split()
        if not p: continue
        if p[0] == 'v': V.append(tuple(map(float, p[1:4])))
        elif p[0] == 'vn': N.append(tuple(map(float, p[1:4])))
        elif p[0] == 'o': objs.append(p[1])
        elif p[0] == 'f': F.append([tuple(int(i) for i in t.split('//')) for t in p[1:]])
    return V, N, F, objs

def check(path, tris=None, length=None):
    V, N, F, objs = load(path); errs = []
    if any(math.isnan(c) or math.isinf(c) for v in V for c in v): errs.append("NaN/inf")
    for f in F:
        if len(f) != 3: errs.append("cara no triangular"); continue
        if any(not (1 <= a <= len(V)) or not (1 <= b <= len(N)) for a, b in f): errs.append("indice invalido"); continue
        a, b, c = (V[i-1] for i, _ in f)
        n = cross(sub(b, a), sub(c, a))
        if dot(n, n) < 1e-10: errs.append("area cero")
        vn = N[f[0][1]-1]
        if dot(n, vn) <= 0: errs.append("normal incoherente")
    mn = [min(v[k] for v in V) for k in range(3)]; mx = [max(v[k] for v in V) for k in range(3)]
    dims = [mx[k]-mn[k] for k in range(3)]
    if tris and not (tris[0] <= len(F) <= tris[1]): errs.append("presupuesto %d fuera de %s" % (len(F), tris))
    if length and abs(dims[2]-length) > 1.0: errs.append("largo %.2f != ~%s" % (dims[2], length))
    print("%s: %d tris, %d verts, dims x=%.2f y=%.2f z=%.2f, z[%.2f..%.2f] y[%.2f..%.2f] objs=%s -> %s" % (
        os.path.basename(path), len(F), len(V), *dims, mn[2], mx[2], mn[1], mx[1], objs, "OK" if not errs else sorted(set(errs))))
    return V, errs

if __name__ == "__main__":
    d = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "models")
    V, e1 = check(os.path.join(d, "ship.obj"), (1000, 1800), 12)
    # morro en -Z: la masa de vertices delgados (cabina/nariz) esta en z<0; el motor en z>0
    nose_w = max(abs(v[0]) for v in V if v[2] < -5.0); tail_w = max(abs(v[0]) for v in V if v[2] > 5.0)
    print("ancho en morro (z<-5): %.2f, en cola (z>5): %.2f -> morro en -Z: %s" % (nose_w, tail_w, nose_w < tail_w))
    _, e2 = check(os.path.join(d, "ship_low.obj"), (300, 550), 12)
    _, e3 = check(os.path.join(d, "tree.obj"), (150, 400))
    _, e4 = check(os.path.join(d, "obsidian.obj"), (80, 250))
    _, e5 = check(os.path.join(d, "ice_crystal.obj"), (60, 200))
    sys.exit(1 if (e1 or e2 or e3 or e4 or e5 or nose_w >= tail_w) else 0)
