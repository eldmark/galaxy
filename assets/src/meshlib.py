"""Mini biblioteca de modelado parametrico (solo stdlib). Genera mallas OBJ con color de vertice."""
import math

def sub(a, b): return (a[0]-b[0], a[1]-b[1], a[2]-b[2])
def cross(a, b): return (a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0])
def dot(a, b): return a[0]*b[0]+a[1]*b[1]+a[2]*b[2]
def norm(a):
    l = math.sqrt(dot(a, a)) or 1.0
    return (a[0]/l, a[1]/l, a[2]/l)

def ring(z, hw, hh, yc=0.0, c=0.0, xc=0.0, n=8):
    """Anillo rectangular (n=4) u octogonal achaflanado (n=8) en el plano z."""
    if n == 4:
        return [(xc+hw, yc+hh, z), (xc-hw, yc+hh, z), (xc-hw, yc-hh, z), (xc+hw, yc-hh, z)]
    return [(xc+hw-c, yc+hh, z), (xc-hw+c, yc+hh, z), (xc-hw, yc+hh-c, z), (xc-hw, yc-hh+c, z),
            (xc-hw+c, yc-hh, z), (xc+hw-c, yc-hh, z), (xc+hw, yc-hh+c, z), (xc+hw, yc+hh-c, z)]

def circ(z, r, n=8, x=0.0, y=0.0, rot=0.0):
    """Anillo circular en el plano z."""
    return [(x+r*math.cos(rot+2*math.pi*i/n), y+r*math.sin(rot+2*math.pi*i/n), z) for i in range(n)]

def circ_y(y, r, n=8, x=0.0, z=0.0, rot=0.0):
    """Anillo circular horizontal (plano y), para cilindros verticales."""
    return [(x+r*math.cos(rot+2*math.pi*i/n), y, z+r*math.sin(rot+2*math.pi*i/n)) for i in range(n)]

def box(x0, x1, y0, y1, z0, z1):
    hw, hh, xc, yc = (x1-x0)/2, (y1-y0)/2, (x0+x1)/2, (y0+y1)/2
    return loft([ring(z0, hw, hh, yc, xc=xc, n=4), ring(z1, hw, hh, yc, xc=xc, n=4)])

def loft(rings, caps=True):
    """Une anillos consecutivos (misma cantidad de puntos). Devuelve (verts, tris) sin orientar."""
    V = []; T = []; idx = {}
    def vid(p):
        k = (round(p[0], 5), round(p[1], 5), round(p[2], 5))
        if k not in idx:
            idx[k] = len(V); V.append(k)
        return idx[k]
    for a, b in zip(rings, rings[1:]):
        n = len(a)
        for i in range(n):
            j = (i+1) % n
            T.append((vid(a[i]), vid(a[j]), vid(b[j])))
            T.append((vid(a[i]), vid(b[j]), vid(b[i])))
    if caps:
        for r in (rings[0], rings[-1]):
            c = tuple(sum(p[k] for p in r)/len(r) for k in range(3))
            if all(abs(p[k]-c[k]) < 1e-6 for p in r for k in range(3)): continue
            for i in range(len(r)):
                T.append((vid(c), vid(r[i]), vid(r[(i+1) % len(r)])))
    return V, T

def transform(part, fn):
    V, T = part
    return [fn(v) for v in V], T

def rot_about(part, pivot, axis, deg):
    c, s = math.cos(math.radians(deg)), math.sin(math.radians(deg))
    def f(v):
        x, y, z = sub(v, pivot)
        if axis == 'x': y, z = y*c - z*s, y*s + z*c
        elif axis == 'y': x, z = x*c + z*s, -x*s + z*c
        else: x, y = x*c - y*s, x*s + y*c
        return (x+pivot[0], y+pivot[1], z+pivot[2])
    return transform(part, f)

class Mesh:
    def __init__(self):
        self.V = []; self.C = []; self.F = []   # F: (a,b,c,grupo)
        self.names = []
    def add(self, name, part, color, jitter=0.04):
        V, T = part
        if name not in self.names: self.names.append(name)
        gi = self.names.index(name)
        cen = tuple(sum(v[k] for v in V)/len(V) for k in range(3))
        loc = {}
        def gid(i):
            if i in loc: return loc[i]
            v = V[i]
            h = math.sin(v[0]*12.9898 + v[1]*78.233 + v[2]*37.719)*43758.5453
            h = (h - math.floor(h)) - 0.5
            col = tuple(max(0.0, min(1.0, ch + h*jitter*2)) for ch in color)
            self.V.append(v); self.C.append(col); loc[i] = len(self.V)-1
            return loc[i]
        for a, b, c in T:
            va, vb, vc = V[a], V[b], V[c]
            n = cross(sub(vb, va), sub(vc, va))
            if dot(n, n) < 1e-12: continue
            tc = tuple((va[k]+vb[k]+vc[k])/3 - cen[k] for k in range(3))
            if dot(n, tc) < 0: b, c = c, b   # antihorario visto desde fuera
            self.F.append((gid(a), gid(b), gid(c), gi))
    def normal(self, f):
        a, b, c = (self.V[i] for i in f[:3])
        return norm(cross(sub(b, a), sub(c, a)))
    def write(self, path, comment=""):
        with open(path, "w") as o:
            o.write("# galaxy - %s\n# unidades: metros; avance -Z, +Y arriba, +X derecha\n" % comment)
            o.write("# v x y z r g b ; triangulos antihorarios; vn por cara\n")
            for v, c in zip(self.V, self.C):
                o.write("v %.4f %.4f %.4f %.3f %.3f %.3f\n" % (*v, *c))
            for f in self.F:
                o.write("vn %.4f %.4f %.4f\n" % self.normal(f))
            for gi, name in enumerate(self.names):
                o.write("o %s\n" % name)
                for k, f in enumerate(self.F):
                    if f[3] == gi:
                        o.write("f %d//%d %d//%d %d//%d\n" % (f[0]+1, k+1, f[1]+1, k+1, f[2]+1, k+1))
