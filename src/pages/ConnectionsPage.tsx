import { useEffect, useState } from "react";
import {
  IconAgent,
  IconEdit,
  IconKey,
  IconLock,
  IconPlay,
  IconPlus,
  IconSearch,
  IconServer,
  IconStar,
  IconTrash,
} from "@/components/Icon";
import { ProfileEditor } from "@/components/ProfileEditor";
import { SecretModal } from "@/components/HostKeyPromptModal";
import { ConfirmDialog, EmptyState } from "@/components/ui";
import * as api from "@/services/api";
import { initials } from "@/lib/format";
import { errorCode, errorMessage, toast } from "@/stores/toast";
import { filteredProfiles, useProfiles } from "@/stores/profiles";
import { useSessions } from "@/stores/sessions";
import type { AuthKind, ConnectRequest, ServerProfile } from "@/types";

function authIcon(kind: AuthKind) {
  if (kind === "agent") return <IconAgent size={15} />;
  if (kind === "password") return <IconLock size={15} />;
  return <IconKey size={15} />;
}

function authLabel(kind: AuthKind) {
  return kind === "agent"
    ? "SSH Agent"
    : kind === "password"
      ? "Password"
      : "Private Key";
}

export function ConnectionsPage() {
  const profiles = useProfiles();
  const list = filteredProfiles(profiles);
  const selected = profiles.profiles.find((p) => p.id === profiles.selectedId) ?? null;

  const addSession = useSessions((s) => s.addSession);

  const [editing, setEditing] = useState<ServerProfile | null | undefined>(undefined);
  const [confirmDelete, setConfirmDelete] = useState<ServerProfile | null>(null);
  const [connecting, setConnecting] = useState<string | null>(null);
  const [secret, setSecret] = useState<{ profile: ServerProfile; kind: "password" | "passphrase" } | null>(null);
  const [hasSavedPassword, setHasSavedPassword] = useState(false);

  useEffect(() => {
    if (selected?.auth.type === "password") {
      api.hasProfilePassword(selected.id).then(setHasSavedPassword).catch(() => {});
    } else {
      setHasSavedPassword(false);
    }
  }, [selected?.id, selected?.auth.type]);

  const doConnect = async (
    profile: ServerProfile,
    secretValue?: string,
    remember?: boolean,
  ) => {
    setConnecting(profile.id);
    try {
      const req: ConnectRequest = { profile_id: profile.id };
      if (secretValue != null) {
        if (secret?.kind === "password") req.password = secretValue;
        else req.passphrase = secretValue;
        req.remember_secret = remember ?? false;
      }
      const info = await api.connect(req);
      addSession(info);
      setSecret(null);
      toast("success", `Connected to ${profile.name}`);
    } catch (e) {
      const code = errorCode(e);
      if (code === "password_required") setSecret({ profile, kind: "password" });
      else if (code === "passphrase_required") setSecret({ profile, kind: "passphrase" });
      else toast("error", errorMessage(e), "Connection failed");
    } finally {
      setConnecting(null);
    }
  };

  const favorites = list.filter((p) => p.favorite);
  const others = list.filter((p) => !p.favorite);

  return (
    <div className="panel" style={{ height: "100%" }}>
      <div className="conn-layout">
        {/* List */}
        <div className="conn-list">
          <div className="conn-list__toolbar">
            <div className="search" style={{ flex: 1 }}>
              <IconSearch />
              <input
                className="input"
                placeholder="Search connections…"
                value={profiles.query}
                onChange={(e) => profiles.setQuery(e.target.value)}
              />
            </div>
            <button
              className="btn btn--primary btn--icon"
              onClick={() => setEditing(null)}
              title="New connection"
            >
              <IconPlus />
            </button>
          </div>

          <div className="conn-list__scroll">
            {list.length === 0 ? (
              <EmptyState
                icon={<IconServer />}
                title={profiles.query ? "No matches" : "No connections yet"}
                hint={profiles.query ? undefined : "Create your first connection to get started."}
              />
            ) : (
              <>
                {favorites.length > 0 && (
                  <>
                    <div className="conn-group__label">Favorites</div>
                    {favorites.map((p) => (
                      <ConnCard
                        key={p.id}
                        profile={p}
                        selected={p.id === profiles.selectedId}
                        onSelect={() => profiles.select(p.id)}
                        onToggleFav={() => profiles.toggleFavorite(p.id)}
                      />
                    ))}
                  </>
                )}
                {others.length > 0 && favorites.length > 0 && (
                  <div className="conn-group__label">All connections</div>
                )}
                {others.map((p) => (
                  <ConnCard
                    key={p.id}
                    profile={p}
                    selected={p.id === profiles.selectedId}
                    onSelect={() => profiles.select(p.id)}
                    onToggleFav={() => profiles.toggleFavorite(p.id)}
                  />
                ))}
              </>
            )}
          </div>
        </div>

        {/* Detail */}
        {selected ? (
          <div className="conn-detail">
            <div className="row row--between">
              <div className="row" style={{ gap: 14 }}>
                <div className="conn-card__avatar" style={{ width: 52, height: 52, fontSize: 18 }}>
                  {initials(selected.name)}
                </div>
                <div>
                  <div style={{ fontSize: 20, fontWeight: 650 }}>{selected.name}</div>
                  <div className="muted mono">
                    {selected.username}@{selected.host}:{selected.port}
                  </div>
                </div>
              </div>
              <button
                className="btn btn--primary"
                onClick={() => doConnect(selected)}
                disabled={connecting === selected.id}
              >
                {connecting === selected.id ? (
                  <span className="spinner" />
                ) : (
                  <IconPlay size={15} />
                )}
                Connect
              </button>
            </div>

            <div className="detail-card">
              <div className="detail-row">
                <span className="detail-row__key">Host</span>
                <span className="detail-row__val">{selected.host}</span>
              </div>
              <div className="detail-row">
                <span className="detail-row__key">Port</span>
                <span className="detail-row__val">{selected.port}</span>
              </div>
              <div className="detail-row">
                <span className="detail-row__key">Username</span>
                <span className="detail-row__val">{selected.username}</span>
              </div>
              <div className="detail-row">
                <span className="detail-row__key">Authentication</span>
                <span className="detail-row__val row" style={{ gap: 6, justifyContent: "flex-end" }}>
                  {authIcon(selected.auth.type)} {authLabel(selected.auth.type)}
                </span>
              </div>
              {selected.auth.type === "public_key" && (
                <div className="detail-row">
                  <span className="detail-row__key">Key file</span>
                  <span className="detail-row__val">{selected.auth.key_path}</span>
                </div>
              )}
            </div>

            {selected.auth.type === "password" && (
              <div className="detail-card row row--between">
                <div className="row" style={{ gap: 10 }}>
                  <IconLock size={18} className="muted" />
                  <div>
                    <div style={{ fontWeight: 600 }}>Saved password</div>
                    <div className="faint" style={{ fontSize: 12 }}>
                      {hasSavedPassword
                        ? "A password is stored in your OS keychain."
                        : "No password stored — you'll be asked on connect."}
                    </div>
                  </div>
                </div>
                {hasSavedPassword && (
                  <button
                    className="btn btn--sm btn--danger"
                    onClick={async () => {
                      await api.clearProfilePassword(selected.id);
                      setHasSavedPassword(false);
                      toast("info", "Password removed from keychain");
                    }}
                  >
                    Forget
                  </button>
                )}
              </div>
            )}

            {selected.tags.length > 0 && (
              <div className="tag-row">
                {selected.tags.map((t) => (
                  <span key={t} className="tag">
                    {t}
                  </span>
                ))}
              </div>
            )}

            {selected.notes && (
              <div className="detail-card">
                <div className="field__label" style={{ marginBottom: 8 }}>
                  Notes
                </div>
                <div className="muted" style={{ whiteSpace: "pre-wrap" }}>
                  {selected.notes}
                </div>
              </div>
            )}

            <div className="row" style={{ gap: 8 }}>
              <button className="btn" onClick={() => setEditing(selected)}>
                <IconEdit size={15} /> Edit
              </button>
              <button
                className="btn"
                onClick={() => profiles.toggleFavorite(selected.id)}
              >
                <IconStar size={15} filled={selected.favorite} /> {selected.favorite ? "Unfavorite" : "Favorite"}
              </button>
              <div className="spacer" />
              <button className="btn btn--danger" onClick={() => setConfirmDelete(selected)}>
                <IconTrash size={15} /> Delete
              </button>
            </div>
          </div>
        ) : (
          <EmptyState
            icon={<IconServer />}
            title="Select a connection"
            hint="Choose a saved connection on the left, or create a new one."
          />
        )}
      </div>

      {editing !== undefined && (
        <ProfileEditor
          existing={editing ?? undefined}
          onClose={() => setEditing(undefined)}
        />
      )}
      {confirmDelete && (
        <ConfirmDialog
          title={`Delete “${confirmDelete.name}”?`}
          message="This removes the saved connection and any password stored for it."
          confirmLabel="Delete"
          danger
          onClose={() => setConfirmDelete(null)}
          onConfirm={() => profiles.remove(confirmDelete.id)}
        />
      )}
      {secret && (
        <SecretModal
          title={secret.kind === "password" ? "Password required" : "Key passphrase"}
          label={
            secret.kind === "password"
              ? `Password for ${secret.profile.username}@${secret.profile.host}`
              : "Passphrase to decrypt the private key"
          }
          onClose={() => setSecret(null)}
          onSubmit={(value, remember) => doConnect(secret.profile, value, remember)}
        />
      )}
    </div>
  );
}

function ConnCard({
  profile,
  selected,
  onSelect,
  onToggleFav,
}: {
  profile: ServerProfile;
  selected: boolean;
  onSelect: () => void;
  onToggleFav: () => void;
}) {
  return (
    <div className={`conn-card ${selected ? "is-selected" : ""}`} onClick={onSelect}>
      <div className="conn-card__avatar">{initials(profile.name)}</div>
      <div className="conn-card__main">
        <div className="conn-card__name">{profile.name}</div>
        <div className="conn-card__sub">
          {profile.username}@{profile.host}
        </div>
      </div>
      <button
        className={`btn btn--icon btn--sm btn--ghost conn-card__star ${profile.favorite ? "is-fav" : ""}`}
        onClick={(e) => {
          e.stopPropagation();
          onToggleFav();
        }}
        title={profile.favorite ? "Unfavorite" : "Favorite"}
      >
        <IconStar size={15} filled={profile.favorite} />
      </button>
    </div>
  );
}
