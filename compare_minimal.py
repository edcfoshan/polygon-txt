import arcpy, os, struct, shutil

# Create minimal GDB for comparison
temp_dir = r'C:\Users\Administrator\AppData\Local\Temp\jisig_debug_gdb'
minimal_gdb = os.path.join(temp_dir, 'minimal.gdb')
if os.path.exists(minimal_gdb):
    shutil.rmtree(minimal_gdb)

arcpy.management.CreateFileGDB(temp_dir, 'minimal')
sr = arcpy.SpatialReference(4490)  # CGCS2000
arcpy.management.CreateFeatureclass(minimal_gdb, 'test_fc', 'POLYGON', spatial_reference=sr)
arcpy.management.AddField(minimal_gdb + r'\test_fc', 'NAME', 'TEXT', field_length=50)

# Compare files
our_gdb = os.path.join(temp_dir, 'debug_gdb.gdb')

def list_files(gdb, label):
    print(f'\n=== {label} ===')
    for root, dirs, files in os.walk(gdb):
        for f in sorted(files):
            fp = os.path.join(root, f)
            sz = os.path.getsize(fp)
            rel = os.path.relpath(fp, gdb)
            print(f'  {rel}: {sz} bytes')

list_files(minimal_gdb, 'ArcPy minimal GDB')
list_files(our_gdb, 'Our GDB')

# Check gdb marker
print('\n=== gdb marker files ===')
for label, path in [('ArcPy', os.path.join(minimal_gdb, 'gdb')), ('Ours', os.path.join(our_gdb, 'gdb'))]:
    d = open(path, 'rb').read()
    print(f'{label}: {len(d)} bytes, hex={d.hex()}')

# Check catalog fields
print('\n=== Catalog field comparison ===')
for label, path in [('ArcPy', os.path.join(minimal_gdb, 'a00000001.gdbtable')), ('Ours', os.path.join(our_gdb, 'a00000001.gdbtable'))]:
    d = open(path, 'rb').read()
    ver = struct.unpack('<i', d[0:4])[0]
    nrec = struct.unpack('<i', d[4:8])[0]
    fs_off = struct.unpack('<q', d[32:40])[0]
    fs_size = struct.unpack('<i', d[fs_off:fs_off+4])[0]
    fs_data = d[fs_off+4:fs_off+4+fs_size]
    nfields = struct.unpack('<h', fs_data[8:10])[0]
    print(f'{label}: ver={ver}, records={nrec}, nfields={nfields}')
    # Parse field names
    pos = 10
    for i in range(nfields):
        name_len = fs_data[pos]
        pos += 1
        name_bytes = fs_data[pos:pos+name_len*2]
        pos += name_len * 2
        alias_len = fs_data[pos]
        pos += 1
        alias_bytes = fs_data[pos:pos+alias_len*2]
        pos += alias_len * 2
        ftype = fs_data[pos]
        width = struct.unpack('<I', fs_data[pos+1:pos+5])[0]
        pos += 9  # type(1) + width(4) + flags(1) + precision(1) + reserved(2)
        name = name_bytes.decode('utf-16-le')
        print(f'  Field[{i}]: "{name}" type={ftype} width={width}')

# Check GDB_Items features
print('\n=== GDB_Items: feature class listing ===')
for label, path in [('ArcPy', os.path.join(minimal_gdb, 'a00000004.gdbtable')), ('Ours', os.path.join(our_gdb, 'a00000004.gdbtable'))]:
    d = open(path, 'rb').read()
    ver = struct.unpack('<i', d[0:4])[0]
    nrec = struct.unpack('<i', d[4:8])[0]
    print(f'{label}: {len(d)} bytes, {nrec} rows')
    # Find Name strings
    for pat in [b'ROOT', b'test_fc', b'debug_gdb', b'GDB_']:
        idx = d.find(pat)
        if idx >= 0:
            print(f'  Found "{pat.decode()}" at {idx}')
