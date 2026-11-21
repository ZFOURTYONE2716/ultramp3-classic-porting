"""Extract UltraMP3's LCG payload and map its verified code segment for analysis."""
import argparse
import hashlib
import struct
import zlib
from pathlib import Path

LIMIT = 16 * 1024 * 1024


def u32(data, offset):
    return struct.unpack_from("<I", data, offset)[0]


def unpack(data):
    if data[0x10:0x14] != b"EPOC":
        raise ValueError("expected the E32 AppLoader main.dll")
    code_size, code_offset = u32(data, 0x30), u32(data, 0x64)
    reloc = max(u32(data, 0x70), u32(data, 0x74))
    if code_size % 4 or not 0x7c <= code_offset <= code_offset + code_size <= reloc <= len(data) - 16:
        raise ValueError("unexpected E32 layout")
    key = sum(word for (word,) in struct.iter_unpack("<I", data[code_offset:code_offset + code_size]))
    key = (key & 0xffffffff).to_bytes(4, "little")
    tail = reloc + 8 + u32(data, reloc)
    packed_size, unpacked_size = struct.unpack_from("<II", data, tail)
    start = tail + 8
    if not 0 < unpacked_size <= LIMIT or not 0 < packed_size <= len(data) - start:
        raise ValueError("invalid payload sizes")
    packed = bytearray(data[start:start + packed_size])
    for index in range(packed_size & ~3):
        packed[index] ^= key[index % 4]
    decoder = zlib.decompressobj()
    payload = decoder.decompress(packed, unpacked_size + 1)
    if not decoder.eof or len(payload) != unpacked_size:
        raise ValueError("decompressed payload did not match its declared length")
    if payload[:8] != b"LCG!\x01\x00\x00\x00":
        raise ValueError("unexpected LCG payload magic/version")
    return payload


def code_elf(payload):
    """Wrap the original ARM bytes in an ELF for disassembly, with no imports resolved."""
    entry, address, size = struct.unpack_from("<III", payload, 0x0c)
    code = payload[0x38:0x38 + size]
    if not size or len(code) != size or not address <= (entry & ~1) < address + size:
        raise ValueError("invalid LCG code segment")
    code_offset = 0x1000
    names = b"\x00.text\x00.shstrtab\x00"
    names_offset = code_offset + len(code)
    sections_offset = (names_offset + len(names) + 3) & ~3
    elf = bytearray(sections_offset + 3 * 40)
    elf[:16] = b"\x7fELF\x01\x01\x01" + bytes(9)
    struct.pack_into("<HHIIIIIHHHHHH", elf, 16, 2, 40, 1, entry, 52, sections_offset,
                     0x05000000, 52, 32, 1, 40, 3, 2)
    struct.pack_into("<8I", elf, 52, 1, code_offset, address, address, size, size, 5, 0x1000)
    elf[code_offset:names_offset] = code
    elf[names_offset:names_offset + len(names)] = names
    struct.pack_into("<10I", elf, sections_offset + 40, 1, 1, 6, address, code_offset,
                     size, 0, 0, 4, 0)
    struct.pack_into("<10I", elf, sections_offset + 80, 7, 3, 0, 0, names_offset,
                     len(names), 0, 0, 1, 0)
    return elf


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path)
    parser.add_argument("payload", type=Path)
    parser.add_argument("--elf", type=Path)
    args = parser.parse_args()
    payload = unpack(args.input.read_bytes())
    args.payload.write_bytes(payload)
    if args.elf:
        args.elf.write_bytes(code_elf(payload))
    print(f"LCG payload: {len(payload)} bytes, SHA-256 {hashlib.sha256(payload).hexdigest()}")


if __name__ == "__main__":
    main()

