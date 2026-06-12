import { useMemo } from "react";
import {
  IconDownload,
  IconRefresh,
  IconTransfers,
  IconTrash,
  IconUpload,
} from "@/components/Icon";
import { EmptyState } from "@/components/ui";
import { formatBytes } from "@/lib/format";
import { useTransfers } from "@/stores/transfers";
import type { TransferTask } from "@/types";

function stateBadge(task: TransferTask) {
  switch (task.state.state) {
    case "completed":
      return <span className="badge badge--success">Completed</span>;
    case "failed":
      return <span className="badge badge--danger">Failed</span>;
    case "cancelled":
      return <span className="badge">Cancelled</span>;
    case "active":
      return <span className="badge badge--accent">Transferring</span>;
    case "queued":
      return <span className="badge badge--warning">Queued</span>;
    case "paused":
      return <span className="badge">Paused</span>;
  }
}

function TransferRow({ task }: { task: TransferTask }) {
  const cancel = useTransfers((s) => s.cancel);
  const retry = useTransfers((s) => s.retry);

  const pct =
    task.state.state === "completed"
      ? 100
      : task.total_bytes > 0
        ? Math.min(100, (task.transferred_bytes / task.total_bytes) * 100)
        : 0;
  const isActive = task.state.state === "active" || task.state.state === "queued";
  const isError = task.state.state === "failed" || task.state.state === "cancelled";

  return (
    <div className="transfer-row">
      <div className="transfer-row__icon">
        {task.direction === "upload" ? (
          <IconUpload size={18} />
        ) : (
          <IconDownload size={18} />
        )}
      </div>
      <div className="transfer-row__main">
        <div className="row row--between">
          <div className="transfer-row__name">{task.file_name}</div>
          {stateBadge(task)}
        </div>
        <div className="transfer-row__sub mono">
          {task.direction === "upload" ? task.source : task.source} →{" "}
          {task.destination}
        </div>
        <div className="progress">
          <div
            className={`progress__bar ${
              task.state.state === "completed"
                ? "is-done"
                : isError
                  ? "is-error"
                  : ""
            }`}
            style={{ width: `${isError ? 100 : pct}%` }}
          />
        </div>
        <div className="row row--between" style={{ marginTop: 6 }}>
          <span className="faint" style={{ fontSize: 12 }}>
            {task.state.state === "failed"
              ? task.state.error
              : `${formatBytes(task.transferred_bytes)}${
                  task.total_bytes ? ` / ${formatBytes(task.total_bytes)}` : ""
                }`}
          </span>
          <span className="faint" style={{ fontSize: 12 }}>
            {Math.round(pct)}%
          </span>
        </div>
      </div>
      <div className="row" style={{ gap: 6 }}>
        {isActive && (
          <button className="btn btn--sm" onClick={() => cancel(task.id)}>
            Cancel
          </button>
        )}
        {isError && (
          <button className="btn btn--sm" onClick={() => retry(task.id)}>
            <IconRefresh size={14} /> Retry
          </button>
        )}
      </div>
    </div>
  );
}

export function TransfersPage() {
  // Select the raw record (stable reference) and derive views with useMemo;
  // returning a freshly-built array straight from the selector would make
  // zustand's useSyncExternalStore loop and crash the page.
  const transfers = useTransfers((s) => s.transfers);
  const clearFinished = useTransfers((s) => s.clearFinished);
  const tasks = useMemo(
    () =>
      Object.values(transfers).sort(
        (a, b) =>
          new Date(b.created_at).getTime() - new Date(a.created_at).getTime(),
      ),
    [transfers],
  );
  const active = useMemo(
    () =>
      Object.values(transfers).filter(
        (t) => t.state.state === "active" || t.state.state === "queued",
      ).length,
    [transfers],
  );

  return (
    <div className="panel">
      <div className="panel__header">
        <div>
          <div className="panel__title">Transfers</div>
          <div className="panel__subtitle">
            {active > 0 ? `${active} in progress` : "No active transfers"}
          </div>
        </div>
        {tasks.length > 0 && (
          <button className="btn" onClick={clearFinished}>
            <IconTrash size={15} /> Clear finished
          </button>
        )}
      </div>
      <div className="panel__body">
        {tasks.length === 0 ? (
          <EmptyState
            icon={<IconTransfers />}
            title="No transfers yet"
            hint="Upload or download files from the Files view of any connection."
          />
        ) : (
          tasks.map((t) => <TransferRow key={t.id} task={t} />)
        )}
      </div>
    </div>
  );
}
