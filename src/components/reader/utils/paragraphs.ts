export interface ReaderParagraph {
  text: string;
  index: number;
  charOffset: number;
}

export function splitReaderParagraphs(content: string): ReaderParagraph[] {
  const paragraphs: ReaderParagraph[] = [];
  const parts = content.split(/\r?\n+/);
  let searchFrom = 0;

  for (const part of parts) {
    const text = part.trim();
    if (!text) {
      searchFrom += part.length + 1;
      continue;
    }

    const pos = content.indexOf(text, searchFrom);
    const charOffset = pos >= 0 ? pos : searchFrom;
    paragraphs.push({
      text,
      index: paragraphs.length,
      charOffset,
    });
    searchFrom = charOffset + text.length;
  }

  return paragraphs;
}
