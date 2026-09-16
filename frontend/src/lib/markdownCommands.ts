import { EditorView, keymap } from "@codemirror/view";
import { EditorSelection, Prec } from "@codemirror/state";

/** Wrap the current selection with `before`/`after`, or insert a placeholder. */
export function wrapSelection(
  view: EditorView,
  before: string,
  after: string,
  placeholderText?: string,
) {
  view.focus();
  view.dispatch(
    view.state.changeByRange((range) => {
      const text = view.state.sliceDoc(range.from, range.to);
      const insert = text
        ? `${before}${text}${after}`
        : `${before}${placeholderText || ""}${after}`;
      return {
        range: EditorSelection.range(range.from, range.from + insert.length),
        changes: { from: range.from, to: range.to, insert },
      };
    }),
  );
}

/** Insert a prefix at the start of the current line. */
export function insertLinePrefix(view: EditorView, prefix: string) {
  view.focus();
  view.dispatch(
    view.state.changeByRange((range) => {
      const line = view.state.doc.lineAt(range.from);
      return {
        range: EditorSelection.range(range.from + prefix.length, range.to + prefix.length),
        changes: { from: line.from, insert: prefix },
      };
    }),
  );
}

export function insertCodeBlock(view: EditorView) {
  view.focus();
  view.dispatch(
    view.state.changeByRange((range) => {
      const text = view.state.sliceDoc(range.from, range.to);
      const insert = text ? `\`\`\`\n${text}\n\`\`\`` : "```\n\n```";
      return {
        range: EditorSelection.range(range.from, range.from + insert.length),
        changes: { from: range.from, to: range.to, insert },
      };
    }),
  );
}

export function insertLink(view: EditorView) {
  view.focus();
  view.dispatch(
    view.state.changeByRange((range) => {
      const text = view.state.sliceDoc(range.from, range.to);
      const insert = text ? `[${text}](url)` : "[text](url)";
      return {
        range: EditorSelection.range(range.from, range.from + insert.length),
        changes: { from: range.from, to: range.to, insert },
      };
    }),
  );
}

/**
 * Keyboard shortcuts matching the toolbar hints. Bound at high precedence so
 * they win over the default keymap.
 */
export const markdownKeymap = Prec.high(
  keymap.of([
    {
      key: "Mod-b",
      run: (v) => {
        wrapSelection(v, "**", "**", "bold");
        return true;
      },
    },
    {
      key: "Mod-i",
      run: (v) => {
        wrapSelection(v, "*", "*", "italic");
        return true;
      },
    },
    {
      key: "Mod-Shift-h",
      run: (v) => {
        insertLinePrefix(v, "## ");
        return true;
      },
    },
    {
      key: "Mod-k",
      run: (v) => {
        insertLink(v);
        return true;
      },
    },
  ]),
);
