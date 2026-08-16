import type { ConversionStats } from "../types";

/**
 * Count markdown table separator rows (e.g. `|---|---|`), ignoring code blocks.
 * Mirrors `markdown_pipeline::count_table_separators` in the Rust backend.
 */
export function countTableSeparators(markdown: string): number {
  let count = 0;
  let inCodeBlock = false;
  for (const line of markdown.split("\n")) {
    if (line.trimStart().startsWith("```")) {
      inCodeBlock = !inCodeBlock;
      continue;
    }
    if (inCodeBlock) continue;
    if (isTableSeparator(line)) count++;
  }
  return count;
}

function isTableSeparator(line: string): boolean {
  const trimmed = line.trim();
  if (!trimmed.includes("|")) return false;
  const inner = trimmed.replace(/^\|/, "").replace(/\|$/, "").trim();
  if (!inner) return false;
  return inner.split("|").every((cell) => /^:?-+:?$/.test(cell.trim()));
}

/**
 * Count words in markdown, ignoring code blocks and inline code.
 *
 * CJK characters (Han, Hiragana, Katakana, Hangul, fullwidth forms) each count
 * as one word; Latin/other scripts count whitespace-delimited words. This
 * mirrors how word processors count CJK documents.
 */
export function countWords(markdown: string): number {
  let count = 0;
  let inCodeBlock = false;
  let inInlineCode = false;
  let latinBuf = "";

  const flush = () => {
    if (latinBuf.trim().length > 0) {
      count += latinBuf.trim().split(/\s+/).filter((w) => w.length > 0).length;
    }
    latinBuf = "";
  };

  for (const line of markdown.split("\n")) {
    if (line.trimStart().startsWith("```")) {
      inCodeBlock = !inCodeBlock;
      continue;
    }
    if (inCodeBlock) continue;

    for (const ch of line) {
      if (ch === "`") {
        flush();
        inInlineCode = !inInlineCode;
        continue;
      }
      if (inInlineCode) continue;
      if (isCjk(ch)) {
        flush();
        count++;
      } else {
        latinBuf += ch;
      }
    }
    latinBuf += " ";
  }
  flush();
  return count;
}

function isCjk(c: string): boolean {
  const code = c.codePointAt(0) ?? 0;
  return (
    (code >= 0x4e00 && code <= 0x9fff) ||
    (code >= 0x3400 && code <= 0x4dbf) ||
    (code >= 0xf900 && code <= 0xfaff) ||
    (code >= 0x3040 && code <= 0x309f) ||
    (code >= 0x30a0 && code <= 0x30ff) ||
    (code >= 0xac00 && code <= 0xd7af) ||
    (code >= 0xff00 && code <= 0xffef)
  );
}

/**
 * Recompute the stats that can be derived purely from markdown content,
 * used when previewing from history (which doesn't persist runtime stats).
 */
export function deriveStatsFromMarkdown(markdown: string): ConversionStats {
  return {
    imageCount: (markdown.match(/!\[[^\]]*\]\([^)]*\)/g) || []).length,
    tableCount: countTableSeparators(markdown),
    wordCount: countWords(markdown),
  };
}
