import struct, os

good_cat = r'C:\Users\Administrator\Documents\txt与gdb互转\test_arcpy\test.gdb\a00000001.gdbtable'
our_cat = r'C:\Users\Administrator\AppData\Local\Temp\jisig_debug_gdb\debug_gdb.gdb\a00000001.gdbtable'

for label, path in [('Known-good', good_cat), ('Our', our_cat)]:
    d = open(path, 'rb').read()
    print(f'{label} catalog: {len(d)} bytes')
    print(f'  Header (first 40): {d[:40].hex()}')
    ver = struct.unpack('<i', d[0:4])[0]
    nrec = struct.unpack('<i', d[4:8])[0]
    print(f'  ver={ver}, records={nrec}')
    
    fs_off = struct.unpack('<q', d[32:40])[0]
    if fs_off < len(d):
        fs_size = struct.unpack('<i', d[fs_off:fs_off+4])[0]
        fs_data = d[fs_off+4:fs_off+4+fs_size]
        ver2 = struct.unpack('<I', fs_data[0:4])[0]
        flags = struct.unpack('<I', fs_data[4:8])[0]
        nfields = struct.unpack('<h', fs_data[8:10])[0]
        print(f'  Field section: ver={ver2}, flags=0x{flags:x}, nfields={nfields}')

print()
good_items = r'C:\Users\Administrator\Documents\txt与gdb互转\test_arcpy\test.gdb\a00000004.gdbtable'
d = open(good_items, 'rb').read()
print(f'Known-good GDB_Items: {len(d)} bytes')
ver = struct.unpack('<i', d[0:4])[0]
nrec = struct.unpack('<i', d[4:8])[0]
print(f'  ver={ver}, records={nrec}')
for pat in [b'ROOT', b'plot_000', b'GDB_', b'Workspace']:
    idx = d.find(pat)
    if idx >= 0:
        name = pat.decode('ascii','ignore')
        print(f'  Found "{name}" at offset {idx}')
