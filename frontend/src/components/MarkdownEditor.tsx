import { useEffect, useRef } from "react";
import { EditorView, keymap, lineNumbers, highlightActiveLineGutter, highlightSpecialChars, drawSelection, rectangularSelection, crosshairCursor, highlightActiveLine, placeholder } from "@codemirror/view";
import { EditorState, Compartment, Annotation } from "@codemirror/state";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { syntaxHighlighting, HighlightStyle, bracketMatching, foldGutter, foldKeymap, indentOnInput } from "@codemirror/language";
import { closeBrackets, closeBracketsKeymap, completionKeymap } from "@codemirror/autocomplete";
import { highlightSelectionMatches, searchKeymap } from "@codemirror/search";
import { oneDark } from "@codemirror/theme-one-dark";
import { tags } from "@lezer/highlight";

const ExternalChange = Annotation.define<boolean>();

const darkThemeCompartment = new Compartment();
const readOnlyCompartment = new Compartment();

const customHighlightStyle = HighlightStyle.define([
  { tag: tags.heading, fontWeight: "bold", fontSize: "1.1em" },
  { tag: tags.strong, fontWeight: "bold" },
  { tag: tags.emphasis, fontStyle: "italic" },
  { tag: tags.link, color: "hsl(var(--primary))", textDecoration: "underline" },
  { tag: tags.url, color: "hsl(var(--primary))", textDecoration: "underline" },
  { tag: tags.strikethrough, textDecoration: "line-through" },
  { tag: tags.monospace, fontFamily: "var(--font-mono, monospace)", background: "hsl(var(--muted))" },
  { tag: tags.quote, fontStyle: "italic", color: "hsl(var(--muted-foreground))" },
  { tag: tags.list, color: "hsl(var(--foreground))" },
  { tag: tags.comment, color: "hsl(var(--muted-foreground))" },
]);

interface MarkdownEditorProps {
  value: string;
  onChange: (value: string) => void;
  readOnly?: boolean;
  className?: string;
  onViewReady?: (view: EditorView) => void;
}

export function MarkdownEditor({ value, onChange, readOnly = false, className, onViewReady }: MarkdownEditorProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  useEffect(() => {
    if (!containerRef.current) return;

    const isDark = document.documentElement.classList.contains("dark");

    const updateListener = EditorView.updateListener.of((update) => {
      if (update.docChanged && !update.transactions.some((t) => t.annotation(ExternalChange))) {
        onChangeRef.current(update.state.doc.toString());
      }
    });

    const state = EditorState.create({
      doc: value,
      extensions: [
        lineNumbers(),
        highlightActiveLineGutter(),
        highlightSpecialChars(),
        history(),
        foldGutter(),
        drawSelection(),
        EditorState.allowMultipleSelections.of(true),
        indentOnInput(),
        bracketMatching(),
        closeBrackets(),
        rectangularSelection(),
        crosshairCursor(),
        highlightActiveLine(),
        highlightSelectionMatches(),
        keymap.of([
          ...defaultKeymap,
          ...searchKeymap,
          ...historyKeymap,
          ...foldKeymap,
          ...closeBracketsKeymap,
          ...completionKeymap,
        ]),
        markdown({ base: markdownLanguage }),
        syntaxHighlighting(customHighlightStyle),
        updateListener,
        darkThemeCompartment.of(isDark ? oneDark : []),
        readOnlyCompartment.of(EditorState.readOnly.of(readOnly)),
        placeholder("Markdown content..."),
      ],
    });

    const view = new EditorView({
      state,
      parent: containerRef.current,
    });

    viewRef.current = view;
    onViewReady?.(view);

    const observer = new MutationObserver(() => {
      const dark = document.documentElement.classList.contains("dark");
      view.dispatch({
        effects: darkThemeCompartment.reconfigure(dark ? oneDark : []),
      });
    });

    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });

    return () => {
      observer.disconnect();
      view.destroy();
      viewRef.current = null;
    };
  }, []);

  useEffect(() => {
    if (viewRef.current) {
      viewRef.current.dispatch({
        effects: readOnlyCompartment.reconfigure(EditorState.readOnly.of(readOnly)),
      });
    }
  }, [readOnly]);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    const current = view.state.doc.toString();
    if (current !== value) {
      view.dispatch({
        changes: { from: 0, to: current.length, insert: value },
        annotations: [ExternalChange.of(true)],
      });
    }
  }, [value]);

  return (
    <div
      ref={containerRef}
      className={className}
      style={{ height: "100%", overflow: "hidden" }}
    />
  );
}