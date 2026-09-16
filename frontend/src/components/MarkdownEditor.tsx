import { useEffect, useRef } from "react";
import { EditorView, keymap, lineNumbers, highlightActiveLineGutter, highlightSpecialChars, drawSelection, rectangularSelection, crosshairCursor, highlightActiveLine, placeholder } from "@codemirror/view";
import { EditorState, Compartment, Annotation } from "@codemirror/state";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { syntaxHighlighting, HighlightStyle, bracketMatching, foldGutter, foldKeymap, indentOnInput } from "@codemirror/language";
import { closeBrackets, closeBracketsKeymap, completionKeymap } from "@codemirror/autocomplete";
import { highlightSelectionMatches, searchKeymap } from "@codemirror/search";
import { tags } from "@lezer/highlight";
import { markdownKeymap } from "../lib/markdownCommands";

const readOnlyCompartment = new Compartment();

const ExternalChange = Annotation.define<boolean>();

const EDITOR_SANS =
  '"Inter Variable", "PingFang SC", "Microsoft YaHei", system-ui, sans-serif';
const EDITOR_MONO =
  'ui-monospace, SFMono-Regular, Consolas, "Liberation Mono", Menlo, monospace';

/**
 * 写作向主题：全部取自应用主题变量，深浅色自动适配（无需随主题切换
 * 重配置）。正文无衬线、代码等宽，标题分级放大，光标/选区/活动行走品牌紫。
 */
const editorTheme = EditorView.theme({
  "&": {
    backgroundColor: "transparent",
    color: "hsl(var(--foreground))",
  },
  ".cm-scroller": {
    lineHeight: "1.75",
  },
  ".cm-content": {
    fontFamily: EDITOR_SANS,
    fontSize: "15px",
    padding: "16px 20px",
    caretColor: "hsl(var(--primary))",
  },
  ".cm-cursor, .cm-dropCursor": {
    borderLeftColor: "hsl(var(--primary))",
    borderLeftWidth: "2px",
  },
  "&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground":
    { backgroundColor: "hsl(var(--primary) / 0.18)" },
  ".cm-selectionMatch": { backgroundColor: "hsl(var(--primary) / 0.12)" },
  ".cm-activeLine": { backgroundColor: "hsl(var(--primary) / 0.05)" },
  ".cm-gutters": {
    backgroundColor: "transparent",
    border: "none",
    color: "hsl(var(--muted-foreground) / 0.5)",
    fontSize: "12.5px",
  },
  ".cm-activeLineGutter": {
    backgroundColor: "transparent",
    color: "hsl(var(--foreground))",
  },
  ".cm-placeholder": { color: "hsl(var(--muted-foreground) / 0.6)" },
  ".cm-foldPlaceholder": {
    backgroundColor: "hsl(var(--muted))",
    border: "none",
    color: "hsl(var(--muted-foreground))",
  },
});

const markdownHighlight = HighlightStyle.define([
  { tag: tags.heading1, fontWeight: "700", fontSize: "1.35em", color: "hsl(var(--foreground))" },
  { tag: tags.heading2, fontWeight: "650", fontSize: "1.22em", color: "hsl(var(--foreground))" },
  { tag: tags.heading3, fontWeight: "600", fontSize: "1.1em", color: "hsl(var(--foreground))" },
  { tag: tags.heading4, fontWeight: "600", color: "hsl(var(--foreground))" },
  { tag: tags.heading5, fontWeight: "600", color: "hsl(var(--foreground))" },
  { tag: tags.heading6, fontWeight: "600", color: "hsl(var(--muted-foreground))" },
  { tag: tags.strong, fontWeight: "650", color: "hsl(var(--foreground))" },
  { tag: tags.emphasis, fontStyle: "italic" },
  { tag: tags.strikethrough, textDecoration: "line-through", color: "hsl(var(--muted-foreground))" },
  { tag: tags.link, color: "hsl(var(--primary))" },
  { tag: tags.url, color: "hsl(var(--primary))", textDecoration: "underline" },
  { tag: tags.monospace, fontFamily: EDITOR_MONO, color: "hsl(var(--foreground))", background: "hsl(var(--muted) / 0.8)", borderRadius: "3px" },
  { tag: tags.labelName, fontFamily: EDITOR_MONO, color: "hsl(var(--muted-foreground))" },
  { tag: tags.quote, color: "hsl(var(--muted-foreground))", fontStyle: "italic" },
  // 所有 Markdown 标记（#、**、-、>、```）统一弱化为灰色，正文色留给内容
  { tag: tags.processingInstruction, color: "hsl(var(--muted-foreground) / 0.65)" },
  { tag: tags.contentSeparator, color: "hsl(var(--muted-foreground))" },
  { tag: tags.escape, color: "hsl(var(--primary))" },
  { tag: tags.character, color: "hsl(var(--primary))" },
]);

interface MarkdownEditorProps {
  value: string;
  onChange: (value: string) => void;
  readOnly?: boolean;
  className?: string;
  onViewReady?: (view: EditorView) => void;
  /** i18n 化的空内容占位文案。 */
  placeholder?: string;
}

/** 大文档上 `doc.toString()` 是 O(n)，每个键击全量序列化会拖慢输入；
 * 150ms 节流（尾沿触发）对自动保存无感，Ctrl+S 走 view 实时读取不受影响。 */
const ONCHANGE_THROTTLE_MS = 150;

export function MarkdownEditor({
  value,
  onChange,
  readOnly = false,
  className,
  onViewReady,
  placeholder: placeholderText,
}: MarkdownEditorProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  useEffect(() => {
    if (!containerRef.current) return;

    let lastEmit = 0;
    let trailingTimer: ReturnType<typeof setTimeout> | null = null;
    let queuedDoc: string | null = null;
    const emitNow = (doc: string) => {
      lastEmit = Date.now();
      queuedDoc = null;
      onChangeRef.current(doc);
    };
    const updateListener = EditorView.updateListener.of((update) => {
      if (update.docChanged && !update.transactions.some((t) => t.annotation(ExternalChange))) {
        const doc = update.state.doc.toString();
        const since = Date.now() - lastEmit;
        if (since >= ONCHANGE_THROTTLE_MS) {
          emitNow(doc);
        } else {
          queuedDoc = doc;
          if (!trailingTimer) {
            trailingTimer = setTimeout(() => {
              trailingTimer = null;
              if (queuedDoc !== null) emitNow(queuedDoc);
            }, ONCHANGE_THROTTLE_MS - since);
          }
        }
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
        markdownKeymap,
        keymap.of([
          ...defaultKeymap,
          ...searchKeymap,
          ...historyKeymap,
          ...foldKeymap,
          ...closeBracketsKeymap,
          ...completionKeymap,
        ]),
        markdown({ base: markdownLanguage }),
        EditorView.lineWrapping,
        editorTheme,
        syntaxHighlighting(markdownHighlight),
        updateListener,
        readOnlyCompartment.of(EditorState.readOnly.of(readOnly)),
        placeholder(placeholderText ?? "Markdown content..."),
      ],
    });

    const view = new EditorView({
      state,
      parent: containerRef.current,
    });

    viewRef.current = view;
    onViewReady?.(view);

    return () => {
      // 卸载瞬间把节流中未发出的最后一次编辑补发出去，避免丢字。
      if (trailingTimer) {
        clearTimeout(trailingTimer);
        trailingTimer = null;
      }
      if (queuedDoc !== null) {
        onChangeRef.current(queuedDoc);
        queuedDoc = null;
      }
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