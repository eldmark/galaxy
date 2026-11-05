"""Vista previa PNG (3 vistas) con rasterizador propio en Python puro + z-buffer. Uso: preview.py in.obj out.png"""
import sys, math, zlib, struct
sys.path.insert(0, __import__('os').path.dirname(__import__('os').path.abspath(__file__)))
from meshlib import sub, cross, dot, norm

def load(path):
    V = []; C = []; F = []
    for ln in open(path):
        p = ln.split()
        if not p: continue
        if p[0] == 'v':
            V.append(tuple(map(float, p[1:4]))); C.append(tuple(map(float, p[4:7])) if len(p) >= 7 else (.7, .7, .7))
        elif p[0] == 'f':
            F.append(tuple(int(t.split('/')[0])-1 for t in p[1:4]))
    return V, C, F

def render(V, C, F, yaw, pitch, W=480, H=320, scale=None):
    cy, sy, cp, sp = math.cos(yaw), math.sin(yaw), math.cos(pitch), math.sin(pitch)
    def cam(v):
        x, y, z = v
        x, z = x*cy + z*sy, -x*sy + z*cy
        y, z = y*cp - z*sp, y*sp + z*cp
        return (x, y, z)   # +z = alejandose de la camara
    P = [cam(v) for v in V]
    ext = max(max(abs(p[0]) for p in P), max(abs(p[1]) for p in P)*1.5)
    s = scale or (W*0.46/ext)
    buf = [[(40, 46, 62)]*W for _ in range(H)]; zb = [[1e9]*W for _ in range(H)]
    L = norm((-0.4, 0.8, -0.5))
    for f in F:
        a, b, c = (P[i] for i in f)
        n = norm(cross(sub(b, a), sub(c, a)))
        # camara mira hacia +z; cara visible si normal apunta hacia -z
        wn = norm(cross(sub(V[f[1]], V[f[0]]), sub(V[f[2]], V[f[0]])))
        if n[2] > 0: continue
        sh = 0.35 + 0.65*max(0.0, -dot(n, (L[0], L[1], L[2])) if False else dot(n, (-L[0]*0+0.3, 0.6, -0.75)))
        pts = [(W/2 + p[0]*s, H/2 - p[1]*s, p[2]) for p in (a, b, c)]
        cols = [C[i] for i in f]
        x0 = max(0, int(min(p[0] for p in pts))); x1 = min(W-1, int(max(p[0] for p in pts))+1)
        y0 = max(0, int(min(p[1] for p in pts))); y1 = min(H-1, int(max(p[1] for p in pts))+1)
        (ax, ay, az), (bx, by, bz), (cx, cy2, cz) = pts
        den = (by-cy2)*(ax-cx) + (cx-bx)*(ay-cy2)
        if abs(den) < 1e-9: continue
        for y in range(y0, y1+1):
            for x in range(x0, x1+1):
                px, py = x+.5, y+.5
                w0 = ((by-cy2)*(px-cx) + (cx-bx)*(py-cy2))/den
                w1 = ((cy2-ay)*(px-cx) + (ax-cx)*(py-cy2))/den
                w2 = 1-w0-w1
                if w0 < 0 or w1 < 0 or w2 < 0: continue
                z = w0*az + w1*bz + w2*cz
                if z < zb[y][x]:
                    zb[y][x] = z
                    buf[y][x] = tuple(min(255, int(255*sh*(w0*cols[0][k]+w1*cols[1][k]+w2*cols[2][k]))) for k in range(3))
    return buf

def png(path, rows):
    H, W = len(rows), len(rows[0])
    raw = b''.join(b'\x00' + bytes(v for px in r for v in px) for r in rows)
    def ch(t, d): return struct.pack('>I', len(d)) + t + d + struct.pack('>I', zlib.crc32(t+d) & 0xffffffff)
    open(path, 'wb').write(b'\x89PNG\r\n\x1a\n' + ch(b'IHDR', struct.pack('>IIBBBBB', W, H, 8, 2, 0, 0, 0)) +
                           ch(b'IDAT', zlib.compress(raw, 9)) + ch(b'IEND', b''))

def sheet(obj, out, views=None):
    V, C, F = load(obj)
    views = views or [(math.pi/2, 0.0), (0.0, math.pi/2 - 0.001), (math.radians(35), 0.45)]
    # lateral: camara mira por el eje X; superior: mira hacia abajo; 3/4: yaw 35 + cabeceo
    # yaw=0 mira por +z (desde atras hacia el morro -Z visto de frente). Usamos yaw pi para ver de frente.
    imgs = [render(V, C, F, y, p) for y, p in views]
    rows = [sum((im[r] for im in imgs), []) for r in range(len(imgs[0]))]
    png(out, rows)

if __name__ == '__main__':
    if len(sys.argv) > 3 and sys.argv[3] == 'ship':
        sheet(sys.argv[1], sys.argv[2], [(math.pi/2, 0.0), (0.0, -(math.pi/2 - 0.001)), (math.radians(-35), -0.4)])
    else:
        sheet(sys.argv[1], sys.argv[2], [(0.4, 0.2), (2.2, 0.2), (3.9, 0.3)])
