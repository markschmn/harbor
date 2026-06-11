import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Modal, Switch } from "./ui";
import { IconAgent, IconFolder, IconKey, IconLock } from "./Icon";
import { useProfiles } from "@/stores/profiles";
import { toast } from "@/stores/toast";
import type { AuthKind, AuthMethod, ProfileDraft, ServerProfile } from "@/types";

function toDraft(p: ServerProfile): ProfileDraft {
  return {
    name: p.name,
    host: p.host,
    port: p.port,
    username: p.username,
    auth: p.auth,
    notes: p.notes,
    tags: p.tags,
    favorite: p.favorite,
  };
}

function emptyDraft(): ProfileDraft {
  return {
    name: "",
    host: "",
    port: 22,
    username: "",
    auth: { type: "agent" },
    notes: "",
    tags: [],
    favorite: false,
  };
}

const AUTH_OPTIONS: { kind: AuthKind; label: string; icon: typeof IconKey }[] = [
  { kind: "agent", label: "SSH Agent", icon: IconAgent },
  { kind: "public_key", label: "Private Key", icon: IconKey },
  { kind: "password", label: "Password", icon: IconLock },
];

export function ProfileEditor({
  existing,
  onClose,
}: {
  existing?: ServerProfile;
  onClose: () => void;
}) {
  const create = useProfiles((s) => s.create);
  const update = useProfiles((s) => s.update);
  const [draft, setDraft] = useState<ProfileDraft>(
    existing ? toDraft(existing) : emptyDraft(),
  );
  const [tagsText, setTagsText] = useState(draft.tags.join(", "));
  const [saving, setSaving] = useState(false);

  const patch = (p: Partial<ProfileDraft>) => setDraft((d) => ({ ...d, ...p }));

  const authKind = draft.auth.type;
  const keyPath = draft.auth.type === "public_key" ? draft.auth.key_path : "";
  const keyEncrypted = draft.auth.type === "public_key" ? draft.auth.encrypted : false;

  const setAuthKind = (kind: AuthKind) => {
    let auth: AuthMethod;
    if (kind === "public_key") {
      auth = { type: "public_key", key_path: keyPath, encrypted: keyEncrypted };
    } else {
      auth = { type: kind };
    }
    patch({ auth });
  };

  const browseKey = async () => {
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        title: "Select a private key",
      });
      if (typeof selected === "string") {
        patch({ auth: { type: "public_key", key_path: selected, encrypted: keyEncrypted } });
      }
    } catch {
      /* user cancelled */
    }
  };

  const submit = async () => {
    if (!draft.name.trim() || !draft.host.trim() || !draft.username.trim()) {
      toast("error", "Name, host and username are required");
      return;
    }
    if (authKind === "public_key" && !keyPath.trim()) {
      toast("error", "Choose a private key file");
      return;
    }
    const finalDraft: ProfileDraft = {
      ...draft,
      tags: tagsText
        .split(",")
        .map((t) => t.trim())
        .filter(Boolean),
    };
    setSaving(true);
    const result = existing
      ? await update(existing.id, finalDraft)
      : await create(finalDraft);
    setSaving(false);
    if (result) onClose();
  };

  return (
    <Modal
      title={existing ? "Edit connection" : "New connection"}
      onClose={onClose}
      wide
      footer={
        <>
          <button className="btn btn--ghost" onClick={onClose}>
            Cancel
          </button>
          <button className="btn btn--primary" onClick={submit} disabled={saving}>
            {saving ? "Saving…" : existing ? "Save changes" : "Create"}
          </button>
        </>
      }
    >
      <div className="form-grid">
        <div className="field span-2">
          <label className="field__label">Name</label>
          <input
            className="input"
            placeholder="Production web server"
            value={draft.name}
            autoFocus
            onChange={(e) => patch({ name: e.target.value })}
          />
        </div>

        <div className="field">
          <label className="field__label">Host</label>
          <input
            className="input"
            placeholder="example.com"
            value={draft.host}
            onChange={(e) => patch({ host: e.target.value })}
          />
        </div>
        <div className="field">
          <label className="field__label">Port</label>
          <input
            className="input"
            type="number"
            min={1}
            max={65535}
            value={draft.port ?? 22}
            onChange={(e) => patch({ port: Number(e.target.value) || 22 })}
          />
        </div>

        <div className="field span-2">
          <label className="field__label">Username</label>
          <input
            className="input"
            placeholder="root"
            value={draft.username}
            onChange={(e) => patch({ username: e.target.value })}
          />
        </div>

        <div className="field span-2">
          <label className="field__label">Authentication</label>
          <div className="segmented" style={{ width: "fit-content" }}>
            {AUTH_OPTIONS.map(({ kind, label, icon: Icon }) => (
              <button
                key={kind}
                className={authKind === kind ? "is-active" : ""}
                onClick={() => setAuthKind(kind)}
              >
                <span className="row" style={{ gap: 6 }}>
                  <Icon size={14} /> {label}
                </span>
              </button>
            ))}
          </div>
        </div>

        {authKind === "public_key" && (
          <>
            <div className="field span-2">
              <label className="field__label">Private key</label>
              <div className="row">
                <input
                  className="input mono"
                  placeholder="~/.ssh/id_ed25519"
                  value={keyPath}
                  onChange={(e) =>
                    patch({
                      auth: {
                        type: "public_key",
                        key_path: e.target.value,
                        encrypted: keyEncrypted,
                      },
                    })
                  }
                />
                <button className="btn" onClick={browseKey}>
                  <IconFolder size={16} /> Browse
                </button>
              </div>
            </div>
            <div className="field span-2 row row--between">
              <div>
                <div className="field__label">Key is passphrase-protected</div>
                <div className="field__hint">
                  You'll be asked for the passphrase when connecting.
                </div>
              </div>
              <Switch
                on={keyEncrypted}
                onChange={(v) =>
                  patch({
                    auth: { type: "public_key", key_path: keyPath, encrypted: v },
                  })
                }
              />
            </div>
          </>
        )}

        {authKind === "agent" && (
          <div className="callout callout--warning span-2">
            Authentication is delegated to your running SSH agent. No secrets are
            stored by Harbor.
          </div>
        )}

        <div className="field span-2">
          <label className="field__label">Tags</label>
          <input
            className="input"
            placeholder="prod, web, eu-west (comma separated)"
            value={tagsText}
            onChange={(e) => setTagsText(e.target.value)}
          />
        </div>

        <div className="field span-2">
          <label className="field__label">Notes</label>
          <textarea
            className="textarea"
            placeholder="Anything worth remembering about this host…"
            value={draft.notes}
            onChange={(e) => patch({ notes: e.target.value })}
          />
        </div>

        <div className="field span-2 row row--between">
          <div className="field__label">Add to favorites</div>
          <Switch on={draft.favorite} onChange={(v) => patch({ favorite: v })} />
        </div>
      </div>
    </Modal>
  );
}
