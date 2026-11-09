import sys

def build_pdf():
    header = b"%PDF-1.4\n"
    objs = [
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n",
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R >>\nendobj\n",
        b"4 0 obj\n<< /Length 34 >>\nstream\nBT\n 70 730 Td\n 12 Tf\n 0 0 0 rg\n (This is a small real PDF) Tj\nET\nendstream\nendobj\n",
    ]
    offsets = []
    body = bytearray(header)
    for o in objs:
        offsets.append(len(body))
        body += o
    xref_body = b"xref\n0 5\n0000000000 65535 f \n"
    for off in offsets:
        xref_body += ("%010d 00000 n \n" % off).encode()
    trailer = b"trailer\n<< /Size 5 /Root 1 0 R >>\n"
    xref_data = body + xref_body + trailer
    # startxref offset: the byte where "startxref" keyword begins.
    startxref_offset = len(xref_data)
    startxref = b"startxref\n" + ("%010d" % startxref_offset).encode() + b"\n%%EOF\n"
    return xref_data + startxref

if __name__ == "__main__":
    out = sys.argv[1] if len(sys.argv) > 1 else "assets/problems/ImageJ-user-guide.pdf"
    data = build_pdf()
    open(out, "wb").write(data)
    print("wrote", out, len(data), "bytes")
