import sys
sys.stdout.reconfigure(encoding="utf-8")

# Read the current file
with open("src/pipeline.rs", "r", encoding="utf-8") as f:
    content = f.read()

# Check what version we have
if "try_ocr_to_result" in content:
    print("File already has OCR functions - checking for pdfium")
    if "ocr_pdf_images" in content:
        print("Already has pdfium OCR")
    else:
        print("Need to add pdfium OCR")
else:
    print("Original version - need to rewrite with OCR + pdfium")
    # Write the complete file
    new_content = open("fix_pipeline2.py", "r", encoding="utf-8").read()
    # This won't work - fix_pipeline2.py is not the content

print("File length:", len(content))
