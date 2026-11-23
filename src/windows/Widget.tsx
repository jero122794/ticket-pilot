import { useEffect, useMemo, useRef, useState } from "react";
import type { MouseEvent } from "react";
import { ArrowUpRight, ClipboardList, CloudOff, LayoutDashboard, Minus, Pause, Play, Search, Square } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { api, asAppError, liveWorked, useNow, useSnapshot } from "../lib/api";
import type { Snapshot, Ticket } from "../lib/api";
import { bucketLabel, clock, duration } from "../lib/format";
import { Button, Clock, IconButton, Input, Textarea, Tip } from "../ui";

type Mode = "main" | "pick" | "pauseReason" | "confirmStop";

/** Alto de la ventana para cada modo (la ventana no tiene bordes y se ajusta al contenido). */
const HEIGHT: Record<Mode, number> = {
  main: 176,
  pick: 400,
  pauseReason: 240,
  confirmStop: 178,
};

export default function Widget() {
  const [snap, setSnap] = useSnapshot();
  const [mode, setMode] = useState<Mode>("main");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [askReason, setAskReason] = useState(true);

  const cur = snap?.current ?? null;
  const now = useNow(cur?.state === "running");

  useEffect(() => {
    api.getSettings().then((s) => setAskReason(s.askPauseReason)).catch(() => {});
  }, []);

  useEffect(() => {
    api.resizeWidget(HEIGHT[mode] + (error ? 38 : 0)).catch(() => {});
  }, [mode, error]);

  async function run(action: () => Promise<Snapshot>) {
    setBusy(true);
    setError(null);
    try {
      setSnap(await action());
      setMode("main");
    } catch (e) {
      const err = asAppError(e);
      if (err.code === "missing_fields" && snap?.current) {
        // No se finaliza con campos vacíos: el formulario se abre en el panel.
        api.showMain(`finish:${snap.current.taskId}`);
        setMode("main");
      } else setError(err.message);
    } finally {
      setBusy(false);
    }
  }

  const drag = (e: MouseEvent) => {
    if (e.button === 0 && !(e.target as HTMLElement).closest("button")) {
      getCurrentWindow().startDragging().catch(() => {});
    }
  };

  if (!snap) return <div className="widget" />;

  const state = cur?.state ?? "idle";
  const syncProblem = snap.lastError ?? (snap.failures.length ? "Hay cambios que no llegaron a ClickUp." : null);

  return (
    <div className="widget">
      <div className="widget__bar" onMouseDown={drag}>
        <span className="status-dot" data-state={state} aria-hidden />
        {cur ? (
          <span className="code">{cur.label}</span>
        ) : (
          <span>{snap.configured ? "sin ticket activo" : "ticketpilot"}</span>
        )}
        {cur && state === "paused" && <span>en pausa</span>}
        <span className="spacer" />
        {syncProblem && (
          <Tip label={syncProblem}>
            <CloudOff size={14} aria-label="Problema de sincronización" />
          </Tip>
        )}
        {cur && cur.missingFields.length > 0 && (
          <IconButton
            size="sm"
            label={`${cur.missingFields.length} campos sin llenar`}
            onClick={() => api.showMain(`fill:${cur.taskId}`)}
          >
            <ClipboardList />
          </IconButton>
        )}
        {cur && (
          <IconButton size="sm" label="Abrir en ClickUp" onClick={() => api.openUrl(cur.url)}>
            <ArrowUpRight />
          </IconButton>
        )}
        <IconButton size="sm" label="Abrir panel" onClick={() => api.showMain()}>
          <LayoutDashboard />
        </IconButton>
        <IconButton size="sm" label="Ocultar (sigue en la bandeja)" onClick={() => api.toggleWidget()}>
          <Minus />
        </IconButton>
      </div>

      <div className="widget__body">
        {error && (
          <div className="mini-alert" role="alert">
            <span className="truncate" style={{ flex: 1 }} title={error}>
              {error}
            </span>
            <Button variant="ghost" size="sm" onClick={() => setError(null)}>
              Cerrar
            </Button>
          </div>
        )}

        {!snap.configured ? (
          <div className="widget__main" style={{ flex: 1 }}>
            <span className="muted" style={{ flex: 1 }}>
              Conecta ClickUp para empezar.
            </span>
            <Button size="sm" variant="primary" onClick={() => api.showMain("settings")}>
              Conectar
            </Button>
          </div>
        ) : mode === "pick" ? (
          <Picker tickets={snap.tickets} busy={busy} onCancel={() => setMode("main")} onPick={(t) => run(() => api.play(t.id))} />
        ) : mode === "pauseReason" && cur ? (
          <PauseReason busy={busy} onCancel={() => setMode("main")} onConfirm={(r) => run(() => api.pause(r))} />
        ) : mode === "confirmStop" && cur ? (
          <>
            <p>
              ¿Finalizar <b>{cur.label}</b>? Se registra la fecha de fin y el ticket cambia de estado en ClickUp.
            </p>
            <div className="row" style={{ justifyContent: "flex-end" }}>
              <Button variant="ghost" size="sm" onClick={() => setMode("main")}>
                Seguir trabajando
              </Button>
              <Button variant="destructive" size="sm" loading={busy} onClick={() => run(() => api.stop(cur.taskId))}>
                Finalizar ticket
              </Button>
            </div>
          </>
        ) : (
          <>
            {/* Centrado: reloj arriba, nombre del ticket y botones debajo. */}
            <div className="widget__center">
              <Clock className="widget__clock" state={state} text={clock(cur ? liveWorked(cur, now) : 0)} />
              <div className="widget__name truncate" title={cur?.name}>
                {cur ? cur.name : "ningún cronómetro corriendo"}
              </div>
            </div>
            <div className="widget__actions">
              {!cur && (
                <Button variant="primary" onClick={() => setMode("pick")}>
                  <Play /> elegir ticket
                </Button>
              )}
              {cur?.state === "running" && (
                <Button variant="primary" disabled={busy} onClick={() => (askReason ? setMode("pauseReason") : run(() => api.pause("")))}>
                  <Pause fill="currentColor" /> pausar
                </Button>
              )}
              {cur?.state === "paused" && (
                <Button variant="primary" disabled={busy} onClick={() => run(() => api.play(cur.taskId))}>
                  <Play fill="currentColor" /> reanudar
                </Button>
              )}
              {cur && (
                <Button disabled={busy} onClick={() => setMode("confirmStop")}>
                  <Square /> finalizar
                </Button>
              )}
              {cur && (
                <IconButton label="Cambiar de ticket" disabled={busy} onClick={() => setMode("pick")}>
                  <Search />
                </IconButton>
              )}
            </div>
          </>
        )}
      </div>
    </div>
  );
}

function PauseReason({ busy, onConfirm, onCancel }: { busy: boolean; onConfirm: (r: string) => void; onCancel: () => void }) {
  const [reason, setReason] = useState("");
  const ref = useRef<HTMLTextAreaElement>(null);
  useEffect(() => ref.current?.focus(), []);
  return (
    <>
      <label className="label" htmlFor="pause-reason">
        ¿Por qué pausas?
      </label>
      <Textarea
        id="pause-reason"
        ref={ref}
        rows={3}
        value={reason}
        placeholder="Ej.: me reasignaron un ticket urgente"
        onChange={(e) => setReason(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) onConfirm(reason);
          if (e.key === "Escape") onCancel();
        }}
      />
      <div className="row">
        <span className="hint" style={{ flex: 1 }}>
          <span className="kbd">Ctrl</span> + <span className="kbd">Enter</span>
        </span>
        <Button variant="ghost" size="sm" onClick={onCancel}>
          Cancelar
        </Button>
        <Button size="sm" variant="primary" loading={busy} onClick={() => onConfirm(reason)}>
          <Pause /> Pausar
        </Button>
      </div>
    </>
  );
}

function Picker({
  tickets,
  busy,
  onPick,
  onCancel,
}: {
  tickets: Ticket[];
  busy: boolean;
  onPick: (t: Ticket) => void;
  onCancel: () => void;
}) {
  const [q, setQ] = useState("");
  const [active, setActive] = useState(0);
  const shown = useMemo(() => {
    const needle = q.trim().toLowerCase();
    return tickets
      .filter((t) => t.bucket !== "done" && t.sessionState !== "running")
      .filter((t) => !needle || `${t.label} ${t.name}`.toLowerCase().includes(needle));
  }, [tickets, q]);

  useEffect(() => setActive(0), [q]);

  return (
    <>
      <div className="input-icon">
        <Search />
        <Input
          autoFocus
          aria-label="Buscar ticket"
          placeholder="Buscar por código o nombre"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") onCancel();
            if (e.key === "ArrowDown") setActive((a) => Math.min(a + 1, shown.length - 1));
            if (e.key === "ArrowUp") setActive((a) => Math.max(a - 1, 0));
            if (e.key === "Enter" && shown[active]) onPick(shown[active]);
          }}
        />
      </div>
      <div className="picker" role="listbox" aria-label="Tickets">
        {shown.map((t, i) => (
          <button
            key={t.id}
            type="button"
            role="option"
            aria-selected={i === active}
            data-active={i === active}
            className="picker__item"
            disabled={busy}
            onMouseEnter={() => setActive(i)}
            onClick={() => onPick(t)}
          >
            <span style={{ display: "contents" }}>
              <span className="code">{t.label}</span>
              <span className="truncate">{t.name}</span>
            </span>
            <span className="hint">
              {t.sessionState === "paused" ? "En pausa" : bucketLabel[t.bucket]} · {t.status}
              {t.workedMs ? ` · ${duration(t.workedMs)}` : ""}
            </span>
          </button>
        ))}
        {shown.length === 0 && (
          <p className="hint" style={{ padding: "16px 8px" }}>
            {q ? "Ningún ticket coincide." : "No tienes tickets abiertos asignados."}
          </p>
        )}
      </div>
      <div className="row">
        <span className="hint" style={{ flex: 1 }}>
          <span className="kbd">↑</span> <span className="kbd">↓</span> <span className="kbd">Enter</span>
        </span>
        <Button variant="ghost" size="sm" onClick={onCancel}>
          Cancelar
        </Button>
      </div>
    </>
  );
}
