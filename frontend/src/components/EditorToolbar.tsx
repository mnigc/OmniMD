import { Bold, Italic, Heading, List, ListOrdered, Link, Code, Quote, Undo2, Redo2 } from "lucide-react";
import { EditorView } from "@codemirror/view";
import { EditorSelection } from "@codemirror/state";
import { undo, redo } from "@codemirror/commands";
import { useI18n } from "../i18n";
import { Tooltip, TooltipContent, TooltipTrigger } from "./ui/tooltip";
import { Separator } from "./ui/separator";

interface EditorToolbarProps {
  view: EditorView | null;
}

function wrapSelection(view: EditorView, before: string, after: string, placeholderText?: string) {
  view.focus();
  view.dispatch(
    view.state.changeByRange((range) => {
      const text = view.state.sliceDoc(range.from, range.to);
      const insert = text ? `${before}${text}${after}` : `${before}${placeholderText || ""}${after}`;
      return {
        range: EditorSelection.range(range.from, range.from + insert.length),
        changes: { from: range.from, to: range.to, insert },
      };
    }),
  );
}

function insertLinePrefix(view: EditorView, prefix: string) {
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

function insertCodeBlock(view: EditorView) {
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

function insertLink(view: EditorView) {
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

type ToolbarButton = {
  icon: React.ReactNode;
  label: string;
  action: (view: EditorView) => void;
  shortcut?: string;
};

export function EditorToolbar({ view }: EditorToolbarProps) {
  const { t } = useI18n();

  const buttons: ToolbarButton[] = [
    { icon: <Bold size={14} />, label: t("editor.bold"), action: (v) => wrapSelection(v, "**", "**", "bold"), shortcut: "Ctrl+B" },
    { icon: <Italic size={14} />, label: t("editor.italic"), action: (v) => wrapSelection(v, "*", "*", "italic"), shortcut: "Ctrl+I" },
    { icon: <Heading size={14} />, label: t("editor.heading"), action: (v) => insertLinePrefix(v, "## "), shortcut: "Ctrl+Shift+H" },
    { icon: <List size={14} />, label: t("editor.bulletList"), action: (v) => insertLinePrefix(v, "- ") },
    { icon: <ListOrdered size={14} />, label: t("editor.orderedList"), action: (v) => insertLinePrefix(v, "1. ") },
    { icon: <Link size={14} />, label: t("editor.link"), action: (v) => insertLink(v), shortcut: "Ctrl+K" },
    { icon: <Code size={14} />, label: t("editor.codeBlock"), action: (v) => insertCodeBlock(v) },
    { icon: <Quote size={14} />, label: t("editor.blockquote"), action: (v) => insertLinePrefix(v, "> ") },
  ];

  const utils: ToolbarButton[] = [
    { icon: <Undo2 size={14} />, label: t("editor.undo"), action: (v) => { v.focus(); undo(v); }, shortcut: "Ctrl+Z" },
    { icon: <Redo2 size={14} />, label: t("editor.redo"), action: (v) => { v.focus(); redo(v); }, shortcut: "Ctrl+Y" },
  ];

  const renderButton = (btn: ToolbarButton) => (
    <Tooltip key={btn.label}>
      <TooltipTrigger asChild>
        <button
          disabled={!view}
          onClick={() => view && btn.action(view)}
          className="h-8 w-8 rounded-md flex items-center justify-center text-muted-foreground hover:text-foreground hover:bg-accent disabled:opacity-30 disabled:pointer-events-none transition-colors"
        >
          {btn.icon}
        </button>
      </TooltipTrigger>
      <TooltipContent side="bottom" className="text-xs">
        {btn.label}{btn.shortcut ? ` (${btn.shortcut})` : ""}
      </TooltipContent>
    </Tooltip>
  );

  return (
    <div className="h-9 border-b border-border px-2 flex items-center gap-0.5 bg-muted/20 shrink-0 overflow-x-auto">
      <div className="flex items-center gap-0.5">{buttons.map(renderButton)}</div>
      <Separator orientation="vertical" className="h-5 mx-1" />
      <div className="flex items-center gap-0.5">{utils.map(renderButton)}</div>
    </div>
  );
}