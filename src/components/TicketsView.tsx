import { useMemo, useRef, useState } from "react";
import { ArrowUpRight, Link2, Pause, Play, Search, Square, X } from "lucide-react";

import { api, asAppError, liveWorked, useNow } from "../lib/api";
import type { Bucket, Current, Punch, Snapshot, Ticket } from "../lib/api";
import { clock, duration, hhmm, shortDate } from "../lib/format";
import { Button, Clock, Dialog, Field, IconButton, Input, Textarea } from "../ui";
import type { Act } from "../windows/MainWindow";

interface Props {
  snap: Snapshot;
  act: Act;
  onFill: (taskId: string, finish?: boolean) => void;
}

type Filter = Bucket | "open";

const FILTERS: { key: Filter; label: string }[] = [
  { key: "open", label: "abiertos" },
  { key: "inProgress", label: "en curso" },
  { key: "notStarted", label: "por iniciar" },
  { key: "reopened", label: "reabiertos" },
  { key: "done", label: "finalizados 30d" },
];

const PUNCH_LABEL: Record<Punch["kind"], string> = {
  start: "inicio",
  resume: "reanuda",
  pause: "pausa",
  stop: "fin",
};

type StopTarget = { id: string; label: string; url: string };

export default function TicketsView({ snap, act, onFill }: Props) {
  const [filter, setFilter] = useState<Filter>("open");
  const [q, setQ] = useState("");
  const [pauseOpen, setPauseOpen] = useState(false);
  const [stopTarget, setStopTarget] = useState<StopTarget | null>(null);
  const [addOpen, setAddOpen] = useState(false);

  const count = (f: Filter) =>
    f === "open" ? snap.counts.notStarted + snap.counts.inProgress + snap.counts.reopened : snap.counts[f];

  const rows = useMemo(() => {
    const needle = q.trim().toLowerCase();
    return snap.tickets
      .filter((t) => (filter === "open" ? t.bucket !== "done" : t.bucket === filter))
      .filter((t) => !needle || `${t.label} ${t.name} ${t.status}`.toLowerCase().includes(needle));
  }, [snap.tickets, filter, q]);

  return (
    <>
      <NowPanel snap={snap} act={act} onPause={() => setPauseOpen(true)} onStop={setStopTarget} onFill={onFill} />

      <div className="stack" style={{ gap: 10 }}>
        <div className="listbar">
          <div className="tabs" role="group" aria-label="Filtrar por estado">
            {FILTERS.map((f) => (
              <button key={f.key} type="button" className="tab" aria-pressed={filter === f.key} onClick={() => setFilter(f.key)}>
                {f.label} <span className="tab__count">{count(f.key)}</span>
              </button>
            ))}
          </div>
          <div className="input-icon">
            <Search />
            <Input aria-label="Buscar ticket" placeholder="grep código, nombre o estado" value={q} onChange={(e) => setQ(e.target.value)} />
          </div>
          <Button size="sm" onClick={() => setAddOpen(true)}>
            <Link2 /> agregar por enlace
          </Button>
        </div>

        <div role="table" aria-label="Tickets asignados">
          <div className="list__head" role="row">
            <span role="columnheader" aria-label="Estado del cronómetro" />
            <span role="columnheader" className="code" style={{ color: "inherit", fontWeight: 400 }}>
              código
            </span>
            <span role="columnheader">ticket</span>
            <span role="columnheader" className="col-status">
              estado
            </span>
            <span role="columnheader" style={{ textAlign: "right" }}>
              tiempo
            </span>
            <span role="columnheader" className="sr-only">
              acciones
            </span>
          </div>
          {rows.map((t) => (
            <Row key={t.id} t={t} act={act} onPause={() => setPauseOpen(true)} onStop={setStopTarget} onFill={onFill} />
          ))}
          {rows.length === 0 && (
            <div className="empty">
              <p className="empty__title">
                {!snap.configured ? "sin conexión con ClickUp" : q ? `0 resultados para «${q}»` : "nada en esta lista"}
              </p>
              <p>{!snap.configured ? "Conecta tu token en la sección 4 (ajustes)." : "Prueba otro filtro o borra la búsqueda."}</p>
            </div>
          )}
        </div>
      </div>

      <AddDialog open={addOpen} onClose={() => setAddOpen(false)} act={act} />
      <PauseDialog open={pauseOpen} onClose={() => setPauseOpen(false)} act={act} />
      <StopDialog target={stopTarget} onClose={() => setStopTarget(null)} act={act} onFill={onFill} />
    </>
  );
}

// ---------------------------------------------------------------- panel "ahora"

function NowPanel({
  snap,
  act,
  onPause,
  onStop,
  onFill,
}: {
  snap: Snapshot;
  act: Act;
  onPause: () => void;
  onStop: (t: StopTarget) => void;
  onFill: (id: string, finish?: boolean) => void;
}) {
  const cur = snap.current;
  const now = useNow(cur?.state === "running");

  if (!cur) {
    const next = snap.tickets.find((t) => t.bucket !== "done");
    return (
      <section className="panel timecard" aria-label="Ticket activo">
        <span className="panel__title">ahora</span>
        <div className="timecard__main">
          <span className="timecard__status">ningún cronómetro corriendo</span>
          <Clock className="clock" state="idle" text="00:00:00" />
          {next && (
            <div className="timecard__actions">
              <Button variant="primary" size="lg" onClick={() => act(() => api.play(next.id))}>
                <Play fill="currentColor" /> iniciar {next.label}
              </Button>
              <span className="muted truncate">{next.name}</span>
            </div>
          )}
        </div>
        <div className="punches">
          <span className="punches__title">-- marcas --</span>
          <span className="muted">Las horas de inicio, pausa y fin aparecen aquí.</span>
        </div>
      </section>
    );
  }

  const running = cur.state === "running";
  const last = cur.punches[cur.punches.length - 1];
  const since = last ? hhmm(last.at) : null;

  return (
    <section className="panel timecard" aria-label="Ticket activo">
      <span className="panel__title">
        {running ? "ahora" : "en pausa"} <b>{cur.label}</b>
      </span>
      <div className="timecard__main">
        <h2 className="timecard__title">{cur.name}</h2>
        <Clock className="clock" state={cur.state} text={clock(liveWorked(cur, now))} />
        <span className="timecard__status">
          {running ? `corriendo desde las ${since ?? "--:--"}` : `en pausa desde las ${since ?? "--:--"}`}
          {cur.pauseCount > 0 && `, ${cur.pauseCount} ${cur.pauseCount === 1 ? "pausa" : "pausas"}`}
        </span>
        <div className="timecard__actions">
          {running ? (
            <Button variant="primary" size="lg" onClick={onPause}>
              <Pause fill="currentColor" /> pausar
            </Button>
          ) : (
            <Button variant="primary" size="lg" onClick={() => act(() => api.play(cur.taskId))}>
              <Play fill="currentColor" /> reanudar
            </Button>
          )}
          <Button size="lg" onClick={() => onStop({ id: cur.taskId, label: cur.label, url: cur.url })}>
            <Square /> finalizar
          </Button>
          {cur.missingFields.length > 0 && (
            <Button size="lg" variant="ghost" onClick={() => onFill(cur.taskId)}>
              completar {cur.missingFields.length} campos
            </Button>
          )}
          <Button size="lg" variant="ghost" onClick={() => api.openUrl(cur.url)}>
            <ArrowUpRight /> clickup
          </Button>
        </div>
      </div>
      <PunchLog cur={cur} />
    </section>
  );
}

/** Log de marcas. Solo la marca recién agregada se "imprime" (no todas al montar). */
function PunchLog({ cur }: { cur: Current }) {
  const seen = useRef<Set<number> | null>(null);
  const fresh = new Set<number>();
  if (seen.current === null) {
    seen.current = new Set(cur.punches.map((p) => p.at));
  } else {
    for (const p of cur.punches) if (!seen.current.has(p.at)) fresh.add(p.at);
    fresh.forEach((at) => seen.current!.add(at));
  }
  const today = new Date().toDateString();

  return (
    <div className="punches" aria-label="Marcas de hora">
      <span className="punches__title">-- marcas --</span>
      {cur.punches.map((p) => (
        <div key={`${p.at}-${p.kind}`} className={`punch${fresh.has(p.at) ? " punch--new" : ""}`}>
          <span className="punch__time">{hhmm(p.at)}</span>
          <span className="punch__what">
            {PUNCH_LABEL[p.kind]}
            {new Date(p.at).toDateString() !== today && <span className="muted"> ({shortDate(p.at)})</span>}
            {p.note && <span className="punch__note">{p.note}</span>}
          </span>
        </div>
      ))}
      {cur.state === "running" && (
        <div className="punch punch--now">
          <span className="punch__time">ahora</span>
          <span className="punch__what cursor" />
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------- fila

function Row({
  t,
  act,
  onPause,
  onStop,
  onFill,
}: {
  t: Ticket;
  act: Act;
  onPause: () => void;
  onStop: (t: StopTarget) => void;
  onFill: (id: string, finish?: boolean) => void;
}) {
  return (
    <div className="ticket" role="row" data-active={t.sessionState === "running"}>
      <span role="cell" className="ticket__mark">
        {t.sessionState && <span className="status-dot" data-state={t.sessionState} aria-label={t.sessionState === "running" ? "corriendo" : "en pausa"} />}
      </span>
      <span role="cell" className="code">
        {t.label}
      </span>
      <div role="cell" style={{ minWidth: 0 }}>
        <div className="ticket__name truncate" title={t.name}>
          {t.name}
          {t.manual && (
            <span className="muted" title="Compartido contigo; agregado por enlace">
              {" "}
              [compartido]
            </span>
          )}
        </div>
        {t.missingFields.length > 0 && (
          <button type="button" className="ticket__missing" title={t.missingFields.join(", ")} onClick={() => onFill(t.id)}>
            faltan {t.missingFields.length} {t.missingFields.length === 1 ? "campo" : "campos"}
          </button>
        )}
      </div>
      <span role="cell" className="ticket__status truncate col-status" data-bucket={t.bucket} title={t.dueDate ? `${t.status}, entrega ${shortDate(t.dueDate)}` : t.status}>
        {t.status}
      </span>
      <span role="cell" className="num">
        {t.workedMs ? duration(t.workedMs) : <span style={{ color: "var(--faint)" }}>--</span>}
      </span>
      <div role="cell" className="ticket__actions">
        {t.sessionState === "running" ? (
          <IconButton size="sm" label={`Pausar ${t.label}`} onClick={onPause}>
            <Pause />
          </IconButton>
        ) : (
          t.bucket !== "done" && (
            <IconButton
              size="sm"
              label={t.sessionState === "paused" ? `Reanudar ${t.label}` : `Iniciar ${t.label}`}
              onClick={() => act(() => api.play(t.id))}
            >
              <Play />
            </IconButton>
          )
        )}
        {t.sessionState && (
          <IconButton size="sm" label={`Finalizar ${t.label}`} onClick={() => onStop({ id: t.id, label: t.label, url: t.url })}>
            <Square />
          </IconButton>
        )}
        <IconButton size="sm" label="Abrir en ClickUp" onClick={() => api.openUrl(t.url)}>
          <ArrowUpRight />
        </IconButton>
        {t.manual && !t.sessionState && (
          <IconButton
            size="sm"
            label={`Dejar de seguir ${t.label}`}
            onClick={() => act(async () => (await api.removeManualTask(t.id), api.snapshot()))}
          >
            <X />
          </IconButton>
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------- agregar por enlace

function AddDialog({ open, onClose, act }: { open: boolean; onClose: () => void; act: Act }) {
  const [ref, setRef] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const close = () => {
    setRef("");
    setError(null);
    onClose();
  };

  async function add() {
    setBusy(true);
    setError(null);
    try {
      await api.addTaskByRef(ref);
      await act(api.snapshot);
      close();
    } catch (e) {
      setError(asAppError(e).message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(v) => !v && close()}
      title="agregar por enlace"
      description="Para tickets que te comparten sueltos («Shared with me»): ClickUp no los incluye en tus asignados. La app los seguirá en cada recarga y los quitará 30 días después de cerrados."
      footer={
        <>
          <Button variant="ghost" onClick={close}>
            cancelar
          </Button>
          <Button variant="primary" loading={busy} disabled={!ref.trim()} onClick={add}>
            agregar
          </Button>
        </>
      }
    >
      <Field label="Enlace o código" htmlFor="ref" error={error} hint="Ej. https://app.clickup.com/t/90133100517/TKM-20742 o TKM-20742">
        <Input
          id="ref"
          autoFocus
          value={ref}
          aria-invalid={!!error}
          onChange={(e) => setRef(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && ref.trim() && add()}
        />
      </Field>
    </Dialog>
  );
}

// ---------------------------------------------------------------- diálogos

function PauseDialog({ open, onClose, act }: { open: boolean; onClose: () => void; act: Act }) {
  const [reason, setReason] = useState("");
  const [busy, setBusy] = useState(false);
  async function confirm() {
    setBusy(true);
    if (await act(() => api.pause(reason))) {
      setReason("");
      onClose();
    }
    setBusy(false);
  }
  return (
    <Dialog
      open={open}
      onOpenChange={(v) => !v && onClose()}
      title="pausar"
      description="El motivo se agrega a «Motivo de Pausa» en ClickUp."
      footer={
        <>
          <span className="hint" style={{ marginRight: "auto" }}>
            <span className="kbd">ctrl</span>+<span className="kbd">enter</span>
          </span>
          <Button variant="ghost" onClick={onClose}>
            cancelar
          </Button>
          <Button variant="primary" loading={busy} onClick={confirm}>
            pausar
          </Button>
        </>
      }
    >
      <Textarea
        autoFocus
        aria-label="Motivo de la pausa"
        rows={4}
        placeholder="ej. me reasignaron un ticket urgente"
        value={reason}
        onChange={(e) => setReason(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && (e.ctrlKey || e.metaKey) && confirm()}
      />
    </Dialog>
  );
}

function StopDialog({
  target,
  onClose,
  act,
  onFill,
}: {
  target: StopTarget | null;
  onClose: () => void;
  act: Act;
  onFill: (id: string, finish?: boolean) => void;
}) {
  const [busy, setBusy] = useState(false);

  async function stop() {
    if (!target) return;
    setBusy(true);
    try {
      await api.stop(target.id);
      await act(api.snapshot);
      onClose();
    } catch (e) {
      const err = asAppError(e);
      onClose();
      // Faltan campos: no se puede finalizar sin llenarlos; se abre el formulario.
      if (err.code === "missing_fields") onFill(target.id, true);
      else await act(() => Promise.reject(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog
      open={!!target}
      onOpenChange={(v) => !v && onClose()}
      title={`finalizar ${target?.label ?? ""}`}
      description="Se detiene el cronómetro, se registra la fecha de fin y el ticket pasa al estado de finalizado en ClickUp. Si faltan campos obligatorios, primero tendrás que llenarlos."
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            seguir trabajando
          </Button>
          <Button variant="destructive" loading={busy} onClick={stop}>
            finalizar
          </Button>
        </>
      }
    />
  );
}
