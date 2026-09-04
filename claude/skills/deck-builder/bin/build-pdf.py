#!/usr/bin/env python
"""build-pdf.py <pngDir> <out.pdf> [--page-width-pt 960]

Assemble slide PNGs (slide-NN.png, sorted) into a PDF. Page height follows the
first image's aspect ratio. Used instead of LibreOffice, which hangs on this Mac.
"""
import sys, os, pymupdf as fitz

def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    src, out = sys.argv[1], sys.argv[2]
    width = float(sys.argv[4]) if len(sys.argv) > 4 and sys.argv[3] == '--page-width-pt' else 960.0
    pngs = sorted(f for f in os.listdir(src) if f.startswith('slide-') and f.endswith('.png'))
    if not pngs:
        sys.exit(f'build-pdf: no slide-NN.png in {src}')
    first = fitz.Pixmap(os.path.join(src, pngs[0]))
    height = width * first.height / first.width
    doc = fitz.open()
    for f in pngs:
        page = doc.new_page(width=width, height=height)
        page.insert_image(page.rect, filename=os.path.join(src, f))
    doc.save(out, deflate=True, garbage=3)
    print(f'pdf: {out} ({len(pngs)} pages, {os.path.getsize(out)//1024} KB)')

if __name__ == '__main__':
    main()
