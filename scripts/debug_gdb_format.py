# -*- coding: utf-8 -*-
import os, struct

ref_gdb = r'C:\Users\Administrator\Documents\txt与gdb互转\test_arcpy\test.gdb'
gen_gdb = os.path.join(os.environ['TEMP'], 'jisig_debug_gdb', 'debug_gdb.gdb')

def dump_file(path, label):
    with open(path, 'rb') as f:
        data = f.read()
    print(f'{label}: {len(data)} bytes')
    print(f'  hex: {data.hex()}')
    return data

# Compare a00000001.gdbindexes
print('=== a00000001.gdbindexes ===')
ref_data = dump_file(os.path.join(ref_gdb, 'a00000001.gdbindexes'), 'Reference')
gen_data = dump_file(os.path.join(gen_gdb, 'a00000001.gdbindexes'), 'Generated')
print()

# Parse reference format step by step
print('=== Parsing reference gdbindexes ===')
pos = 0
n = struct.unpack_from('<I', ref_data, pos)[0]; pos += 4
print(f'num_indexes: {n}')
for i in range(n):
    name_charlen = struct.unpack_from('<I', ref_data, pos)[0]; pos += 4
    name = ref_data[pos:pos+name_charlen*2].decode('utf-16-le')
    pos += name_charlen * 2
    print(f'  Index {i}: name="{name}" (charlen={name_charlen})')
    
    # Read until we find field_count
    idx_start = pos
    while pos < len(ref_data):
        val = struct.unpack_from('<I', ref_data, pos)[0]
        if val == 1 or val == 2 or val == 3:
            # Check if this could be field_count (small number)
            # Look ahead to see if it makes sense
            break
        pos += 1
    unknown = ref_data[idx_start:pos]
    print(f'    gap bytes: {unknown.hex()}')
    
    field_count = struct.unpack_from('<I', ref_data, pos)[0]; pos += 4
    print(f'    field_count: {field_count}')
    for j in range(field_count):
        fname_charlen = struct.unpack_from('<I', ref_data, pos)[0]; pos += 4
        fname = ref_data[pos:pos+fname_charlen*2].decode('utf-16-le')
        pos += fname_charlen * 2
        print(f'    field[{j}]: "{fname}"')
        # Read trailing bytes for this field
        trail_start = pos
        if j < field_count - 1 or i < n - 1:
            # There should be more data; skip to next
            while pos < len(ref_data):
                val = struct.unpack_from('<I', ref_data, pos)[0]
                if val <= 20 and pos + 4 < len(ref_data):
                    next_val = struct.unpack_from('<I', ref_data, pos+4)[0]
                    if next_val == 1 or next_val == 2 or next_val == 3 or next_val == 4:
                        break
                pos += 1
        trail = ref_data[trail_start:pos]
        print(f'    trail: {trail.hex()}')

print()

# Now compare the catalog table (a00000001.gdbtable)
print('=== a00000001.gdbtable comparison ===')
ref_tbl = dump_file(os.path.join(ref_gdb, 'a00000001.gdbtable'), 'Reference')
gen_tbl = dump_file(os.path.join(gen_gdb, 'a00000001.gdbtable'), 'Generated')

# Parse both
for label, tbl in [('Ref', ref_tbl), ('Gen', gen_tbl)]:
    print(f'\n--- {label} catalog ---')
    ver = struct.unpack_from('<I', tbl, 0)[0]
    nrec = struct.unpack_from('<I', tbl, 4)[0]
    maxrow = struct.unpack_from('<I', tbl, 8)[0]
    c5 = struct.unpack_from('<I', tbl, 12)[0]
    fsz = struct.unpack_from('<Q', tbl, 24)[0]
    fd_off = struct.unpack_from('<Q', tbl, 32)[0]
    print(f'  version={ver} records={nrec} maxrow={maxrow} c5={c5} filesize={fsz} fd_offset={fd_off}')
    
    # Parse field descriptors
    pos = fd_off
    sec_size = struct.unpack_from('<I', tbl, pos)[0]; pos += 4
    fmt_ver = struct.unpack_from('<I', tbl, pos)[0]; pos += 4
    flags = struct.unpack_from('<I', tbl, pos)[0]; pos += 4
    nfields = struct.unpack_from('<h', tbl, pos)[0]; pos += 2
    print(f'  section_size={sec_size} fmt_ver={fmt_ver} flags={hex(flags)} nfields={nfields}')
    
    for fi in range(nfields):
        name_len = tbl[pos]; pos += 1
        name = tbl[pos:pos+name_len*2].decode('utf-16-le')
        pos += name_len * 2
        alias_len = tbl[pos]; pos += 1
        if alias_len > 0:
            pos += alias_len * 2
        ftype = tbl[pos]; pos += 1
        print(f'    field[{fi}]: "{name}" type={ftype}')
        if ftype == 4:  # String
            max_len = struct.unpack_from('<I', tbl, pos)[0]; pos += 4
            flag = tbl[pos]; pos += 1
            dlen = tbl[pos]; pos += 1
            print(f'      string max_len={max_len} flag={flag} dlen={dlen}')
        elif ftype == 6:  # ObjectId
            w = tbl[pos]; pos += 1
            flag = tbl[pos]; pos += 1
            print(f'      objectid width={w} flag={flag}')
    
    # Parse rows
    rows_start = 40 + sec_size + 4
    pos = rows_start
    for ri in range(nrec):
        row_len = struct.unpack_from('<i', tbl, pos)[0]; pos += 4
        row_start = pos
        # null bitmap (1 byte for 1 nullable field)
        null_bm = tbl[pos]; pos += 1
        # Name string
        name_byte_len = tbl[pos]; pos += 1
        name = tbl[pos:pos+name_byte_len].decode('utf-8')
        pos += name_byte_len
        print(f'    row[{ri}]: null_bm={null_bm} name="{name}"')

print()
# Compare a00000001.TablesByName.atx
print('=== a00000001.TablesByName.atx ===')
ref_atx = dump_file(os.path.join(ref_gdb, 'a00000001.TablesByName.atx'), 'Reference')
gen_atx = dump_file(os.path.join(gen_gdb, 'a00000001.TablesByName.atx'), 'Generated')
print(f'Ref first 32: {ref_atx[:32].hex()}')
print(f'Gen first 32: {gen_atx[:32].hex()}')
