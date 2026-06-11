import struct, os, sys

def check_shp(path):
    if not os.path.exists(path): return
    d = open(path, 'rb').read()
    magic = struct.unpack('>i', d[0:4])[0]
    length = struct.unpack('>i', d[24:28])[0]
    stype = struct.unpack('<i', d[32:36])[0]
    print(f'SHP: magic={magic} (9994=ok), file_len_16bit_words={length}, shape_type={stype}')
    return stype

def check_dbf(path):
    if not os.path.exists(path): return
    dd = open(path, 'rb').read()
    nrec = struct.unpack('<I', dd[4:8])[0]
    hlen = struct.unpack('<H', dd[8:10])[0]
    rlen = struct.unpack('<H', dd[10:12])[0]
    lang = dd[29]
    nfields = (hlen - 33) // 32
    print(f'DBF: records={nrec}, header_len={hlen}, record_len={rlen}, lang=0x{lang:02x}, fields={nfields}')
    for i in range(nfields):
        off = 32 + i*32
        name = dd[off:off+11].split(b'\x00')[0].decode('ascii','ignore')
        ftype = chr(dd[off+11])
        flen = dd[off+16]
        fdec = dd[off+17]
        print(f'  [{i}] {name}: type={ftype}, len={flen}, dec={fdec}')

def check_prj(path):
    if not os.path.exists(path):
        print(f'PRJ: MISSING at {path}')
        return
    d = open(path, 'r', encoding='utf-8').read()[:300]
    print(f'PRJ: {d[:200]}...')

def check_shx(path):
    if not os.path.exists(path): return
    d = open(path, 'rb').read()
    magic = struct.unpack('>i', d[0:4])[0]
    length = struct.unpack('>i', d[24:28])[0]
    nrec = (length - 50) // 8
    print(f'SHX: magic={magic}, file_len_16bit={length}, records={nrec}')

base = 'test_data/44120000072_0'
for ext in ['.shp','.shx','.dbf','.prj']:
    path = base + ext
    if os.path.exists(path):
        print(f'\n--- {path} ---')
        if ext == '.shp': check_shp(path)
        elif ext == '.shx': check_shx(path)
        elif ext == '.dbf': check_dbf(path)
        elif ext == '.prj': check_prj(path)

# Also check test_polygon
print('\n\n--- test_polygon ---')
for ext in ['.shp','.shx','.dbf','.prj']:
    path = 'test_data/test_polygon' + ext
    if os.path.exists(path):
        print(f'\n--- {path} ---')
        if ext == '.shp': check_shp(path)
        elif ext == '.shx': check_shx(path)
        elif ext == '.dbf': check_dbf(path)
        elif ext == '.prj': check_prj(path)
