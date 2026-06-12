import { useCallback, useEffect, useRef, useState } from "react";
import {
  IconArrowUp,
  IconChevronRight,
  IconDownload,
  IconEdit,
  IconFile,
  IconFolder,
  IconLink,
  IconPlus,
  IconRefresh,
  IconTrash,
  IconUpload,
} from "./Icon";
import { ConfirmDialog, EmptyState, TextPrompt } from "./ui";
import * as api from "@/services/api";
import { formatBytes, formatDate, joinLocal, joinPath } from "@/lib/format";
import { errorMessage, toast } from "@/stores/toast";
import { useTransfers } from "@/stores/transfers";
import type { DirEntry } from "@/types";

type Side = "local" | "remote";

interface DragPayload {
  side: Side;
  path: string;
  name: string;
  isDir: boolean;
}

function remoteParent(path: string): string {
  if (path === "/" || path === "") return "/";
  const trimmed = path.replace(/\/+$/, "");
  const idx = trimmed.lastIndexOf("/");
  return idx <= 0 ? "/" : trimmed.slice(0, idx);
}

/** Split a path into clickable breadcrumb segments (handles Windows + POSIX). */
function pathCrumbs(path: string): { label: string; path: string }[] {
  if (!path) return [];
  if (/^[A-Za-z]:[\\/]/.test(path)) {
    const parts = path.replace(/\//g, "\\").split("\\").filter(Boolean);
    const crumbs: { label: string; path: string }[] = [];
    let acc = "";
    parts.forEach((p, i) => {
      acc = i === 0 ? `${p}\\` : `${acc.replace(/\\$/, "")}\\${p}`;
      crumbs.push({ label: p, path: acc });
    });
    return crumbs;
  }
  const parts = path.split("/").filter(Boolean);
  const crumbs = [{ label: "/", path: "/" }];
  let acc = "";
  parts.forEach((p) => {
    acc += `/${p}`;
    crumbs.push({ label: p, path: acc });
  });
  return crumbs;
}

function Breadcrumbs({
  path,
  onNavigate,
}: {
  path: string;
  onNavigate: (p: string) => void;
}) {
  const crumbs = pathCrumbs(path);
  return (
    <div className="crumbs" title={path}>
      {crumbs.map((c, i) => (
        <span key={c.path} className="row" style={{ gap: 0 }}>
          {i > 0 && <IconChevronRight size={13} className="crumb-sep" />}
          <button
            className={`crumb ${i === crumbs.length - 1 ? "is-last" : ""}`}
            onClick={() => onNavigate(c.path)}
          >
            {c.label}
          </button>
        </span>
      ))}
    </div>
  );
}

function FileRow({
  entry,
  selected,
  onSelect,
  onNavigate,
  onTransfer,
  onDragStart,
}: {
  entry: DirEntry;
  selected: boolean;
  onSelect: () => void;
  onNavigate: (p: string) => void;
  onTransfer: (e: DirEntry) => void;
  onDragStart: (e: React.DragEvent) => void;
}) {
  const isDir = entry.kind === "directory";
  const Icon = isDir ? IconFolder : entry.kind === "symlink" ? IconLink : IconFile;
  return (
    <div
      className={`file-row ${selected ? "is-selected" : ""}`}
      onClick={onSelect}
      onDoubleClick={() => (isDir ? onNavigate(entry.path) : onTransfer(entry))}
      draggable={!isDir}
      onDragStart={onDragStart}
      title={isDir ? `Open ${entry.name}` : `Double-click to transfer ${entry.name}`}
    >
      <div className="file-row__name">
        <Icon size={16} className={`file-row__icon ${isDir ? "dir" : ""}`} />
        <span>{entry.name}</span>
      </div>
      <div className="file-row__meta">{isDir ? "—" : formatBytes(entry.size)}</div>
      <div className="file-row__meta">{formatDate(entry.modified)}</div>
    </div>
  );
}

function FilePane({
  side,
  path,
  entries,
  loading,
  selected,
  onSelect,
  onNavigate,
  onUp,
  onTransfer,
  onRefresh,
  onNewFolder,
  onRename,
  onDelete,
  onDropFromOther,
}: {
  side: Side;
  path: string;
  entries: DirEntry[];
  loading: boolean;
  selected: DirEntry | null;
  onSelect: (e: DirEntry | null) => void;
  onNavigate: (p: string) => void;
  onUp: () => void;
  onTransfer: (e: DirEntry) => void;
  onRefresh: () => void;
  onNewFolder: () => void;
  onRename: (e: DirEntry) => void;
  onDelete: (e: DirEntry) => void;
  onDropFromOther: (payload: DragPayload) => void;
}) {
  const [dragOver, setDragOver] = useState(false);
  const canGoUp = pathCrumbs(path).length > 1;
  const TransferIcon = side === "local" ? IconUpload : IconDownload;
  const transferLabel = side === "local" ? "Upload" : "Download";

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setDragOver(false);
    const raw = e.dataTransfer.getData("application/json");
    if (!raw) return;
    try {
      const payload = JSON.parse(raw) as DragPayload;
      if (payload.side !== side) onDropFromOther(payload);
    } catch {
      /* ignore */
    }
  };

  return (
    <div className="file-pane">
      <div className="file-pane__header">
        <span className="file-pane__title">{side}</span>
        <Breadcrumbs path={path} onNavigate={onNavigate} />
        <button
          className="btn btn--icon btn--sm btn--ghost"
          onClick={onUp}
          disabled={!canGoUp}
          title="Up one level"
        >
          <IconArrowUp size={16} />
        </button>
        <button
          className="btn btn--icon btn--sm btn--ghost"
          onClick={onRefresh}
          title="Refresh"
        >
          <IconRefresh size={16} />
        </button>
        <button
          className="btn btn--icon btn--sm btn--ghost"
          onClick={onNewFolder}
          title="New folder"
        >
          <IconPlus size={16} />
        </button>
      </div>

      {/* Always render this bar so selecting a row never shifts the list
          (which previously moved the row out from under a double-click). */}
      <div className="file-pane__selbar">
        {selected ? (
          <>
            {selected.kind !== "directory" && (
              <button
                className="btn btn--sm btn--primary"
                onClick={() => onTransfer(selected)}
              >
                <TransferIcon size={15} /> {transferLabel}
              </button>
            )}
            <button className="btn btn--sm" onClick={() => onRename(selected)}>
              <IconEdit size={14} /> Rename
            </button>
            <button className="btn btn--sm btn--danger" onClick={() => onDelete(selected)}>
              <IconTrash size={14} /> Delete
            </button>
            <span className="faint" style={{ marginLeft: "auto", fontSize: 12 }}>
              {selected.name}
            </span>
          </>
        ) : (
          <>
            <span className="faint" style={{ fontSize: 12 }}>
              {entries.length} item{entries.length === 1 ? "" : "s"}
            </span>
            <span
              className="faint row"
              style={{ marginLeft: "auto", gap: 6, fontSize: 11 }}
            >
              <TransferIcon size={13} />
              Double-click or drag a file to{" "}
              {side === "local" ? "upload" : "download"}
            </span>
          </>
        )}
      </div>

      <div
        className={`file-list ${dragOver ? "is-dragover" : ""}`}
        onDragOver={(e) => {
          e.preventDefault();
          setDragOver(true);
        }}
        onDragLeave={() => setDragOver(false)}
        onDrop={handleDrop}
        onClick={(e) => {
          if (e.target === e.currentTarget) onSelect(null);
        }}
      >
        {loading ? (
          <div className="empty">
            <div className="spinner" />
          </div>
        ) : (
          <>
            {canGoUp && (
              <div className="file-row up-row" onClick={onUp} title="Up one level">
                <div className="file-row__name">
                  <IconArrowUp size={16} className="file-row__icon" />
                  <span>..</span>
                </div>
                <div className="file-row__meta" />
                <div className="file-row__meta" />
              </div>
            )}
            {entries.length === 0 ? (
              <EmptyState icon={<IconFolder />} title="Empty folder" />
            ) : (
              entries.map((entry) => (
                <FileRow
                  key={entry.path}
                  entry={entry}
                  selected={selected?.path === entry.path}
                  onSelect={() => onSelect(entry)}
                  onNavigate={onNavigate}
                  onTransfer={onTransfer}
                  onDragStart={(e) => {
                    e.dataTransfer.setData(
                      "application/json",
                      JSON.stringify({
                        side,
                        path: entry.path,
                        name: entry.name,
                        isDir: entry.kind === "directory",
                      } satisfies DragPayload),
                    );
                  }}
                />
              ))
            )}
          </>
        )}
      </div>
    </div>
  );
}

export function FileBrowser({
  sessionId,
  active,
}: {
  sessionId: string;
  active: boolean;
}) {
  const [localPath, setLocalPath] = useState("");
  const [remotePath, setRemotePath] = useState("");
  const [localEntries, setLocalEntries] = useState<DirEntry[]>([]);
  const [remoteEntries, setRemoteEntries] = useState<DirEntry[]>([]);
  const [localLoading, setLocalLoading] = useState(false);
  const [remoteLoading, setRemoteLoading] = useState(false);
  const [localSel, setLocalSel] = useState<DirEntry | null>(null);
  const [remoteSel, setRemoteSel] = useState<DirEntry | null>(null);
  const [prompt, setPrompt] = useState<React.ReactNode>(null);

  const transfers = useTransfers((s) => s.transfers);
  const seenComplete = useRef<Set<string>>(new Set());
  const initialised = useRef(false);

  const loadLocal = useCallback(async (p: string) => {
    setLocalLoading(true);
    try {
      setLocalEntries(await api.listLocalDir(p));
      setLocalPath(p);
      setLocalSel(null);
    } catch (e) {
      toast("error", errorMessage(e), "Local folder");
    } finally {
      setLocalLoading(false);
    }
  }, []);

  const loadRemote = useCallback(
    async (p: string) => {
      setRemoteLoading(true);
      try {
        setRemoteEntries(await api.listRemoteDir(sessionId, p));
        setRemotePath(p);
        setRemoteSel(null);
      } catch (e) {
        toast("error", errorMessage(e), "Remote folder");
      } finally {
        setRemoteLoading(false);
      }
    },
    [sessionId],
  );

  // Initial load: local home + remote home.
  useEffect(() => {
    if (initialised.current) return;
    initialised.current = true;
    (async () => {
      try {
        const home = await api.localHomeDir();
        await loadLocal(home);
      } catch {
        await loadLocal("/");
      }
      try {
        const home = await api.remoteHomeDir(sessionId);
        await loadRemote(home || ".");
      } catch (e) {
        toast("error", errorMessage(e), "Could not open remote home");
      }
    })();
  }, [sessionId, loadLocal, loadRemote]);

  // Auto-refresh the relevant pane when a transfer for this session completes.
  useEffect(() => {
    for (const t of Object.values(transfers)) {
      if (
        t.session_id === sessionId &&
        t.state.state === "completed" &&
        !seenComplete.current.has(t.id)
      ) {
        seenComplete.current.add(t.id);
        if (t.direction === "upload") loadRemote(remotePath);
        else loadLocal(localPath);
      }
    }
  }, [transfers, sessionId, remotePath, localPath, loadRemote, loadLocal]);

  const upload = (entry: DirEntry) => {
    if (entry.kind === "directory") {
      toast("info", "Folder transfers aren't supported yet — open the folder first");
      return;
    }
    const dest = joinPath(remotePath, entry.name);
    api
      .uploadFile(sessionId, entry.path, dest)
      .then(() => toast("info", `Uploading ${entry.name}`))
      .catch((e) => toast("error", errorMessage(e), "Upload failed"));
  };

  const download = (entry: DirEntry) => {
    if (entry.kind === "directory") {
      toast("info", "Folder transfers aren't supported yet — open the folder first");
      return;
    }
    const dest = joinLocal(localPath, entry.name);
    api
      .downloadFile(sessionId, entry.path, dest)
      .then(() => toast("info", `Downloading ${entry.name}`))
      .catch((e) => toast("error", errorMessage(e), "Download failed"));
  };

  const onDropToRemote = (p: DragPayload) => {
    if (p.side === "local") upload(placeholderEntry(p));
  };
  const onDropToLocal = (p: DragPayload) => {
    if (p.side === "remote") download(placeholderEntry(p));
  };

  const newFolder = (side: Side) =>
    setPrompt(
      <TextPrompt
        title="New folder"
        label="Folder name"
        confirmLabel="Create"
        onClose={() => setPrompt(null)}
        onSubmit={async (name) => {
          try {
            if (side === "local") {
              await api.makeLocalDir(joinLocal(localPath, name));
              loadLocal(localPath);
            } else {
              await api.makeRemoteDir(sessionId, joinPath(remotePath, name));
              loadRemote(remotePath);
            }
          } catch (e) {
            toast("error", errorMessage(e));
          }
        }}
      />,
    );

  const renameEntry = (side: Side, entry: DirEntry) =>
    setPrompt(
      <TextPrompt
        title="Rename"
        label="New name"
        initial={entry.name}
        confirmLabel="Rename"
        onClose={() => setPrompt(null)}
        onSubmit={async (name) => {
          try {
            if (side === "local") {
              await api.renameLocal(entry.path, joinLocal(localPath, name));
              loadLocal(localPath);
            } else {
              await api.renameRemote(sessionId, entry.path, joinPath(remotePath, name));
              loadRemote(remotePath);
            }
          } catch (e) {
            toast("error", errorMessage(e));
          }
        }}
      />,
    );

  const deleteEntry = (side: Side, entry: DirEntry) =>
    setPrompt(
      <ConfirmDialog
        title={`Delete ${entry.name}?`}
        message={`This permanently removes “${entry.name}”${
          entry.kind === "directory" ? " and everything inside it" : ""
        }.`}
        confirmLabel="Delete"
        danger
        onClose={() => setPrompt(null)}
        onConfirm={async () => {
          try {
            const isDir = entry.kind === "directory";
            if (side === "local") {
              await api.removeLocal(entry.path, isDir);
              loadLocal(localPath);
            } else {
              await api.removeRemote(sessionId, entry.path, isDir);
              loadRemote(remotePath);
            }
          } catch (e) {
            toast("error", errorMessage(e));
          }
        }}
      />,
    );

  void active;

  return (
    <div className="files">
      <FilePane
        side="local"
        path={localPath}
        entries={localEntries}
        loading={localLoading}
        selected={localSel}
        onSelect={setLocalSel}
        onNavigate={loadLocal}
        onUp={async () => {
          const parent = await api.localParentDir(localPath);
          if (parent) loadLocal(parent);
        }}
        onTransfer={upload}
        onRefresh={() => loadLocal(localPath)}
        onNewFolder={() => newFolder("local")}
        onRename={(e) => renameEntry("local", e)}
        onDelete={(e) => deleteEntry("local", e)}
        onDropFromOther={onDropToLocal}
      />

      <FilePane
        side="remote"
        path={remotePath}
        entries={remoteEntries}
        loading={remoteLoading}
        selected={remoteSel}
        onSelect={setRemoteSel}
        onNavigate={loadRemote}
        onUp={() => loadRemote(remoteParent(remotePath))}
        onTransfer={download}
        onRefresh={() => loadRemote(remotePath)}
        onNewFolder={() => newFolder("remote")}
        onRename={(e) => renameEntry("remote", e)}
        onDelete={(e) => deleteEntry("remote", e)}
        onDropFromOther={onDropToRemote}
      />

      {prompt}
    </div>
  );
}

// A drag payload only carries minimal info; reconstruct a partial DirEntry.
function placeholderEntry(p: DragPayload): DirEntry {
  return {
    name: p.name,
    path: p.path,
    kind: p.isDir ? "directory" : "file",
    size: 0,
    permissions: null,
    modified: null,
  };
}
