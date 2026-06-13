import { useCallback, useEffect, useRef, useState } from "react";
import {
  IconActivity,
  IconAlert,
  IconClock,
  IconCpu,
  IconDisk,
  IconMemory,
  IconRefresh,
  IconServer,
} from "./Icon";
import { EmptyState } from "./ui";
import * as api from "@/services/api";
import { formatBytes } from "@/lib/format";
import { errorMessage } from "@/stores/toast";
import type { ServerMetrics } from "@/types";

/** How often to re-poll the server while the panel is visible. */
const REFRESH_MS = 3000;

/** KiB → human bytes (the backend reports memory and disk in kibibytes). */
const fmtKb = (kb: number) => formatBytes(kb * 1024);

function formatUptime(sec: number): string {
  if (sec <= 0) return "—";
  const d = Math.floor(sec / 86400);
  const h = Math.floor((sec % 86400) / 3600);
  const m = Math.floor((sec % 3600) / 60);
  const parts: string[] = [];
  if (d) parts.push(`${d}d`);
  if (h) parts.push(`${h}h`);
  if (m || parts.length === 0) parts.push(`${m}m`);
  return parts.join(" ");
}

function ago(ts: number, now: number): string {
  const s = Math.max(0, Math.round((now - ts) / 1000));
  return s < 1 ? "just now" : `${s}s ago`;
}

/** Severity colour by utilisation: calm under 70%, amber to 90%, red beyond. */
function usageColor(pct: number): string {
  if (pct >= 90) return "var(--danger)";
  if (pct >= 70) return "var(--warning)";
  return "var(--accent)";
}

/** A circular donut gauge with a centred percentage. */
function Gauge({
  percent,
  caption,
}: {
  percent: number;
  caption?: string;
}) {
  const pct = Math.max(0, Math.min(100, percent));
  return (
    <div
      className="gauge"
      style={
        {
          "--pct": pct,
          "--ring": usageColor(pct),
        } as React.CSSProperties
      }
    >
      <div className="gauge__ring">
        <div className="gauge__center">
          <div className="gauge__pct">
            {Math.round(pct)}
            <span>%</span>
          </div>
          {caption && <div className="gauge__cap">{caption}</div>}
        </div>
      </div>
    </div>
  );
}

/** A horizontal usage bar coloured by severity. */
function Bar({ percent }: { percent: number }) {
  const pct = Math.max(0, Math.min(100, percent));
  return (
    <div className="meter">
      <div
        className="meter__fill"
        style={{ width: `${pct}%`, background: usageColor(pct) }}
      />
    </div>
  );
}

function MetricCard({
  icon,
  title,
  meta,
  children,
}: {
  icon: React.ReactNode;
  title: string;
  meta?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <div className="metric-card">
      <div className="metric-card__head">
        <span className="metric-card__icon">{icon}</span>
        <span className="metric-card__title">{title}</span>
        <span className="spacer" />
        {meta && <span className="metric-card__meta">{meta}</span>}
      </div>
      <div className="metric-card__body">{children}</div>
    </div>
  );
}

export function MetricsPane({
  sessionId,
  active,
}: {
  sessionId: string;
  active: boolean;
}) {
  const [metrics, setMetrics] = useState<ServerMetrics | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [updatedAt, setUpdatedAt] = useState<number | null>(null);
  const [auto, setAuto] = useState(true);
  const [now, setNow] = useState(() => Date.now());
  const polling = useRef(false);

  const poll = useCallback(async () => {
    if (polling.current) return; // never overlap a slow probe
    polling.current = true;
    try {
      const m = await api.serverMetrics(sessionId);
      setMetrics(m);
      setError(null);
      setUpdatedAt(Date.now());
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      polling.current = false;
    }
  }, [sessionId]);

  // Poll only while the panel is visible and auto-refresh is on — no background
  // load when the user is on the terminal or files tab.
  useEffect(() => {
    if (!active || !auto) return;
    let cancelled = false;
    const tick = () => {
      if (!cancelled) void poll();
    };
    tick();
    const h = window.setInterval(tick, REFRESH_MS);
    return () => {
      cancelled = true;
      window.clearInterval(h);
    };
  }, [active, auto, poll]);

  // A 1s heartbeat so the "updated Ns ago" label stays honest.
  useEffect(() => {
    if (!active) return;
    const h = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(h);
  }, [active]);

  if (!metrics && error) {
    return (
      <div className="metrics">
        <EmptyState
          icon={<IconAlert />}
          title="Couldn't read server metrics"
          hint={error}
          action={
            <button className="btn btn--primary btn--sm" onClick={() => void poll()}>
              <IconRefresh size={14} /> Retry
            </button>
          }
        />
      </div>
    );
  }

  if (!metrics) {
    return (
      <div className="metrics">
        <div className="empty">
          <div className="spinner" />
        </div>
      </div>
    );
  }

  const { cpu, memory, swap, disks, processes, load } = metrics;
  const loadWarn = cpu.cores > 0 && load.one / cpu.cores >= 1;

  return (
    <div className="metrics">
      {/* ---- Host strip + refresh controls ---- */}
      <div className="metrics-bar">
        <span className="metric-card__icon">
          <IconServer size={16} />
        </span>
        <div className="metrics-bar__id">
          <div className="metrics-bar__host">
            {metrics.hostname || metrics.os || "Remote host"}
          </div>
          {metrics.os && metrics.hostname && (
            <div className="metrics-bar__os">{metrics.os}</div>
          )}
        </div>

        <div className="metrics-bar__chips">
          <span className="stat-chip" title="Uptime">
            <IconClock size={13} /> {formatUptime(metrics.uptime_seconds)}
          </span>
          <span
            className={`stat-chip ${loadWarn ? "is-warn" : ""}`}
            title="Load average (1 / 5 / 15 min)"
          >
            <IconActivity size={13} /> {load.one.toFixed(2)} · {load.five.toFixed(2)} ·{" "}
            {load.fifteen.toFixed(2)}
          </span>
        </div>

        <span className="spacer" />

        <span className="faint metrics-bar__updated">
          {updatedAt ? `Updated ${ago(updatedAt, now)}` : "Loading…"}
        </span>
        <button
          className={`btn btn--sm ${auto ? "btn--primary" : "btn--ghost"}`}
          onClick={() => setAuto((v) => !v)}
          title={auto ? "Auto-refresh on" : "Auto-refresh off"}
        >
          <IconActivity size={14} /> Auto
        </button>
        <button
          className="btn btn--icon btn--sm btn--ghost"
          onClick={() => void poll()}
          title="Refresh now"
        >
          <IconRefresh size={15} />
        </button>
      </div>

      <div className="metrics-body">
        {metrics.unsupported && (
          <div className="callout callout--warning" style={{ marginBottom: "var(--s-4)" }}>
            <IconAlert size={18} style={{ flexShrink: 0 }} />
            <div>
              Live CPU and memory stats need a Linux host that exposes{" "}
              <span className="mono">/proc</span>. Showing whatever this server
              reported.
            </div>
          </div>
        )}

        {/* ---- Gauges: CPU / Memory / Swap ---- */}
        <div className="metric-grid">
          <MetricCard
            icon={<IconCpu size={16} />}
            title="CPU"
            meta={`${cpu.cores} core${cpu.cores === 1 ? "" : "s"}`}
          >
            <div className="metric-card__gauge">
              <Gauge percent={cpu.usage_percent} caption="used" />
              <div className="metric-card__aside">
                {cpu.model && (
                  <div className="metric-kv__model" title={cpu.model}>
                    {cpu.model}
                  </div>
                )}
                {cpu.per_core.length > 0 && (
                  <div className="cores" title="Per-core utilisation">
                    {cpu.per_core.map((p, i) => (
                      <div key={i} className="cores__col" title={`Core ${i}: ${Math.round(p)}%`}>
                        <div
                          className="cores__bar"
                          style={{ height: `${Math.max(3, p)}%`, background: usageColor(p) }}
                        />
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </div>
          </MetricCard>

          <MetricCard
            icon={<IconMemory size={16} />}
            title="Memory"
            meta={fmtKb(memory.total_kb)}
          >
            <div className="metric-card__gauge">
              <Gauge percent={memory.used_percent} caption="used" />
              <div className="metric-card__aside">
                <div className="metric-kv">
                  <span className="muted">Used</span>
                  <span className="mono">{fmtKb(memory.used_kb)}</span>
                </div>
                <div className="metric-kv">
                  <span className="muted">Available</span>
                  <span className="mono">{fmtKb(memory.available_kb)}</span>
                </div>
                <div className="metric-kv">
                  <span className="muted">Total</span>
                  <span className="mono">{fmtKb(memory.total_kb)}</span>
                </div>
              </div>
            </div>
          </MetricCard>

          <MetricCard
            icon={<IconActivity size={16} />}
            title="Swap"
            meta={swap.total_kb > 0 ? fmtKb(swap.total_kb) : undefined}
          >
            {swap.total_kb > 0 ? (
              <div className="metric-card__gauge">
                <Gauge percent={swap.used_percent} caption="used" />
                <div className="metric-card__aside">
                  <div className="metric-kv">
                    <span className="muted">Used</span>
                    <span className="mono">{fmtKb(swap.used_kb)}</span>
                  </div>
                  <div className="metric-kv">
                    <span className="muted">Total</span>
                    <span className="mono">{fmtKb(swap.total_kb)}</span>
                  </div>
                </div>
              </div>
            ) : (
              <div className="metric-empty faint">No swap configured</div>
            )}
          </MetricCard>
        </div>

        {/* ---- Disks ---- */}
        <MetricCard
          icon={<IconDisk size={16} />}
          title="Disks"
          meta={`${disks.length} mount${disks.length === 1 ? "" : "s"}`}
        >
          {disks.length === 0 ? (
            <div className="metric-empty faint">No filesystems reported</div>
          ) : (
            <div className="disk-list">
              {disks.map((d) => (
                <div key={d.mount} className="disk-row">
                  <div className="disk-row__head">
                    <span className="disk-row__mount mono" title={d.filesystem}>
                      {d.mount}
                    </span>
                    <span className="disk-row__nums faint">
                      {fmtKb(d.used_kb)} / {fmtKb(d.total_kb)} ·{" "}
                      {Math.round(d.used_percent)}%
                    </span>
                  </div>
                  <Bar percent={d.used_percent} />
                </div>
              ))}
            </div>
          )}
        </MetricCard>

        {/* ---- Top processes ---- */}
        <MetricCard icon={<IconActivity size={16} />} title="Top processes">
          {processes.length === 0 ? (
            <div className="metric-empty faint">No process data</div>
          ) : (
            <div className="proc-table">
              <div className="proc-row proc-row--head">
                <span>Command</span>
                <span>CPU</span>
                <span>MEM</span>
              </div>
              {processes.map((p, i) => (
                <div key={i} className="proc-row">
                  <span className="proc-row__cmd mono" title={p.command}>
                    {p.command}
                  </span>
                  <span className="proc-row__num" style={{ color: usageColor(p.cpu_percent) }}>
                    {p.cpu_percent.toFixed(1)}%
                  </span>
                  <span className="proc-row__num muted">{p.mem_percent.toFixed(1)}%</span>
                </div>
              ))}
            </div>
          )}
        </MetricCard>
      </div>
    </div>
  );
}
