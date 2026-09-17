import { useCallback, useDeferredValue, useEffect, useRef, useState } from "react";
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
  Loader2,
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
import { pickDir, confirmDialog } from "../api/dialogs";
import { MarkdownPreview } from "../components/MarkdownPreview";
import { MarkdownEditor } from "../components/MarkdownEditor";
import { useAutoSave } from "../hooks/useAutoSave";
import { writeTextFile } from "../api/tauriApi";
import { Badge } from "../components/ui/badge";
import { Button } from "../components/ui/button";
import { Input } from "../components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../components/ui/select";
import { VirtualList } from "../components/VirtualList";
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
  // `loaded: false` so nested subfolders are fetched lazily when first expanded.
  return { ...folder, expanded: false, loaded: false, children: [] };
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

function findNode(nodes: TreeNode[], target: string): TreeNode | null {
  for (const node of nodes) {
    if (node.path === target) return node;
    const found = findNode(node.children, target);
    if (found) return found;
  }
  return null;
}

function joinPath(root: string, rel: string): string {
  // Document paths are stored ABSOLUTE (normalized by the backend); joining
  // a root onto them would produce "D:/D:/…" which Windows rejects with
  // os error 123. Only prefix genuinely relative fragments.
  if (/^[a-zA-Z]:[\\/]/.test(rel) || rel.startsWith("//") || rel.startsWith("\\\\")) {
    return rel;
  }
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

/// Display a document's path relative to the workspace root when possible —
/// absolute drive paths are long and dominate narrow columns.
function displayPath(abs: string, wsRoot?: string): string {
  if (!wsRoot) return abs;
  const norm = wsRoot.replace(/[\\/]+$/, "");
  return abs.startsWith(norm) ? abs.slice(norm.length + 1) : abs;
}

/// 后端返回的 snippet 已整体 HTML 转义、仅保留 `<mark>` 高亮标记；这里再
/// 做一道前端兜底：任何不属于 `<mark>`/`</mark>` 的 "<" 一律转为实体，
/// 确保 dangerouslySetInnerHTML 永远收不到可执行的标签。
function safeSnippetHtml(snippet: string): string {
  return snippet.replace(/<(?!\/?mark>)/g, "&lt;");
}

const FOLDER_PANEL_WIDTH_KEY = "omnidm_folder_panel_width";
const FOLDER_PANEL_MIN = 160;
const FOLDER_PANEL_MAX = 480;
const FOLDER_PANEL_DEFAULT = 208;

function readStoredFolderPanelWidth(): number {
  const n = Number(localStorage.getItem(FOLDER_PANEL_WIDTH_KEY));
  return Number.isFinite(n) && n >= FOLDER_PANEL_MIN && n <= FOLDER_PANEL_MAX
    ? n
    : FOLDER_PANEL_DEFAULT;
}

/// Workspaces already auto-scanned during this app session. Module-level on
/// purpose: it must survive LibraryPage unmount/remount when switching pages,
/// which is exactly what used to re-trigger a full disk re-index.
const sessionScannedWorkspaces = new Set<number>();

/// Everything worth restoring when the user leaves the Library page and comes
/// back: the opened document, preview content/mode, folder tree (including
/// expansion state), current folder/view, and the last search. Module-level
/// so it survives the page unmount; overwritten on every unmount.
interface LibrarySessionSnapshot {
  workspaceId: number;
  tree: TreeNode[];
  currentFolder: string;
  viewMode: ViewMode;
  documents: LibraryDocument[];
  selectedDoc: LibraryDocument | null;
  previewContent: string;
  previewDocId: number | null;
  libraryViewMode: "preview" | "edit";
  query: string;
  searchHits: SearchHit[] | null;
  scanResult: ScanResult | null;
}
let librarySession: LibrarySessionSnapshot | null = null;

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

  // Folder panel width is user-resizable: deep trees need more room than the
  // 208px default. The choice persists locally across sessions.
  const [folderPanelWidth, setFolderPanelWidth] = useState(readStoredFolderPanelWidth);
  const folderPanelWidthRef = useRef(folderPanelWidth);
  folderPanelWidthRef.current = folderPanelWidth;
  const resizeCleanupRef = useRef<(() => void) | null>(null);

  useEffect(() => () => resizeCleanupRef.current?.(), []);

  const startFolderPanelResize = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    const startX = e.clientX;
    const startWidth = folderPanelWidthRef.current;
    const onMove = (ev: MouseEvent) => {
      const next = Math.min(
        FOLDER_PANEL_MAX,
        Math.max(FOLDER_PANEL_MIN, startWidth + ev.clientX - startX)
      );
      setFolderPanelWidth(next);
    };
    const onUp = () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      document.documentElement.classList.remove("col-resizing");
      resizeCleanupRef.current = null;
      localStorage.setItem(FOLDER_PANEL_WIDTH_KEY, String(folderPanelWidthRef.current));
    };
    document.documentElement.classList.add("col-resizing");
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    resizeCleanupRef.current = onUp;
  }, []);

  const resetFolderPanelWidth = useCallback(() => {
    setFolderPanelWidth(FOLDER_PANEL_DEFAULT);
    localStorage.setItem(FOLDER_PANEL_WIDTH_KEY, String(FOLDER_PANEL_DEFAULT));
  }, []);

  // Documents
  const [viewMode, setViewMode] = useState<ViewMode>("browse");
  const [documents, setDocuments] = useState<LibraryDocument[]>([]);
  const [selectedDoc, setSelectedDoc] = useState<LibraryDocument | null>(null);

  // Preview / Edit
  const [previewContent, setPreviewContent] = useState("");
  const [previewDocId, setPreviewDocId] = useState<number | null>(null);
  const [libraryViewMode, setLibraryViewMode] = useState<"preview" | "edit">("preview");

  // Search
  const [query, setQuery] = useState("");
  const [searching, setSearching] = useState(false);
  const [searchHits, setSearchHits] = useState<SearchHit[] | null>(null);

  // Request tokens: only the newest async request may commit its result.
  const openReqRef = useRef(0);
  const searchReqRef = useRef(0);
  const wsReqRef = useRef(0);
  const docsReqRef = useRef(0);
  // 当前工作区 id 的镜像：reindex / 加载子目录等异步操作完成后据此判断
  // 工作区是否已切换，防止把旧工作区的数据渲染进新视图。
  const wsIdRef = useRef<number | null>(null);

  // 离开知识库页面时保存浏览上下文，下次进入同一工作区时整体恢复。
  const sessionStateRef = useRef<LibrarySessionSnapshot | null>(null);
  sessionStateRef.current = activeWs
    ? {
        workspaceId: activeWs.id,
        tree,
        currentFolder,
        viewMode,
        documents,
        selectedDoc,
        previewContent,
        previewDocId,
        libraryViewMode,
        query,
        searchHits,
        scanResult,
      }
    : null;
  useEffect(
    () => () => {
      if (sessionStateRef.current) librarySession = sessionStateRef.current;
    },
    []
  );

  const loadFolders = useCallback(async (wsId: number) => {
    const roots = await listSubfolders(wsId);
    setTree(roots.map(toNode));
  }, []);

  const loadDocsFor = useCallback(async (wsId: number, folder: string) => {
    // 令牌防竞态：快速连点两个文件夹时，先发后至的响应不得覆盖后者。
    const req = ++docsReqRef.current;
    const docs = await listDocuments(wsId, folder || undefined);
    if (req !== docsReqRef.current) return;
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
    const req = ++wsReqRef.current;
    wsIdRef.current = activeWs?.id ?? null;
    if (!activeWs) {
      setTree([]);
      setDocuments([]);
      setCurrentFolder("");
      setSelectedDoc(null);
      setPreviewContent("");
      setPreviewDocId(null);
      return;
    }

    // 回到上次浏览过的同一工作区：整体恢复会话快照（选中文档、预览、
    // 文件夹树及展开状态、搜索），零请求，也不重复扫描。
    if (librarySession && librarySession.workspaceId === activeWs.id) {
      setTree(librarySession.tree);
      setCurrentFolder(librarySession.currentFolder);
      setViewMode(librarySession.viewMode);
      setDocuments(librarySession.documents);
      setSelectedDoc(librarySession.selectedDoc);
      setPreviewContent(librarySession.previewContent);
      setPreviewDocId(librarySession.previewDocId);
      setLibraryViewMode(librarySession.libraryViewMode);
      setQuery(librarySession.query);
      setSearchHits(librarySession.searchHits);
      setScanResult(librarySession.scanResult);
      setScanning(false);
      sessionScannedWorkspaces.add(activeWs.id);
      librarySession = null;
      return;
    }

    setCurrentFolder("");
    setSelectedDoc(null);
    setPreviewContent("");
    setPreviewDocId(null);
    setDocuments([]);
    setTree([]);
    setSearchHits(null);
    // 索引持久化在 SQLite 里，本会话已经扫过的工作区切页回来时直接读库，
    // 不再自动重扫磁盘（万级文件的全量扫描让每次切页都像卡死）。
    // 「重新索引」按钮始终强制真正重扫。
    const alreadyScanned = sessionScannedWorkspaces.has(activeWs.id);
    if (!alreadyScanned) setScanning(true);
    (async () => {
      try {
        if (alreadyScanned) {
          const docs = await listDocuments(activeWs.id, "");
          if (req !== wsReqRef.current) return;
          setScanResult({ indexed: 0, updated: 0, removed: 0, total: docs.length });
          await loadFolders(activeWs.id);
          if (req !== wsReqRef.current) return;
          await loadDocsFor(activeWs.id, "");
          return;
        }
        const result = await scanWorkspace(activeWs.id);
        if (req !== wsReqRef.current) return;
        sessionScannedWorkspaces.add(activeWs.id);
        setScanResult(result);
        await loadFolders(activeWs.id);
        if (req !== wsReqRef.current) return;
        await loadDocsFor(activeWs.id, "");
      } catch (e) {
        if (req === wsReqRef.current) showToast(String(e), 3000, "error");
      } finally {
        if (req === wsReqRef.current) setScanning(false);
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
      showToast(String(e), 3000, "error");
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
      showToast(String(e), 3000, "error");
    } finally {
      setBusy(false);
    }
  }

  async function handleDeleteWorkspace() {
    if (!activeWs || busy) return;
    if (!(await confirmDialog(t("library.deleteWorkspaceConfirm"), t("library.deleteWorkspace")))) return;
    setBusy(true);
    try {
      await removeWorkspace(activeWs.id);
      setWorkspaces((prev) => prev.filter((w) => w.id !== activeWs.id));
      setActiveWs(null);
    } catch (e) {
      showToast(String(e), 3000, "error");
    } finally {
      setBusy(false);
    }
  }

  async function handleReindex() {
    if (!activeWs) return;
    const wsId = activeWs.id;
    setScanning(true);
    try {
      const result = await scanWorkspace(wsId);
      // 期间切换了工作区：本次结果作废，交给新工作区自己的加载流程。
      if (wsIdRef.current !== wsId) return;
      sessionScannedWorkspaces.add(wsId);
      setScanResult(result);
      await loadFolders(wsId);
      if (wsIdRef.current !== wsId) return;
      await loadDocsFor(wsId, currentFolder);
    } catch (e) {
      showToast(String(e), 3000, "error");
    } finally {
      if (wsIdRef.current === wsId) setScanning(false);
    }
  }

  async function loadChildren(node: TreeNode) {
    if (!activeWs || node.loaded) return;
    const wsId = activeWs.id;
    try {
      const kids = await listSubfolders(wsId, node.path || undefined);
      if (wsIdRef.current !== wsId) return;
      setTree((prev) =>
        mapTree(prev, node.path, (n) => ({
          ...n,
          loaded: true,
          children: kids.map(toNode),
        }))
      );
    } catch (e) {
      showToast(String(e), 3000, "error");
    }
  }

  async function enterFolder(path: string) {
    if (!activeWs) return;
    setCurrentFolder(path);
    setSelectedDoc(null);
    setPreviewContent("");
    setPreviewDocId(null);
    setDocuments([]);
    setViewMode("browse");
    setSearchHits(null);
    if (path) {
      const node = findNode(tree, path);
      if (node) await loadChildren(node);
      setTree((prev) =>
        mapTree(prev, path, (n) => ({ ...n, expanded: true }))
      );
    }
    try {
      await loadDocsFor(activeWs.id, path);
    } catch (e) {
      showToast(String(e), 3000, "error");
    }
  }

  async function toggleNode(node: TreeNode) {
    if (!node.expanded) await loadChildren(node);
    setTree((prev) =>
      mapTree(prev, node.path, (n) => ({ ...n, expanded: !n.expanded }))
    );
  }

  async function switchViewMode(mode: ViewMode) {
    if (!activeWs) return;
    setViewMode(mode);
    setSearchHits(null);
    // 与 loadDocsFor 共用令牌：切换 tab 的三次并发请求只有最新一次能提交。
    const req = ++docsReqRef.current;
    try {
      if (mode === "browse") {
        await loadDocsFor(activeWs.id, currentFolder);
      } else if (mode === "favorites") {
        const docs = await listFavorites(activeWs.id);
        if (req === docsReqRef.current) setDocuments(docs);
      } else {
        const docs = await listRecent(activeWs.id);
        if (req === docsReqRef.current) setDocuments(docs);
      }
    } catch (e) {
      showToast(String(e), 3000, "error");
    }
  }

  async function openDocument(doc: LibraryDocument) {
    if (!activeWs) return;
    const req = ++openReqRef.current;
    setSelectedDoc(doc);
    setPreviewDocId(null);
    try {
      const content = await readTextFile(joinPath(activeWs.path, doc.path));
      // Ignore a stale response (the user opened another document meanwhile).
      if (req !== openReqRef.current) return;
      setPreviewContent(content);
      setPreviewDocId(doc.id);
      recordDocumentOpen(doc.id).catch(() => {});
      setDocuments((docs) =>
        docs.map((d) =>
          d.id === doc.id
            ? { ...d, openedAt: new Date().toISOString() }
            : d
        )
      );
    } catch (e) {
      if (req === openReqRef.current) {
        showToast(String(e), 3000, "error");
        // 关键防护：读取失败时清空编辑器内容。若保留上一个文档的文本，
        // 自动保存会把旧内容（连同用户的新输入）写进刚选中的这个文件。
        setPreviewContent("");
        setPreviewDocId(null);
      }
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
      setSearchHits((hits) =>
        hits
          ? hits.map((h) =>
              h.document.id === doc.id
                ? { ...h, document: { ...h.document, favorite: next } }
                : h
            )
          : hits
      );
    } catch (e) {
      showToast(String(e), 3000, "error");
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
    const req = ++searchReqRef.current;
    setSearching(true);
    try {
      const hits = await searchDocuments(q, activeWs.id);
      if (req === searchReqRef.current) setSearchHits(hits);
    } catch (err) {
      if (req === searchReqRef.current) showToast(String(err), 3000, "error");
    } finally {
      if (req === searchReqRef.current) setSearching(false);
    }
  }

  function clearSearch() {
    setQuery("");
    setSearchHits(null);
  }

  const editorFilePath = selectedDoc && activeWs ? joinPath(activeWs.path, selectedDoc.path) : null;
  const { saving: librarySaving } = useAutoSave(
    previewContent,
    libraryViewMode === "edit" ? editorFilePath : null
  );

  // Markdown parsing is expensive; defer it so clicking a document updates the
  // selection/list immediately and the preview catches up right after paint.
  const deferredPreviewContent = useDeferredValue(previewContent);
  const isRenderingPreview = deferredPreviewContent !== previewContent;

  const tabs: { id: ViewMode; label: string }[] = [
    { id: "browse", label: t("library.allDocs") },
    { id: "favorites", label: t("library.favorites") },
    { id: "recent", label: t("library.recent") },
  ];

  function renderTree(nodes: TreeNode[], depth = 0) {
    return nodes.map((node) => (
      <div key={node.path}>
        <div
          role="button"
          tabIndex={0}
          aria-expanded={node.expanded}
          className={cn(
            "flex items-center gap-1 rounded-md pr-2 py-1 text-sm cursor-pointer hover:bg-accent focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring",
            currentFolder === node.path &&
              viewMode === "browse" &&
              "bg-accent text-accent-foreground"
          )}
          style={{ paddingLeft: 8 + depth * 12 }}
          onClick={() => enterFolder(node.path)}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              enterFolder(node.path);
            }
          }}
        >
          <span
            role="button"
            tabIndex={0}
            aria-label={t("library.folders")}
            className="shrink-0 w-4 h-4 flex items-center justify-center text-muted-foreground hover:text-foreground"
            onClick={(e) => {
              e.stopPropagation();
              toggleNode(node);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                e.stopPropagation();
                toggleNode(node);
              }
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
          <span className="truncate flex-1" title={node.name}>
            {node.name}
          </span>
          <span className="text-xs text-muted-foreground shrink-0">
            {node.docCount}
          </span>
        </div>
        {node.expanded && renderTree(node.children, depth + 1)}
      </div>
    ));
  }

  function renderDocItem(doc: LibraryDocument) {
    return (
      <button
        className={cn(
          "group w-full text-left rounded-md px-2.5 py-2 hover:bg-accent overflow-hidden",
          selectedDoc?.id === doc.id && "bg-accent"
        )}
        onClick={() => openDocument(doc)}
      >
        <div className="flex items-center gap-1.5 min-w-0">
          <FileText size={14} className="shrink-0 text-muted-foreground" />
          <span className="text-sm truncate flex-1 min-w-0" title={doc.title}>
            {doc.title}
          </span>
          <span
            role="button"
            tabIndex={0}
            aria-label={t("library.favorites")}
            aria-pressed={doc.favorite}
            className="shrink-0 flex items-center"
            onClick={(e) => {
              e.stopPropagation();
              toggleFavorite(doc);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                e.stopPropagation();
                toggleFavorite(doc);
              }
            }}
          >
            <Star
              size={14}
              className={cn(
                doc.favorite
                  ? "text-amber-500 fill-amber-500"
                  : "text-muted-foreground opacity-0 group-hover:opacity-100"
              )}
            />
          </span>
        </div>
        <div
          className="mt-0.5 pl-5 text-xs text-muted-foreground truncate"
          title={doc.path}
        >
          {displayPath(doc.path, activeWs?.path)}
        </div>
      </button>
    );
  }

  function renderHitItem(hit: SearchHit) {
    return (
      <button
        className={cn(
          "w-full text-left rounded-md px-2.5 py-2 hover:bg-accent overflow-hidden",
          selectedDoc?.id === hit.document.id && "bg-accent"
        )}
        onClick={() => openHit(hit)}
      >
        <div className="flex items-center gap-1.5 min-w-0">
          <FileText size={14} className="shrink-0 text-muted-foreground" />
          <span className="text-sm truncate flex-1 min-w-0" title={hit.document.title}>
            {hit.document.title}
          </span>
          {hit.document.favorite && (
            <Star size={13} className="shrink-0 text-amber-500 fill-amber-500" />
          )}
        </div>
        <div className="mt-0.5 pl-5 min-w-0">
          {hit.snippet ? (
            <div
              className="text-xs text-muted-foreground line-clamp-3 [&_mark]:bg-yellow-300/60 [&_mark]:text-foreground [&_mark]:rounded-sm [&_mark]:px-0.5 break-words"
              dangerouslySetInnerHTML={{ __html: safeSnippetHtml(hit.snippet) }}
            />
          ) : (
            <div
              className="text-xs text-muted-foreground truncate"
              title={hit.document.path}
            >
              {displayPath(hit.document.path, activeWs?.path)}
            </div>
          )}
        </div>
      </button>
    );
  }

  return (
    <div className="h-full flex flex-col">
      {/* Toolbar */}
      <div className="h-12 shrink-0 border-b border-border flex items-center gap-2 px-4">
        <Select
          value={activeWs ? String(activeWs.id) : undefined}
          onValueChange={(v) => handleSelectWorkspace(Number(v))}
          disabled={workspaces.length === 0}
        >
          <SelectTrigger className="w-44 shrink-0">
            <SelectValue placeholder={t("library.selectWorkspace")} />
          </SelectTrigger>
          <SelectContent>
            {workspaces.map((ws) => (
              <SelectItem key={ws.id} value={String(ws.id)}>
                {ws.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>

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
                aria-label={t("library.clearSearch")}
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
        {/* Left: folder tree (drag the right edge to resize) */}
        <aside
          className="relative shrink-0 border-r border-border flex flex-col"
          style={{ width: folderPanelWidth }}
        >
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
            <div className="flex-1 overflow-y-auto">
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
            </div>
          )}
          <div
            role="separator"
            aria-orientation="vertical"
            aria-label={t("library.resizeFoldersHint")}
            title={t("library.resizeFoldersHint")}
            onMouseDown={startFolderPanelResize}
            onDoubleClick={resetFolderPanelWidth}
            className="absolute inset-y-0 right-0 z-10 w-1 cursor-col-resize transition-colors hover:bg-primary/50 active:bg-primary/70"
          />
        </aside>

        {/* Middle: documents / search results */}
        <div className="w-80 shrink-0 border-r border-border flex flex-col">
          <div className="flex items-center gap-1 px-2 pt-2 pb-1.5 shrink-0 border-b" role="tablist">
            {tabs.map((tab) => (
              <button
                key={tab.id}
                role="tab"
                aria-selected={viewMode === tab.id}
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
          {searchHits ? (
            searchHits.length === 0 ? (
              <div className="flex-1 flex flex-col items-center justify-center text-muted-foreground text-sm px-4 text-center">
                <Search size={28} className="mb-2 opacity-50" />
                <p>{t("library.searchEmpty")}</p>
              </div>
            ) : (
              <VirtualList
                items={searchHits}
                className="flex-1 min-h-0 p-1.5"
                estimateSize={72}
                gap={2}
                itemKey={(hit) => hit.document.id}
                renderItem={(hit) => renderHitItem(hit)}
              />
            )
          ) : documents.length === 0 ? (
            <div className="flex-1 flex flex-col items-center justify-center text-muted-foreground text-sm px-4 text-center">
              <FileText size={28} className="mb-2 opacity-50" />
              <p>{t("library.noDocuments")}</p>
            </div>
          ) : (
            <VirtualList
              items={documents}
              className="flex-1 min-h-0 p-1.5"
              estimateSize={58}
              gap={2}
              itemKey={(doc) => doc.id}
              renderItem={(doc) => renderDocItem(doc)}
            />
          )}
        </div>

        {/* Right: preview */}
        <section className="flex-1 flex flex-col overflow-hidden relative">
          {selectedDoc ? (
            <>
              <div className="px-4 py-2.5 border-b shrink-0">
                <div className="flex items-center gap-2 min-w-0">
                  <h2 className="text-sm font-semibold truncate">
                    {selectedDoc.title}
                  </h2>
                  <button
                    type="button"
                    onClick={() => toggleFavorite(selectedDoc)}
                    aria-pressed={selectedDoc.favorite}
                    title={t("library.favorites")}
                    className="shrink-0 flex items-center"
                  >
                    <Star
                      size={15}
                      className={cn(
                        selectedDoc.favorite
                          ? "text-amber-500 fill-amber-500"
                          : "text-muted-foreground hover:text-amber-500"
                      )}
                    />
                  </button>
                  {/* 预览/编辑 药丸切换 */}
                  <div className="inline-flex items-center rounded-lg bg-muted p-0.5 shrink-0">
                    {(
                      [
                        { mode: "preview", icon: Eye, label: t("editor.previewMode") },
                        { mode: "edit", icon: Edit, label: t("editor.editMode") },
                      ] as const
                    ).map(({ mode, icon: Icon, label }) => (
                      <button
                        key={mode}
                        type="button"
                        onClick={() => setLibraryViewMode(mode)}
                        aria-pressed={libraryViewMode === mode}
                        className={cn(
                          "flex items-center gap-1 h-6 px-2 rounded-md text-xs font-medium transition-colors",
                          libraryViewMode === mode
                            ? "bg-card text-foreground dark:bg-background"
                            : "text-muted-foreground hover:text-foreground"
                        )}
                      >
                        <Icon size={12} />
                        {label}
                      </button>
                    ))}
                  </div>
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
              <div className="flex-1 min-h-0 relative">
                <div className="absolute inset-0 overflow-auto">
                  {/* Keep the editor mounted so switching to preview and back
                      preserves undo history and cursor position. */}
                  <div className={cn("h-full", libraryViewMode === "edit" ? "block" : "hidden")}>
                    <MarkdownEditor value={previewContent} onChange={setPreviewContent} />
                  </div>
                  {libraryViewMode === "preview" &&
                    (previewDocId === selectedDoc.id ? (
                      <MarkdownPreview content={deferredPreviewContent} />
                    ) : (
                      <div className="flex items-center justify-center h-full text-muted-foreground text-sm">
                        <Loader2 size={14} className="animate-spin mr-2 text-primary" />
                        {t("library.rendering")}
                      </div>
                    ))}
                </div>
                {isRenderingPreview &&
                  libraryViewMode === "preview" &&
                  previewDocId === selectedDoc.id && (
                    <div className="absolute inset-0 flex items-start justify-center pt-4 pointer-events-none">
                      <div className="flex items-center gap-2 text-xs text-muted-foreground bg-background/95 border border-border rounded-full px-3 py-1.5">
                        <Loader2 size={12} className="animate-spin text-primary" />
                        {t("library.rendering")}
                      </div>
                    </div>
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
