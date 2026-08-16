import sys
sys.stdout.reconfigure(encoding="utf-8")

# Fix Cargo.toml
with open("Cargo.toml", "r", encoding="utf-8") as f:
    content = f.read()

content = content.replace(
    'pdfium-bundled = "0.1"',
    'pdfium-bundled = "0.1"\npdfium-render = "0.9"\nimage = "0.25"'
)
content = content.replace('oar-ocr = 0.9"', 'oar-ocr = "0.9"')

with open("Cargo.toml", "w", encoding="utf-8") as f:
    f.write(content)
print("Cargo.toml fixed")

# Fix pipeline.rs
with open("src/pipeline.rs", "r", encoding="utf-8") as f:
    content = f.read()

content = content.replace(
    "use pdfium_render::prelude::*;",
    "use pdfium_bundled::pdfium_render::prelude::*;"
)
content = content.replace("page.get_width()", "page.width()")
content = content.replace("page.get_height()", "page.height()")

with open("src/pipeline.rs", "w", encoding="utf-8") as f:
    f.write(content)
print("pipeline.rs fixed")
