#!/usr/bin/env python3
"""Generate the parity corpus: VINs that describe real models, built from vPIC patterns.

For a sampled model pattern, other patterns of the same schema are merged in, a
model year inside the schema's range is chosen, and the check digit is computed.
The serial number is always 000001, so no VIN here belongs to a real vehicle.
A few VINs already published as examples are added at the end.

The random seed is fixed: the same data file always gives the same corpus.

    python3 tools/corpus/generate.py DATA_FILE OUTPUT_JSON
"""
import collections
import json
import random
import sqlite3
import sys

if len(sys.argv) != 3:
    sys.exit(__doc__)
DB, OUTPUT = sys.argv[1], sys.argv[2]
PUBLISHED_EXAMPLES = ["1HGCM82633A004352", "KM8K2CAB4PU001140", "5YJSA1E26HF000001", "1M8GDM9AXKP042788"]
random.seed(20260930)
con=sqlite3.connect(DB)
YEAR="ABCDEFGHJKLMNPRSTVWXY123456789"
T={**{str(i):i for i in range(10)},**dict(zip("ABCDEFGH",range(1,9))),**dict(zip("JKLMN",range(1,6))),"P":7,"R":9,**dict(zip("STUVWXYZ",range(2,10)))}
W=[8,7,6,5,4,3,2,10,0,9,8,7,6,5,4,3,2]
def check(v):
    r=sum(T[c]*w for c,w in zip(v,W))%11
    return 'X' if r==10 else str(r)
def tokens(keys):
    out=[];i=0
    while i<len(keys):
        c=keys[i]
        if c=='*': out.append(None); i+=1
        elif c=='[':
            j=keys.find(']',i)
            if j<0: return None
            body=keys[i+1:j]
            if body.startswith('^') or not body: return None
            s=set();k=0
            while k<len(body):
                if k+2<len(body) and body[k+1]=='-':
                    s|={chr(x) for x in range(ord(body[k]),ord(body[k+2])+1)}; k+=3
                else: s.add(body[k]); k+=1
            out.append(s); i=j+1
        else: out.append({c}); i+=1
    return out
VALID=set("ABCDEFGHJKLMNPRSTUVWXYZ0123456789")
def merge(cons, keys):
    t=tokens(keys)
    if t is None or len(t)>14: return None
    new=list(cons)
    for i,x in enumerate(t):
        if x is None: continue
        if i==5:
            if x!={'|'}: return None
            continue
        x=x&VALID
        if new[i] is None: new[i]=x
        else:
            new[i]=new[i]&x
        if not new[i]: return None
    return new
def make_vin(wmi, light, schema, yf, yt, model_keys):
    cons=merge([None]*14, model_keys)
    if cons is None: return None
    rows=con.execute("SELECT keys, element_id FROM pattern WHERE schema_id=? AND element_id IN (38,34,5,15,13,11,12,9,24,31,75,37,14) ORDER BY RANDOM() LIMIT 40",(schema,)).fetchall()
    seen=set()
    for keys,el in rows:
        if el in seen: continue
        m=merge(cons,keys)
        if m is not None: cons=m; seen.add(el)
    yt=min(yt or 2026,2026); yf=max(yf,1981)
    if yf>yt: return None
    years=list(range(yf,yt+1)); random.shuffle(years)
    for year in years:
        code=YEAR[(year-1980)%30]
        if cons[6] is not None and code not in cons[6]: continue
        p7=cons[3]
        want_digit = year<2010
        if light:
            pool=[c for c in (p7 or VALID) if c.isdigit()==want_digit]
            if not pool: continue
            c7=sorted(pool)[0]
        else:
            c7=sorted(p7)[0] if p7 else 'A'
        vds=[]
        for i in range(5):
            if i==3: vds.append(c7)
            else: vds.append(sorted(cons[i])[0] if cons[i] else 'A')
        vis=[code]
        for i in range(7,14):
            s=cons[i]
            default='A' if i==7 else '0'
            if i==13: default='1'
            if s:
                digits=sorted(c for c in s if c.isdigit())
                vis.append((digits or sorted(s))[0] if i>=9 else sorted(s)[0])
            else: vis.append(default)
        if len(wmi)==6:
            # low-volume: positions 12-14 carry the rest of the code
            vis[2:5]=list(wmi[3:6])
        v=wmi[:3]+"".join(vds)+"0"+"".join(vis)
        if len(v)!=17 or any(c not in VALID for c in v): return None
        v=v[:8]+check(v)+v[9:]
        return v,year
    return None
def sample(where, n, params=()):
    out=[]
    rows=con.execute(f"""SELECT w.code, w.light_vehicle, w.vehicle_type, s.schema_id, s.year_from, s.year_to, p.keys, p.value, p.make
        FROM wmi w JOIN wmi_schema s ON s.wmi=w.code JOIN pattern p ON p.schema_id=s.schema_id AND p.element_id=28
        WHERE {where} ORDER BY RANDOM() LIMIT ?""",(*params,n*6)).fetchall()
    used=set()
    for code,light,vt,schema,yf,yt,keys,model,make in rows:
        if len(out)>=n: break
        if (code,model) in used: continue
        r=make_vin(code,bool(light),schema,yf,yt,keys)
        if r:
            used.add((code,model)); out.append({"vin":r[0],"year":r[1],"wmi":code,"vehicle_type":vt,"model":model,"make":make})
    return out
corpus=[]
for make in ["Toyota","Honda","Ford","Chevrolet","Nissan","Hyundai","Kia","Jeep","Ram","GMC","Subaru","Volkswagen","BMW","Mercedes-Benz","Mazda","Tesla","Dodge","Lexus","Audi","Acura","Chrysler","Buick","Cadillac","Volvo","Mitsubishi"]:
    corpus+=sample("p.make = ? COLLATE NOCASE AND w.light_vehicle=1",10,(make,))
corpus+=sample("w.light_vehicle=1",60)
corpus+=sample("w.vehicle_type = 'Truck' AND w.light_vehicle = 1",40)
for vt in ["Truck","Motorcycle","Trailer","Bus","Incomplete Vehicle","Low Speed Vehicle (LSV)","Off Road Vehicle"]:
    corpus+=sample("w.vehicle_type LIKE ? AND w.light_vehicle=0",15,(vt+"%",))
vins = sorted({entry["vin"] for entry in corpus} | set(PUBLISHED_EXAMPLES))
with open(OUTPUT, "w") as handle:
    handle.write("[\n" + ",\n".join(json.dumps(vin) for vin in vins) + "\n]\n")
by_type = collections.Counter((entry["vehicle_type"] or "?") for entry in corpus)
print(f"{len(vins)} VINs written to {OUTPUT}")
for vehicle_type, count in by_type.most_common():
    print(f"  {count:4} {vehicle_type}")
