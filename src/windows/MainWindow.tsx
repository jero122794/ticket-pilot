import { useEffect, useState } from "react";
import { CircleAlert, RefreshCw } from "lucide-react";
import { listen } from "@tauri-apps/api/event";

import { api, asAppError, useSnapshot } from "../lib/api";
import type { Snapshot } from "../lib/api";
import { duration, relativeTime } from "../lib/format";
import { Button, IconButton, Spinner, useToast } from "../ui";
import TicketsView from "../components/TicketsView";
import PendingView from "../components/PendingView";
import SyncView from "../components/SyncView";
import SettingsView from "../components/SettingsView";
import FillFieldsDialog from "../components/FillFieldsDialog";

export type View = "tickets" | "pending" | "sync" | "settings";
export type Act = (fn: () => Promise<Snapshot>) => Promise<boolean>;

const VIEWS: { id: View; label: string; cmd: string }[] = [
  { id: "tickets", label: "mis tickets", cmd: "tickets --asignados-a mí" },
  { id: "pending", label: "pendientes", cmd: "campos --pendientes" },
  { id: "sync", label: "sincronización", cmd: "sync --estado" },
  { id: "settings", label: "ajustes", cmd: "config --editar" },
];

/** Usuario corto para el indicador de comandos: "Jerónimo Aristizábal" → "jeronimo". */
function shortUser(name?: string): string {
  return (name ?? "usuario").split(/\s+/)[0].normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
}

export default function MainWindow() {
  const [snap, setSnap] = useSnapshot();
  const [view, setView] = useState<View>("tickets");
  /** Formulario de campos abierto; `finish` = completar para poder finalizar. */
  const [fill, setFill] = useState<{ id: string; finish: boolean } | null>(null);
  const openFill = (id: string, finish = false) => setFill({ id, finish });
  const { notify, view: toast } = useToast();

  // Rust pide cambiar de vista (bandeja, widget, recordatorios). "fill:<id>" abre el formulario.
  useEffect(() => {
    const un = listen<string>("navigate", (e) => {
      if (e.payload.startsWith("fill:")) {
        setView("pending");
        setFill({ id: e.payload.slice(5), finish: false });
      } else if (e.payload.startsWith("finish:")) {
        setView("tickets");
        setFill({ id: e.payload.slice(7), finish: true });
      } else setView(e.payload as View);
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  // Atajos 1–4 para cambiar de sección (fuera de campos de texto).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement;
      if (e.ctrlKey || e.metaKey || e.altKey || t.closest("input, textarea, [role=dialog], [role=listbox]")) return;
      const i = Number(e.key) - 1;
      if (i >= 0 && i < VIEWS.length) setView(VIEWS[i].id);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    if (snap && !snap.configured) setView("settings");
  }, [snap?.configured]); // eslint-disable-line react-hooks/exhaustive-deps

  const act: Act = async (fn) => {
    try {
      setSnap(await fn());
      return true;
    } catch (e) {
      notify(asAppError(e).message);
      return false;
    }
  };

  if (!snap) {
    return (
      <div style={{ display: "grid", placeItems: "center", height: "100%" }}>
        <Spinner />
      </div>
    );
  }

  const problems = snap.failures.length > 0 || !!snap.lastError;
  const cmd = VIEWS.find((v) => v.id === view)!.cmd;

  return (
    <div className="shell">
      <header className="header">
        <div className="prompt">
          <span>
            <span className="prompt__host">{shortUser(snap.user?.username)}@ticketpilot</span>:~$
          </span>
          {/* key = cmd: al cambiar de sección el comando se vuelve a escribir. */}
          <span key={cmd} className="prompt__cmd prompt__cmd--typing cursor" style={{ ["--chars" as string]: cmd.length + 1 }}>
            {cmd}
          </span>
          <div className="header__aside">
            <span className="today">
              hoy <b>{duration(snap.todayWorkedMs)}</b>
            </span>
            <IconButton
              size="sm"
              label={snap.syncing ? "Sincronizando…" : `Sincronizar ahora (última: ${relativeTime(snap.lastSync)})`}
              disabled={!snap.configured || snap.syncing}
              onClick={() => act(api.refresh)}
            >
              <RefreshCw className={snap.syncing ? "spin" : undefined} />
            </IconButton>
            <Button size="sm" onClick={() => api.toggleWidget()}>
              cronómetro flotante
            </Button>
          </div>
        </div>
        <nav className="tabs" aria-label="Secciones">
          {VIEWS.map((v, i) => (
            <button key={v.id} type="button" className="tab" aria-current={view === v.id ? "page" : undefined} onClick={() => setView(v.id)}>
              <span className="tab__key">{i + 1}</span>
              {v.label}
              {v.id === "pending" && snap.counts.withMissing > 0 && <span className="tab__count">{snap.counts.withMissing}</span>}
              {v.id === "sync" && problems && <span className="tab__alert" aria-label="con problemas" />}
            </button>
          ))}
        </nav>
      </header>

      <main className="content">
        <div className="content__inner">
          {snap.lastError && view !== "settings" && (
            <div className="banner" role="alert">
              <CircleAlert />
              <span style={{ flex: 1 }}>
                {snap.lastError} Lo que marques se guarda en el equipo y se envía cuando vuelva la conexión.
              </span>
              <Button size="sm" onClick={() => act(api.refresh)}>
                Reintentar
              </Button>
            </div>
          )}
          {view === "tickets" && <TicketsView snap={snap} act={act} onFill={openFill} />}
          {view === "pending" && <PendingView snap={snap} onFill={openFill} />}
          {view === "sync" && <SyncView snap={snap} act={act} />}
          {view === "settings" && <SettingsView snap={snap} notify={notify} />}
        </div>
      </main>

      <FillFieldsDialog taskId={fill?.id ?? null} finish={!!fill?.finish} onClose={() => setFill(null)} notify={notify} />
      {toast}
    </div>
  );
}
