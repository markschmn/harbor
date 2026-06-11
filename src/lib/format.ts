// Small formatting helpers shared across the UI.

export function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.min(
    units.length - 1,
    Math.floor(Math.log(bytes) / Math.log(1024)),
  );
  const value = bytes / Math.pow(1024, i);
  return `${value.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

export function formatSpeed(bytesPerSecond: number): string {
  return `${formatBytes(bytesPerSecond)}/s`;
}

export function formatDate(iso: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "—";
  const now = Date.now();
  const diff = now - d.getTime();
  const min = 60_000;
  const hour = 60 * min;
  const day = 24 * hour;
  if (diff < min) return "just now";
  if (diff < hour) return `${Math.floor(diff / min)}m ago`;
  if (diff < day) return `${Math.floor(diff / hour)}h ago`;
  if (diff < 7 * day) return `${Math.floor(diff / day)}d ago`;
  return d.toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

export function permString(perm: number | null, isDir: boolean): string {
  if (perm == null) return "";
  const flags = [
    0o400, 0o200, 0o100, 0o040, 0o020, 0o010, 0o004, 0o002, 0o001,
  ];
  const chars = "rwxrwxrwx";
  let out = isDir ? "d" : "-";
  flags.forEach((bit, i) => {
    out += perm & bit ? chars[i] : "-";
  });
  return out;
}

export function initials(name: string): string {
  const parts = name.trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0) return "?";
  if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase();
  return (parts[0][0] + parts[1][0]).toUpperCase();
}

export function joinPath(dir: string, name: string): string {
  if (!dir || dir === "/") return `/${name}`;
  return dir.endsWith("/") ? `${dir}${name}` : `${dir}/${name}`;
}

/** Join a local path using the platform separator inferred from `base`. */
export function joinLocal(dir: string, name: string): string {
  const sep = dir.includes("\\") && !dir.includes("/") ? "\\" : "/";
  if (dir.endsWith(sep)) return `${dir}${name}`;
  return `${dir}${sep}${name}`;
}
