import { useCallback, useEffect, useRef, useState } from "react";
import {
  IconArrowUp,
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

function FileRow({
  entry,
  selected,
  onSelect,
  onOpen,
  onDragStart,
}: {
  entry: DirEntry;
  selected: boolean;
  onSelect: () => void;
  onOpen: () => void;
  onDragStart: (e: React.DragEvent) => void;
}) {
  const Icon =
    entry.kind === "directory"
      ? IconFolder
      : entry.kind === "symlink"
        ? IconLink
        : IconFile;
  return (
    <div
      className={`file-row ${selected ? "is-selected" : ""}`}
      onClick={onSelect}
      onDoubleClick={onOpen}
      draggable={entry.kind !== "directory"}
      onDragStart={onDragStart}
      title={entry.name}
    >
      <div className="file-row__name">
        <Icon
          size={16}
          className={`file-row__icon ${entry.kind === "directory" ? "dir" : ""}`}
        />
        <span>{entry.name}</span>
      </div>
      <div className="file-row__meta">
        {entry.kind === "directory" ? "—" : formatBytes(entry.size)}
      </div>
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
  onUp,
  onOpenEntry,
  onRefresh,
  onDropFromOther,
  actions,
}: {
  side: Side;
  path: string;
  entries: DirEntry[];
  loading: boolean;
  selected: DirEntry | null;
  onSelect: (e: DirEntry | null) => void;
  onUp: () => void;
  onOpenEntry: (e: DirEntry) => void;
  onRefresh: () => void;
  onDropFromOther: (payload: DragPayload) => void;
  actions: React.ReactNode;
}) {
  const [dragOver, setDragOver] = useState(false);

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
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
        <div className="path-bar" title={path}>
          {path || "—"}
        </div>
        <button className="btn btn--icon btn--sm btn--ghost" onClick={onUp} title="Up">
          <IconArrowUp size={16} />
        </button>
        <button
          className="btn btn--icon btn--sm btn--ghost"
          onClick={onRefresh}
          title="Refresh"
        >
          <IconRefresh size={16} />
        </button>
        {actions}
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
        style={dragOver ? { boxShadow: "inset 0 0 0 2px var(--accent)" } : undefined}
      >
        {loading ? (
          <div className="empty">
            <div className="spinner" />
          </div>
        ) : entries.length === 0 ? (
          <EmptyState icon={<IconFolder />} title="Empty folder" />
        ) : (
          entries.map((entry) => (
            <FileRow
              key={entry.path}
              entry={entry}
              selected={selected?.path === entry.path}
              onSelect={() => onSelect(entry)}
              onOpen={() => onOpenEntry(entry)}
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
    if (p.side === "local") upload({ ...placeholderEntry(p) });
  };
  const onDropToLocal = (p: DragPayload) => {
    if (p.side === "remote") download({ ...placeholderEntry(p) });
  };

  const newFolderLocal = () =>
    setPrompt(
      <TextPrompt
        title="New folder"
        label="Folder name"
        confirmLabel="Create"
        onClose={() => setPrompt(null)}
        onSubmit={async (name) => {
          try {
            await api.makeLocalDir(joinLocal(localPath, name));
            loadLocal(localPath);
          } catch (e) {
            toast("error", errorMessage(e));
          }
        }}
      />,
    );

  const newFolderRemote = () =>
    setPrompt(
      <TextPrompt
        title="New folder"
        label="Folder name"
        confirmLabel="Create"
        onClose={() => setPrompt(null)}
        onSubmit={async (name) => {
          try {
            await api.makeRemoteDir(sessionId, joinPath(remotePath, name));
            loadRemote(remotePath);
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
        onUp={async () => {
          const parent = await api.localParentDir(localPath);
          if (parent) loadLocal(parent);
        }}
        onOpenEntry={(e) => e.kind === "directory" && loadLocal(e.path)}
        onRefresh={() => loadLocal(localPath)}
        onDropFromOther={onDropToLocal}
        actions={
          <>
            <button
              className="btn btn--icon btn--sm btn--ghost"
              onClick={newFolderLocal}
              title="New folder"
            >
              <IconPlus size={16} />
            </button>
            {localSel && (
              <>
                <button
                  className="btn btn--sm"
                  onClick={() => upload(localSel)}
                  title="Upload to remote"
                >
                  <IconUpload size={15} /> Upload
                </button>
                <button
                  className="btn btn--icon btn--sm btn--ghost"
                  onClick={() => renameEntry("local", localSel)}
                  title="Rename"
                >
                  <IconEdit size={15} />
                </button>
                <button
                  className="btn btn--icon btn--sm btn--danger"
                  onClick={() => deleteEntry("local", localSel)}
                  title="Delete"
                >
                  <IconTrash size={15} />
                </button>
              </>
            )}
          </>
        }
      />

      <FilePane
        side="remote"
        path={remotePath}
        entries={remoteEntries}
        loading={remoteLoading}
        selected={remoteSel}
        onSelect={setRemoteSel}
        onUp={() => loadRemote(remoteParent(remotePath))}
        onOpenEntry={(e) => e.kind === "directory" && loadRemote(e.path)}
        onRefresh={() => loadRemote(remotePath)}
        onDropFromOther={onDropToRemote}
        actions={
          <>
            <button
              className="btn btn--icon btn--sm btn--ghost"
              onClick={newFolderRemote}
              title="New folder"
            >
              <IconPlus size={16} />
            </button>
            {remoteSel && (
              <>
                <button
                  className="btn btn--sm"
                  onClick={() => download(remoteSel)}
                  title="Download to local"
                >
                  <IconDownload size={15} /> Download
                </button>
                <button
                  className="btn btn--icon btn--sm btn--ghost"
                  onClick={() => renameEntry("remote", remoteSel)}
                  title="Rename"
                >
                  <IconEdit size={15} />
                </button>
                <button
                  className="btn btn--icon btn--sm btn--danger"
                  onClick={() => deleteEntry("remote", remoteSel)}
                  title="Delete"
                >
                  <IconTrash size={15} />
                </button>
              </>
            )}
          </>
        }
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
