import pypdf

reader = pypdf.PdfReader("/Users/saumyathacker/Desktop/rag_sys/T_AI_Personal_Assistant_MVP.pdf")
print("Total pages:", len(reader.pages))

text_content = []
for i, page in enumerate(reader.pages):
    text_content.append(f"--- PAGE {i+1} ---")
    text_content.append(page.extract_text())

with open("/Users/saumyathacker/Desktop/rag_sys/scratch/pdf_content.txt", "w", encoding="utf-8") as f:
    f.write("\n".join(text_content))

print("PDF text content successfully extracted to scratch/pdf_content.txt")
