import { useEffect, useState } from "react";
import { ArrowUpRight, ChevronLeft, ChevronRight, RefreshCw } from "lucide-react";

import { api, asAppError } from "../lib/api";
import type { Period, Report } from "../lib/api";
import { duration, hhmm, plural, shortDate } from "../lib/format";
import { Button, IconButton, Spinner } from "../ui";

const PERIODS: { key: Period; label: string }[] = [
  { key: "day", label: "día" },
  { key: "week", label: "semana" },
  { key: "month", label: "mes" },
];

const weekday = new Intl.DateTimeFormat("es-CO", { weekday: "short" });
const longDay = new Intl.DateTimeFormat("es-CO", { weekday: "long", day: "numeric", month: "long" });
const monthYear = new Intl.DateTimeFormat("es-CO", { month: "long", year: "numeric" });

function periodLabel(r: Report): string {
  if (r.period === "day") return longDay.format(new Date(r.from));
  if (r.period === "month") return monthYear.format(new Date(r.from));
  return `${shortDate(r.from)} a ${shortDate(r.to - 1)}`;
}

const CURRENT: Record<Period, string> = { day: "hoy", week: "esta semana", month: "este mes" };

/** Tiempo trabajado y tickets ejecutados, consultados a ClickUp (no se guardan en el equipo). */
export default function ReportsView() {
  const [period, setPeriod] = useState<Period>("week");
  const [offset, setOffset] = useState(0);
  const [report, setReport] = useState<Report | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [tick, setTick] = useState(0);

  useEffect(() => {
    let alive = true;
    setLoading(true);
    setError(null);
    api
      .report(period, offset)
      .then((r) => alive && setReport(r))
      .catch((e) => alive && setError(asAppError(e).message))
      .finally(() => alive && setLoading(false));
    return () => {
      alive = false;
    };
  }, [period, offset, tick]);

  const pick = (p: Period) => {
    setPeriod(p);
    setOffset(0);
  };

  const r = report && report.period === period ? report : null;
  const max = r ? Math.max(...r.days.map((d) => d.workedMs), 1) : 1;
  const avg = r && r.workedTickets ? r.workedMs / r.workedTickets : 0;
  // Precisión de la estimación: solo con los tickets finalizados que tenían horas estimadas.
  const est = r ? r.tickets.filter((t) => t.finishedAt && t.estimateMs) : [];
  const estTotal = est.reduce((n, t) => n + (t.estimateMs ?? 0), 0);
  const estWorked = est.reduce((n, t) => n + t.workedMs, 0);

  return (
    <div className="stack" style={{ gap: 18 }}>
      <div className="listbar">
        <div className="tabs" role="group" aria-label="Período">
          {PERIODS.map((p) => (
            <button key={p.key} type="button" className="tab" aria-pressed={period === p.key} onClick={() => pick(p.key)}>
              {p.label}
            </button>
          ))}
        </div>
        <div className="report__nav">
          <IconButton size="sm" label="Período anterior" onClick={() => setOffset(offset - 1)}>
            <ChevronLeft />
          </IconButton>
          <span className="report__period">{r ? (offset === 0 ? `${CURRENT[period]}, ${periodLabel(r)}` : periodLabel(r)) : "…"}</span>
          <IconButton size="sm" label="Período siguiente" disabled={offset >= 0} onClick={() => setOffset(offset + 1)}>
            <ChevronRight />
          </IconButton>
          {offset < 0 && (
            <Button size="sm" variant="ghost" onClick={() => setOffset(0)}>
              volver a {CURRENT[period]}
            </Button>
          )}
        </div>
        <IconButton size="sm" label="Volver a consultar ClickUp" disabled={loading} onClick={() => setTick(tick + 1)}>
          <RefreshCw className={loading ? "spin" : undefined} />
        </IconButton>
      </div>

      {error && (
        <div className="banner" role="alert">
          <span style={{ flex: 1 }}>No se pudo consultar ClickUp: {error}</span>
          <Button size="sm" onClick={() => setTick(tick + 1)}>
            Reintentar
          </Button>
        </div>
      )}

      {!r && loading && (
        <div style={{ display: "grid", placeItems: "center", padding: 40 }}>
          <Spinner />
        </div>
      )}

      {r && (
        <>
          <section className="panel report__totals" aria-label="Resumen" data-loading={loading}>
            <span className="panel__title">resumen</span>
            <div className="stat">
              <span className="stat__value">{duration(r.workedMs)}</span>
              <span className="stat__label">tiempo registrado</span>
            </div>
            <div className="stat">
              <span className="stat__value">{r.workedTickets}</span>
              <span className="stat__label">{r.workedTickets === 1 ? "ticket trabajado" : "tickets trabajados"}</span>
            </div>
            <div className="stat">
              <span className="stat__value">{r.finishedTickets}</span>
              <span className="stat__label">{r.finishedTickets === 1 ? "ticket finalizado" : "tickets finalizados"}</span>
            </div>
            <div className="stat">
              <span className="stat__value">{avg ? duration(avg) : "—"}</span>
              <span className="stat__label">promedio por ticket</span>
            </div>
            <div className="stat">
              <span className="stat__value">{estTotal ? `${Math.round((estWorked / estTotal) * 100)} %` : "—"}</span>
              <span className="stat__label">
                {estTotal ? `del estimado en finalizados (${duration(estWorked)} de ${duration(estTotal)})` : "sin finalizados con estimación"}
              </span>
            </div>
          </section>

          {r.period !== "day" && (
            <section className="panel report__days" aria-label="Tiempo por día">
              <span className="panel__title">por día</span>
              {r.days.map((d) => (
                <div key={d.at} className="day" data-empty={d.workedMs === 0}>
                  <span className="day__date">
                    {weekday.format(new Date(d.at)).replace(".", "")} {new Date(d.at).getDate()}
                  </span>
                  <span className="day__track" aria-hidden>
                    <span className="day__bar" style={{ width: `${(d.workedMs / max) * 100}%` }} />
                  </span>
                  <span className="day__value num">{d.workedMs ? duration(d.workedMs) : "—"}</span>
                  <span className="day__tickets num muted">{d.tickets ? plural(d.tickets, "ticket", "tickets") : ""}</span>
                </div>
              ))}
            </section>
          )}

          {r.tickets.length === 0 ? (
            <div className="empty">
              <p className="empty__title">Sin tiempo registrado en este período</p>
              <p>Aquí aparecen los tickets con registros de tiempo en ClickUp o con «Fecha Fin» dentro del período.</p>
            </div>
          ) : (
            <div role="table" aria-label="Tickets del período">
              <div className="report__row list__head" role="row">
                <span className="code">código</span>
                <span>ticket</span>
                <span className="num">tiempo</span>
                <span className="num">estimado</span>
                <span>finalizado</span>
                <span />
              </div>
              {r.tickets.map((t) => (
                <div key={t.taskId} className="report__row ticket" role="row">
                  <span className="code">{t.label}</span>
                  <span className="truncate ticket__name" title={t.name}>
                    {t.name}
                  </span>
                  <span className="num">{t.workedMs ? duration(t.workedMs) : "—"}</span>
                  <span className="num" data-over={!!t.estimateMs && t.workedMs > t.estimateMs}>
                    {t.estimateMs ? duration(t.estimateMs) : "—"}
                  </span>
                  <span className={t.finishedAt ? undefined : "muted"}>
                    {t.finishedAt ? `${shortDate(t.finishedAt)} ${hhmm(t.finishedAt)}` : "en curso"}
                  </span>
                  <span className="ticket__actions">
                    <IconButton size="sm" label={`Abrir ${t.label} en ClickUp`} onClick={() => api.openUrl(t.url)}>
                      <ArrowUpRight />
                    </IconButton>
                  </span>
                </div>
              ))}
            </div>
          )}

          <p className="page-intro">
            Datos tomados de ClickUp: tus registros de tiempo y los tickets asignados con «Fecha Fin» en el período. Lo que
            aún está en cola de sincronización aparece cuando llegue a ClickUp.
          </p>
        </>
      )}
    </div>
  );
}
