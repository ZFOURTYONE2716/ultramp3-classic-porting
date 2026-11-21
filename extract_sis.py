import hashlib, json, pathlib, struct, sys, zlib
source = pathlib.Path(sys.argv[1])
data = source.read_bytes()
u16 = lambda o: struct.unpack_from('<H', data, o)[0]
u32 = lambda o: struct.unpack_from('<I', data, o)[0]
def chunk(offset, length):
    if offset < 0 or length < 0 or offset + length > len(data): raise ValueError('out of bounds')
    return data[offset:offset+length]
if u32(4) != 0x10003a12 or u32(8) != 0x10000419: raise ValueError('unsupported SIS format')
langs, count, options, pos = u16(18), u16(20), u16(36), u32(52)
out = pathlib.Path(__file__).parent / 'extracted'
out.mkdir(exist_ok=True)
manifest = []
for index in range(count):
    kind = u32(pos)
    if kind in (5, 6): pos += 4; continue
    if kind in (3, 4): pos += 8 + u32(pos+4); continue
    if kind == 2: pos += 24 + 8 * u32(pos+4) * langs; continue
    if kind not in (0,1): raise ValueError(f'unknown record {kind} at {pos:x}')
    n = 1 if kind == 0 else langs
    filetype = u32(pos+4)
    namelen, nameptr = u32(pos+20), u32(pos+24)
    name = chunk(nameptr, namelen).decode('utf-16le' if options & 1 else 'latin1')
    for lang in range(n):
        size, ptr, original = (u32(pos+28+4*lang), u32(pos+28+4*n+4*lang), u32(pos+28+8*n+4*lang))
        if not size: continue
        payload = chunk(ptr, size)
        if not options & 8: payload = zlib.decompress(payload)
        if len(payload) != original: raise ValueError(f'length mismatch at record {index}')
        basename = name.replace('\\','/').split('/')[-1] or 'unnamed'
        basename = ''.join(c if c.isalnum() or c in '._-' else '_' for c in basename)
        filename = f'{index:02d}_{lang}_{basename}'
        (out/filename).write_bytes(payload)
        manifest.append(dict(record=index, destination=name, file_type=filetype, file=filename, size=len(payload), sha256=hashlib.sha256(payload).hexdigest()))
    pos += 36+12*n
(out/'manifest.json').write_text(json.dumps(manifest, indent=2, ensure_ascii=False), encoding='utf-8')
print(json.dumps(manifest, indent=2, ensure_ascii=False))
