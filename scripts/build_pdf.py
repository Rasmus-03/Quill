import sys

def build_pdf():
    header = b"%PDF-1.4\n"
    objs = [
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n",
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R >>\nendobj\n",
        b"4 0 obj\n<< /Length 65 >>\nstream\nBT\n 70 730 Td\n 12 Tf\n 0 0 0 rg\n (This is a small real PDF) Tj\nET\nendstream\nendobj\n",
    ]
    offsets = {}
    body = bytearray(header)
    for i,o in enumerate(objs,1):
        offsets[i] = len(body)
        body += o
    xref_body = b"xref\n0 5\n0000000000 65535 f \n"
    for i in range(1,5):
        xref_body += ("%010d 00000 n \n" % offsets[i]).encode()
    trailer = b"trailer\n<< /Size 5 /Root 1 0 R >>\n"
    xref_data = body + xref_body + trailer
    # startxref points to the byte offset of the 'xref' keyword.
    startxref_offset = xref_data.find(b"xref")
    startxref = b"startxref\n" + ("%010d" % startxref_offset).encode() + b"\n%%EOF\n"
    return xref_data + startxref

data = build_pdf()
open("assets/problems/ImageJ-user-guide.pdf","wb").write(data)
print("wrote", len(data), "bytes")
