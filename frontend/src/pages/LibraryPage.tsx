import { useCallback, useEffect, useState } from "react";
import {
  BookOpenText,
  ChevronDown,
  ChevronRight,
  Clock,
  Edit,
  Eye,
  FileText,
  Folder,
  FolderOpen,
  LibraryBig,
  Plus,
  RefreshCw,
  Save,
  Search,
  Star,
  Trash2,
  X,
} from "lucide-react";
import { useI18n } from "../i18n";
import {
  addWorkspace,
  getActiveWorkspace,
  listDocuments,
  listFavorites,
  listRecent,
  listSubfolders,
  listWorkspaces,
  readTextFile,
  recordDocumentOpen,
  removeWorkspace,
  scanWorkspace,
  searchDocuments,
  setActiveWorkspace,
  setDocumentFavorite,
} from "../api/tauriApi";
import { pickDir } from "../api/dialogs";
import { MarkdownPreview } from "../components/MarkdownPreview";
import { MarkdownEditor } from "../components/MarkdownEditor";
import { useAutoSave } from "../hooks/useAutoSave";
import { writeTextFile } from "../api/tauriApi";
import { Badge } from "../components/ui/badge";
import { Button } from "../components/ui/button";
import { Input } from "../components/ui/input";
import { ScrollArea } from "../components/ui/scroll-area";
import { cn } from "../lib/utils";
import { showToast } from "../lib/toast";
import type {
  LibraryDocument,
  LibraryFolder,
  ScanResult,
  SearchHit,
  WorkspaceInfo,
} from "../types";

type ViewMode = "browse" | "favorites" | "recent";

interface TreeNode extends LibraryFolder {
  expanded: boolean;
  loaded: boolean;
  children: TreeNode[];
}

function toNode(folder: LibraryFolder): TreeNode {
  return { ...folder, expanded: false, loaded: true, children: [] };
}

function mapTree(
  nodes: TreeNode[],
  target: string,
  fn: (node: TreeNode) => TreeNode
): TreeNode[] {
  return nodes.map((node) => {
    if (node.path === target) return fn(node);
    return { ...node, children: mapTree(node.children, target, fn) };
  });
}

function joinPath(root: string, rel: string): string {
  const rootNorm = root.replace(/[\\/]+$/, "");
  const relNorm = rel.replace(/^[\\/]+/, "");
  return `${rootNorm}/${relNorm}`;
}

function formatTime(iso: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "—";
  return d.toLocaleString();
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function LibraryPage() {
  const { t } = useI18n();

  const [workspaces, setWorkspaces] = useState<WorkspaceInfo[]>([]);
  const [activeWs, setActiveWs] = useState<WorkspaceInfo | null>(null);
  const [scanning, setScanning] = useState(false);
  const [scanResult, setScanResult] = useState<ScanResult | null>(null);
  const [busy, setBusy] = useState(false);

  // Folder tree
  const [tree, setTree] = useState<TreeNode[]>([]);
  const [currentFolder, setCurrentFolder] = useState("");

  // Documents
  const [viewMode, setViewMode] = useState<ViewMode>("browse");
  const [documents, setDocuments] = useState<LibraryDocument[]>([]);
  const [selectedDoc, setSelectedDoc] = useState<LibraryDocument | null>(null);

  // Preview / Edit
  const [previewContent, setPreviewContent] = useState("");
  const [libraryViewMode, setLibraryViewMode] = useState<"preview" | "edit">("preview");

  // Search
  const [query, setQuery] = useState("");
  const [searching, setSearching] = useState(false);
  const [searchHits, setSearchHits] = useState<SearchHit[] | null>(null);

  const loadFolders = useCallback(async (wsId: number) => {
    const roots = await listSubfolders(wsId);
    setTree(roots.map(toNode));
  }, []);

  const loadDocsFor = useCallback(async (wsId: number, folder: string) => {
    const docs = await listDocuments(wsId, folder || undefined);
    setDocuments(docs);
  }, []);

  // Bootstrap: load workspace list, restore the active workspace
  useEffect(() => {
    (async () => {
      try {
        const list = await listWorkspaces();
        setWorkspaces(list);
        const active = await getActiveWorkspace();
        setActiveWs(active);
      } catch (e) {
        showToast(String(e));
      }
    })();
  }, []);

  // When the active workspace changes: incremental index + load tree + documents
  useEffect(() => {
    if (!activeWs) {
      setTree([]);
      setDocuments([]);
      setCurrentFolder("");
      setSelectedDoc(null);
      setPreviewContent("");
      return;
    }
    setScanning(true);
    setCurrentFolder("");
    setSelectedDoc(null);
    setPreviewContent("");
    setDocuments([]);
    setTree([]);
    setSearchHits(null);
    (async () => {
      try {
        const result = await scanWorkspace(activeWs.id);
        setScanResult(result);
        await loadFolders(activeWs.id);
        await loadDocsFor(activeWs.id, "");
      } catch (e) {
        showToast(String(e));
      } finally {
        setScanning(false);
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeWs?.id]);

  async function handleSelectWorkspace(id: number) {
    if (!id || id === activeWs?.id) return;
    const ws = workspaces.find((w) => w.id === id);
    if (!ws) return;
    try {
      await setActiveWorkspace(id);
      setActiveWs(ws);
    } catch (e) {
      showToast(String(e));
    }
  }

  async function handleNewWorkspace() {
    const dir = await pickDir();
    if (!dir) return;
    setBusy(true);
    try {
      const name =
        dir.split(/[\\/]/).filter(Boolean).pop() || "Workspace";
      const ws = await addWorkspace(name, dir);
      await setActiveWorkspace(ws.id);
      setWorkspaces((prev) => [...prev, ws]);
      setQuery("");
      setSearchHits(null);
      setViewMode("browse");
      setActiveWs(ws);
    } catch (e) {
      showToast(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function handleDeleteWorkspace() {
    if (!activeWs) return;
    if (!window.confirm(t("library.deleteWorkspaceConfirm"))) return;
    try {
      await removeWorkspace(activeWs.id);
      setWorkspaces((prev) => prev.filter((w) => w.id !== activeWs.id));
      setActiveWs(null);
    } catch (e) {
      showToast(String(e));
    }
  }

  async function handleReindex() {
    if (!activeWs) return;
    setScanning(true);
    try {
      const result = await scanWorkspace(activeWs.id);
      setScanResult(result);
      await loadFolders(activeWs.id);
      await loadDocsFor(activeWs.id, currentFolder);
    } catch (e) {
      showToast(String(e));
    } finally {
      setScanning(false);
    }
  }

  async function enterFolder(path: string) {
    if (!activeWs) return;
    setCurrentFolder(path);
    setSelectedDoc(null);
    setPreviewContent("");
    setDocuments([]);
    setViewMode("browse");
    setSearchHits(null);
    setTree((prev) =>
      path
        ? mapTree(prev, path, (node) => ({ ...node, expanded: true }))
        : prev
    );
    try {
      await loadDocsFor(activeWs.id, path);
    } catch (e) {
      showToast(String(e));
    }
  }

  async function toggleNode(node: TreeNode) {
    if (!activeWs) return;
    if (!node.expanded && !node.loaded) {
      try {
        const kids = await listSubfolders(activeWs.id, node.path || undefined);
        setTree((prev) =>
          mapTree(prev, node.path, (n) => ({
            ...n,
            loaded: true,
            children: kids.map(toNode),
          }))
        );
      } catch (e) {
        showToast(String(e));
        return;
      }
    }
    setTree((prev) =>
      mapTree(prev, node.path, (n) => ({ ...n, expanded: !n.expanded }))
    );
  }

  async function switchViewMode(mode: ViewMode) {
    if (!activeWs) return;
    setViewMode(mode);
    setSearchHits(null);
    try {
      if (mode === "browse") {
        await loadDocsFor(activeWs.id, currentFolder);
      } else if (mode === "favorites") {
        setDocuments(await listFavorites(activeWs.id));
      } else {
        setDocuments(await listRecent(activeWs.id));
      }
    } catch (e) {
      showToast(String(e));
    }
  }

  async function openDocument(doc: LibraryDocument) {
    if (!activeWs) return;
    setSelectedDoc(doc);
    try {
      const content = await readTextFile(joinPath(activeWs.path, doc.path));
      setPreviewContent(content);
      recordDocumentOpen(doc.id).catch(() => {});
      setDocuments((docs) =>
        docs.map((d) =>
          d.id === doc.id
            ? { ...d, openedAt: new Date().toISOString() }
            : d
        )
      );
    } catch (e) {
      showToast(String(e));
    }
  }

  async function openHit(hit: SearchHit) {
    await openDocument(hit.document);
  }

  async function toggleFavorite(doc: LibraryDocument) {
    const next = !doc.favorite;
    try {
      await setDocumentFavorite(doc.id, next);
      setDocuments((docs) =>
        docs.map((d) => (d.id === doc.id ? { ...d, favorite: next } : d))
      );
      setSelectedDoc((sel) =>
        sel && sel.id === doc.id ? { ...sel, favorite: next } : sel
      );
    } catch (e) {
      showToast(String(e));
    }
  }

  async function handleSearch(e?: React.FormEvent) {
    e?.preventDefault();
    const q = query.trim();
    if (!activeWs) return;
    if (!q) {
      setSearchHits(null);
      return;
    }
    setSearching(true);
    try {
      setSearchHits(await searchDocuments(q, activeWs.id));
    } catch (err) {
      showToast(String(err));
    } finally {
      setSearching(false);
    }
  }

  function clearSearch() {
    setQuery("");
    setSearchHits(null);
  }

  const editorFilePath = selectedDoc && activeWs ? joinPath(activeWs.path, selectedDoc.path) : null;
  const { saving: librarySaving, saveNow: librarySaveNow } = useAutoSave(previewContent, libraryViewMode === "edit" ? editorFilePath : null);

  const tabs: { id: ViewMode; label: string }[] = [
    { id: "browse", label: t("library.allDocs") },
    { id: "favorites", label: t("library.favorites") },
    { id: "recent", label: t("library.recent") },
  ];

  function renderTree(nodes: TreeNode[], depth = 0) {
    return nodes.map((node) => (
      <div key={node.path}>
        <div
          className={cn(
            "flex items-center gap-1 rounded-md pr-2 py-1 text-sm cursor-pointer hover:bg-accent",
            currentFolder === node.path &&
              viewMode === "browse" &&
              "bg-accent text-accent-foreground"
          )}
          style={{ paddingLeft: 8 + depth * 14 }}
          onClick={() => enterFolder(node.path)}
        >
          <span
            className="shrink-0 w-4 h-4 flex items-center justify-center text-muted-foreground hover:text-foreground"
            onClick={(e) => {
              e.stopPropagation();
              toggleNode(node);
            }}
          >
            {node.children.length > 0 || !node.loaded ? (
              node.expanded ? (
                <ChevronDown size={13} />
              ) : (
                <ChevronRight size={13} />
              )
            ) : (
              <span className="w-3" />
            )}
          </span>
          <Folder
            size={14}
            className={cn(
              "shrink-0",
              node.expanded ? "text-primary" : "text-muted-foreground"
            )}
          />
          <span className="truncate flex-1">{node.name}</span>
          <span className="text-xs text-muted-foreground shrink-0">
            {node.docCount}
          </span>
        </div>
        {node.expanded && renderTree(node.children, depth + 1)}
      </div>
    ));
  }

  function renderDocList() {
    if (documents.length === 0) {
      return (
        <div className="flex flex-col items-center justify-center h-40 text-muted-foreground text-sm px-4 text-center">
          <FileText size={28} className="mb-2 opacity-50" />
          <p>{t("library.noDocuments")}</p>
        </div>
      );
    }
    return (
      <div className="p-1.5 flex flex-col gap-0.5">
        {documents.map((doc) => (
          <button
            key={doc.id}
            className={cn(
              "group w-full text-left rounded-md px-2.5 py-2 hover:bg-accent",
              selectedDoc?.id === doc.id && "bg-accent"
            )}
            onClick={() => openDocument(doc)}
          >
            <div className="flex items-center gap-1.5">
              <FileText size={14} className="shrink-0 text-muted-foreground" />
              <span className="text-sm truncate flex-1">{doc.title}</span>
              <Star
                size={14}
                className={cn(
                  "shrink-0 cursor-pointer",
                  doc.favorite
                    ? "text-amber-500 fill-amber-500"
                    : "text-muted-foreground opacity-0 group-hover:opacity-100"
                )}
                onClick={(e) => {
                  e.stopPropagation();
                  toggleFavorite(doc);
                }}
              />
            </div>
            <div className="mt-0.5 pl-5 text-xs text-muted-foreground truncate">
              {doc.path}
            </div>
          </button>
        ))}
      </div>
    );
  }

  function renderSearchResults() {
    if (!searchHits) return null;
    if (searchHits.length === 0) {
      return (
        <div className="flex flex-col items-center justify-center h-40 text-muted-foreground text-sm px-4 text-center">
          <Search size={28} className="mb-2 opacity-50" />
          <p>{t("library.searchEmpty")}</p>
        </div>
      );
    }
    return (
      <div className="p-1.5 flex flex-col gap-0.5">
        {searchHits.map((hit) => (
          <button
            key={hit.document.id}
            className={cn(
              "w-full text-left rounded-md px-2.5 py-2 hover:bg-accent",
              selectedDoc?.id === hit.document.id && "bg-accent"
            )}
            onClick={() => openHit(hit)}
          >
            <div className="flex items-center gap-1.5">
              <FileText size={14} className="shrink-0 text-muted-foreground" />
              <span className="text-sm truncate flex-1">
                {hit.document.title}
              </span>
              {hit.document.favorite && (
                <Star size={13} className="shrink-0 text-amber-500 fill-amber-500" />
              )}
            </div>
            <div className="mt-0.5 pl-5">
              {hit.snippet ? (
                <div
                  className="text-xs text-muted-foreground line-clamp-3 [&_mark]:bg-yellow-300/60 [&_mark]:text-foreground [&_mark]:rounded-sm [&_mark]:px-0.5"
                  dangerouslySetInnerHTML={{ __html: hit.snippet }}
                />
              ) : (
                <div className="text-xs text-muted-foreground truncate">
                  {hit.document.path}
                </div>
              )}
            </div>
          </button>
        ))}
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      {/* Toolbar */}
      <div className="h-12 shrink-0 border-b border-border flex items-center gap-2 px-4">
        <LibraryBig size={18} className="text-primary shrink-0" />
        <span className="font-semibold text-sm shrink-0">
          {t("library.title")}
        </span>

        <select
          value={activeWs?.id ?? ""}
          onChange={(e) => handleSelectWorkspace(Number(e.target.value))}
          className="h-8 max-w-48 shrink-0 rounded-md border border-input bg-background px-2 text-sm"
          disabled={workspaces.length === 0}
        >
          <option value="" disabled>
            {t("library.selectWorkspace")}
          </option>
          {workspaces.map((ws) => (
            <option key={ws.id} value={ws.id}>
              {ws.name}
            </option>
          ))}
        </select>

        <Button
          variant="outline"
          size="sm"
          onClick={handleNewWorkspace}
          disabled={busy}
          title={t("library.newWorkspace")}
        >
          <Plus size={14} />
          {t("library.newWorkspace")}
        </Button>
        <Button
          variant="outline"
          size="sm"
          onClick={handleReindex}
          disabled={!activeWs || scanning}
          title={t("library.reindex")}
        >
          <RefreshCw size={14} className={scanning ? "animate-spin" : ""} />
          {t("library.reindex")}
        </Button>
        <Button
          variant="ghost"
          size="icon"
          onClick={handleDeleteWorkspace}
          disabled={!activeWs}
          title={t("library.deleteWorkspace")}
        >
          <Trash2 size={15} className="text-destructive" />
        </Button>

        {scanResult && !scanning && (
          <span className="hidden lg:inline text-xs text-muted-foreground shrink-0">
            +{scanResult.indexed} ~{scanResult.updated} -{scanResult.removed}{" "}
            · {scanResult.total}
          </span>
        )}

        <form
          onSubmit={handleSearch}
          className="ml-auto flex items-center gap-1.5 min-w-0"
        >
          <div className="relative">
            <Search
              size={14}
              className="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted-foreground pointer-events-none"
            />
            <Input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={t("library.searchPlaceholder")}
              className="w-56 pl-8 pr-7"
              disabled={!activeWs}
            />
            {query && (
              <button
                type="button"
                onClick={clearSearch}
                className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                aria-label="Clear search"
              >
                <X size={14} />
              </button>
            )}
          </div>
          <Button type="submit" variant="secondary" size="sm" disabled={!activeWs || searching}>
            {searching ? t("library.searching") : t("library.search")}
          </Button>
        </form>
      </div>

      {/* Three columns */}
      <div className="flex flex-1 overflow-hidden">
        {/* Left: folder tree */}
        <aside className="w-60 shrink-0 border-r border-border flex flex-col">
          <div className="px-3 py-2 text-xs font-medium text-muted-foreground uppercase tracking-wide shrink-0">
            {t("library.folders")}
          </div>
          {!activeWs ? (
            <div className="flex-1 flex flex-col items-center justify-center text-center text-muted-foreground px-4 gap-2">
              <FolderOpen size={32} className="opacity-40" />
              <p className="text-sm">{t("library.noWorkspace")}</p>
              <p className="text-xs opacity-80">{t("library.noWorkspaceHint")}</p>
            </div>
          ) : (
            <ScrollArea className="flex-1">
              <div className="px-2 pb-2">
                <button
                  onClick={() => enterFolder("")}
                  className={cn(
                    "w-full flex items-center gap-2 rounded-md px-2 py-1.5 text-sm hover:bg-accent",
                    currentFolder === "" &&
                      viewMode === "browse" &&
                      "bg-accent text-accent-foreground"
                  )}
                >
                  <FolderOpen size={15} className="text-primary shrink-0" />
                  <span className="truncate">{t("library.root")}</span>
                  <span className="ml-auto text-xs text-muted-foreground">
                    {scanResult?.total ?? ""}
                  </span>
                </button>
                {renderTree(tree)}
              </div>
            </ScrollArea>
          )}
        </aside>

        {/* Middle: documents / search results */}
        <div className="w-80 shrink-0 border-r border-border flex flex-col">
          <div className="flex items-center gap-1 px-2 pt-2 pb-1.5 shrink-0 border-b">
            {tabs.map((tab) => (
              <button
                key={tab.id}
                onClick={() => switchViewMode(tab.id)}
                className={cn(
                  "flex-1 rounded-md px-2 py-1.5 text-xs font-medium",
                  viewMode === tab.id && !searchHits
                    ? "bg-accent text-accent-foreground"
                    : "text-muted-foreground hover:bg-muted"
                )}
              >
                {tab.label}
              </button>
            ))}
          </div>
          <ScrollArea className="flex-1">
            {searchHits ? renderSearchResults() : renderDocList()}
          </ScrollArea>
        </div>

        {/* Right: preview */}
        <section className="flex-1 flex flex-col overflow-hidden">
          {selectedDoc && previewContent !== null ? (
            <>
              <div className="px-4 py-2.5 border-b shrink-0">
                <div className="flex items-center gap-2 min-w-0">
                  <h2 className="text-sm font-semibold truncate">
                    {selectedDoc.title}
                  </h2>
                  <Star
                    size={15}
                    className={cn(
                      "shrink-0 cursor-pointer",
                      selectedDoc.favorite
                        ? "text-amber-500 fill-amber-500"
                        : "text-muted-foreground hover:text-amber-500"
                    )}
                    onClick={() => toggleFavorite(selectedDoc)}
                  />
                  <button
                    onClick={() => setLibraryViewMode((m) => (m === "preview" ? "edit" : "preview"))}
                    className="h-7 w-7 rounded-md flex items-center justify-center text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
                    title={libraryViewMode === "preview" ? t("editor.editMode") : t("editor.previewMode")}
                  >
                    {libraryViewMode === "preview" ? <Edit size={14} /> : <Eye size={14} />}
                  </button>
                  {libraryViewMode === "edit" && librarySaving && (
                    <span className="text-xs text-muted-foreground ml-auto">{t("editor.saving")}</span>
                  )}
                </div>
                <div className="flex items-center gap-2 text-xs text-muted-foreground mt-1 min-w-0">
                  <span className="truncate">{selectedDoc.path}</span>
                  {selectedDoc.source && (
                    <Badge variant="secondary" className="shrink-0">
                      {selectedDoc.source}
                    </Badge>
                  )}
                  <span className="shrink-0 flex items-center gap-1 ml-auto">
                    {formatSize(selectedDoc.fileSize)}
                    <span className="flex items-center gap-0.5">
                      <Clock size={11} />
                      {formatTime(selectedDoc.openedAt)}
                    </span>
                  </span>
                </div>
              </div>
              <div className="flex-1 overflow-auto">
                {libraryViewMode === "edit" ? (
                  <MarkdownEditor value={previewContent} onChange={setPreviewContent} />
                ) : (
                  <MarkdownPreview content={previewContent} />
                )}
              </div>
            </>
          ) : (
            <div className="flex-1 flex flex-col items-center justify-center text-muted-foreground gap-2">
              <BookOpenText size={36} className="opacity-40" />
              <p className="text-sm">{t("library.noPreview")}</p>
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
