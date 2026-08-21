import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import type { Extension } from "@codemirror/state";
import { Prec } from "@codemirror/state";
import { tags as t } from "@lezer/highlight";

const highlightStyle = HighlightStyle.define([
  { tag: t.heading, color: "var(--syntax-heading)", fontWeight: "700" },
  { tag: t.heading1, color: "var(--syntax-heading)", fontWeight: "700" },
  { tag: t.heading2, color: "var(--syntax-heading)", fontWeight: "700" },
  { tag: t.heading3, color: "var(--syntax-heading)", fontWeight: "600" },
  { tag: t.comment, color: "var(--syntax-comment)", fontStyle: "italic" },
  { tag: t.link, color: "var(--syntax-link)" },
  { tag: t.url, color: "var(--syntax-link)" },
  { tag: t.monospace, color: "var(--syntax-code)" },
  { tag: t.emphasis, fontStyle: "italic" },
  { tag: t.strong, fontWeight: "700" },
  { tag: t.meta, color: "var(--text-muted)" },
  { tag: t.processingInstruction, color: "var(--text-faint)" },
  { tag: t.punctuation, color: "var(--text-muted)" },
]);

export function editorHighlight(): Extension {
  return Prec.high(syntaxHighlighting(highlightStyle));
}
