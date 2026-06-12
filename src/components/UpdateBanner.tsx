import { Modal } from "./ui";
import { IconDownload } from "./Icon";
import { useUpdates } from "@/stores/updates";

/**
 * Shown when a newer version is available. Offers a one-click
 * download-install-restart, with a progress bar while downloading.
 */
export function UpdateBanner() {
  const update = useUpdates((s) => s.update);
  const downloading = useUpdates((s) => s.downloading);
  const progress = useUpdates((s) => s.progress);
  const install = useUpdates((s) => s.install);
  const dismiss = useUpdates((s) => s.dismiss);

  if (!update) return null;

  return (
    <Modal
      title="Update available"
      onClose={downloading ? () => {} : dismiss}
      footer={
        downloading ? undefined : (
          <>
            <button className="btn btn--ghost" onClick={dismiss}>
              Later
            </button>
            <button className="btn btn--primary" onClick={install}>
              <IconDownload size={15} /> Install &amp; restart
            </button>
          </>
        )
      }
    >
      <div className="row" style={{ gap: 12, alignItems: "flex-start" }}>
        <div className="empty__icon" style={{ width: 40, height: 40, color: "var(--accent)" }}>
          <IconDownload size={20} />
        </div>
        <div>
          <strong>Harbor {update.version}</strong> is available — you have{" "}
          {update.currentVersion}.
        </div>
      </div>

      {update.body && (
        <div
          className="muted"
          style={{ whiteSpace: "pre-wrap", fontSize: 13, maxHeight: 200, overflow: "auto" }}
        >
          {update.body}
        </div>
      )}

      {downloading && (
        <div>
          <div className="progress">
            <div
              className="progress__bar"
              style={{ width: `${Math.round(progress * 100)}%` }}
            />
          </div>
          <div className="faint" style={{ fontSize: 12, marginTop: 6 }}>
            Downloading… {Math.round(progress * 100)}% — Harbor will restart when done.
          </div>
        </div>
      )}
    </Modal>
  );
}
