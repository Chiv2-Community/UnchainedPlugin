#!/usr/bin/env python3
"""Read-only check of the production IPv6 signatures against supplied PE files."""
import pathlib
import re
import struct
import sys


def check(path):
    data = path.read_bytes()
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    assert data[pe:pe + 4] == b"PE\0\0", "Not a PE file"
    base = struct.unpack_from("<Q", data, pe + 48)[0]
    count = struct.unpack_from("<H", data, pe + 6)[0]
    optional_size = struct.unpack_from("<H", data, pe + 20)[0]
    sections = [struct.unpack_from("<IIII", data, pe + 24 + optional_size + 40 * i + 8)
                for i in range(count)]

    def va(offset):
        return next(base + address + offset - raw
                    for _, address, size, raw in sections if raw <= offset < raw + size)

    source = (pathlib.Path(__file__).resolve().parents[1] / "src/resolvers/ipv6/mod.rs").read_text()
    locations = {}
    for name, patterns in re.findall(r'define_pattern_resolver!\(\s*(\w+),\s*\[(.*?)\]\s*\)', source, re.DOTALL):
        matches = []
        for pattern in re.findall(r'"([^"]+)"', patterns):
            tokens = pattern.split()
            marker = tokens.index("|") if "|" in tokens else 0
            tokens = [token for token in tokens if token != "|"]
            regex = b"".join(
                b"[" + b"".join(re.escape(bytes([byte])) for byte in range(256)
                               if all(char == "?" or char.lower() == f"{byte:02x}"[i]
                                      for i, char in enumerate(token))) + b"]"
                for token in tokens
            )
            matches.extend(match.start() + marker for match in re.finditer(regex, data, re.DOTALL))
        assert len(matches) == 1, f"{name}: expected one match, got {len(matches)}"
        offset = matches[0]
        locations[name] = (va(offset), offset)
        print(f"  {name}: {va(offset):#x}")

    assert len(locations) == 6, "Expected all six production resolvers"

    for name in ["Ipv6BrowserFind", "Ipv6PingFind", "Ipv6PendingPingFind", "Ipv6UdpSplit"]:
        address, offset = locations[name]
        assert data[offset] == 0xE8, f"{name}: not an E8 call"
        target = address + 5 + struct.unpack_from("<i", data, offset + 1)[0]
        expected = locations["FStringParseIntoArray" if name == "Ipv6UdpSplit" else "FStringFind"][0]
        assert target == expected, f"{name}: unexpected callee {target:#x}"
        assert address & 0xFFF <= 0xFFB, f"{name}: crosses a page boundary"


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit("Usage: python3 tests/check_ipv6_signatures.py <game.exe> [<game.exe> ...]")
    for filename in sys.argv[1:]:
        print(filename)
        check(pathlib.Path(filename))
