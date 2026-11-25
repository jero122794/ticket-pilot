import { useEffect, useState } from "react";
import type { ReactNode } from "react";
import { ChevronDown } from "lucide-react";

import { api, asAppError } from "../lib/api";
import type { FieldNames, Settings, Snapshot, Team, WorkDay } from "../lib/api";
import { Button, Checkbox, Field, Input, Select, SwitchRow, Textarea } from "../ui";

interface Props {
  snap: Snapshot;
  notify: (msg: string) => void;
}

function Section({ title, description, children }: { title: string; description?: ReactNode; children: ReactNode }) {
  return (
    <section className="settings-section">
      <div>
        <h2 className="settings-section__title">{title}</h2>
        {description && <p className="settings-section__desc">{description}</p>}
      </div>
      <div className="stack" style={{ gap: 14 }}>
        {children}
      </div>
    </section>
  );
}

const same = (a: string, b: string) => a.trim().toLowerCase() === b.trim().toLowerCase();

const DAYS = ["lunes", "martes", "miércoles", "jueves", "viernes", "sábado", "domingo"];

export default function SettingsView({ snap, notify }: Props) {
  const [s, setS] = useState<Settings | null>(null);
  const [saved, setSaved] = useState("");
  const [token, setToken] = useState("");
  const [tokenError, setTokenError] = useState<string | null>(null);
  const [connecting, setConnecting] = useState(false);
  const [teams, setTeams] = useState<Team[]>([]);
  const [smtpPassword, setSmtpPassword] = useState("");
  const [hasSmtp, setHasSmtp] = useState(false);
  const [autostart, setAutostart] = useState(false);
  const [sending, setSending] = useState(false);
  const [required, setRequired] = useState<string[]>([]);
  const [advanced, setAdvanced] = useState(false);

  useEffect(() => {
    api.getSettings().then((x) => {
      setS(x);
      setSaved(JSON.stringify(x));
    });
    api.hasSmtpPassword().then(setHasSmtp);
    api.getAutostart().then(setAutostart).catch(() => {});
    api
      .requiredFields()
      .then((r) => setRequired(r.map((x) => x.name)))
      .catch(() => {});
  }, []);

  useEffect(() => {
    if (snap.configured) api.teams().then(setTeams).catch(() => {});
  }, [snap.configured]);

  if (!s) return null;
  const dirty = JSON.stringify(s) !== saved;

  const set = <K extends keyof Settings>(k: K, v: Settings[K]) => setS({ ...s, [k]: v });
  const setEmail = <K extends keyof Settings["email"]>(k: K, v: Settings["email"][K]) => setS({ ...s, email: { ...s.email, [k]: v } });
  const setField = <K extends keyof FieldNames>(k: K, v: FieldNames[K]) => setS({ ...s, fields: { ...s.fields, [k]: v } });
  const setNotify = <K extends keyof Settings["notify"]>(k: K, v: Settings["notify"][K]) => setS({ ...s, notify: { ...s.notify, [k]: v } });
  const setDay = (i: number, patch: Partial<WorkDay>) =>
    setS({ ...s, schedule: s.schedule.map((d, j) => (j === i ? { ...d, ...patch } : d)) });
  const int = (v: string, min: number) => Math.max(min, Number.parseInt(v, 10) || 0);

  async function save() {
    if (!s) return;
    try {
      const next = await api.saveSettings(s);
      setS(next);
      setSaved(JSON.stringify(next));
      notify("Ajustes guardados.");
    } catch (e) {
      notify(asAppError(e).message);
    }
  }

  async function connect() {
    setConnecting(true);
    setTokenError(null);
    try {
      const user = await api.setToken(token);
      setToken("");
      notify(`Conectado como ${user.username}.`);
    } catch (e) {
      setTokenError(asAppError(e).message);
    } finally {
      setConnecting(false);
    }
  }

  async function storeSmtp() {
    if (!smtpPassword) return;
    await api.setSmtpPassword(smtpPassword);
    setSmtpPassword("");
    setHasSmtp(true);
  }

  async function testEmail() {
    setSending(true);
    try {
      if (dirty) await save();
      await storeSmtp();
      await api.testEmail();
      notify(`Resumen enviado a ${s?.email.to}.`);
    } catch (e) {
      notify(asAppError(e).message);
    } finally {
      setSending(false);
    }
  }

  const applies = (name: string) => !s.ignoredFields.some((x) => same(x, name));
  const toggleApplies = (name: string, on: boolean) =>
    set("ignoredFields", on ? s.ignoredFields.filter((x) => !same(x, name)) : [...s.ignoredFields, name]);

  return (
    <div>
      <Section
        title="Cuenta de ClickUp"
        description="El token se guarda en el llavero del sistema operativo, nunca en un archivo."
      >
        {snap.configured ? (
          <div className="setting-row" style={{ paddingTop: 0 }}>
            <div>
              <div style={{ fontWeight: 500 }}>Conectado{snap.user ? ` como ${snap.user.username}` : ""}</div>
              <div className="hint">{snap.user?.email}</div>
            </div>
            <Button variant="outline" onClick={() => api.clearToken()}>
              Desconectar
            </Button>
          </div>
        ) : (
          <Field
            label="Token personal"
            htmlFor="token"
            error={tokenError}
            hint="En ClickUp: avatar › Ajustes › Apps › API Token › Generar. Empieza por «pk_»."
          >
            <div className="row">
              <Input
                id="token"
                type="password"
                autoComplete="off"
                placeholder="pk_…"
                value={token}
                aria-invalid={!!tokenError}
                onChange={(e) => setToken(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && token && connect()}
              />
              <Button variant="primary" loading={connecting} disabled={!token} onClick={connect}>
                Conectar
              </Button>
            </div>
          </Field>
        )}
        {teams.length > 1 && (
          <Field label="Workspace" htmlFor="team">
            <Select id="team" value={s.teamId ?? ""} onChange={(v) => set("teamId", v)} options={teams.map((t) => ({ value: t.id, label: t.name }))} />
          </Field>
        )}
        <SwitchRow
          id="autostart"
          label="Abrir al iniciar sesión"
          description="TicketPilot arranca minimizado en la bandeja."
          checked={autostart}
          onChange={async (v) => {
            try {
              await api.setAutostart(v);
              setAutostart(v);
            } catch (err) {
              notify(asAppError(err).message);
            }
          }}
        />
      </Section>

      <Section title="Play, Pausa y Stop" description="Qué estado toma el ticket en ClickUp en cada acción.">
        <div className="grid-2">
          <Field label="Estado al iniciar" htmlFor="st-in">
            <Input id="st-in" value={s.statuses.inProgress} onChange={(e) => set("statuses", { ...s.statuses, inProgress: e.target.value })} />
          </Field>
          <Field label="Estado al finalizar" htmlFor="st-done">
            <Input id="st-done" value={s.statuses.done} onChange={(e) => set("statuses", { ...s.statuses, done: e.target.value })} />
          </Field>
        </div>
        <SwitchRow id="ask-reason" label="Pedir motivo al pausar" checked={s.askPauseReason} onChange={(v) => set("askPauseReason", v)} />
        <SwitchRow
          id="open-play"
          label="Abrir el ticket en el navegador al dar Play"
          checked={s.openInBrowserOnPlay}
          onChange={(v) => set("openInBrowserOnPlay", v)}
        />
      </Section>

      <Section
        title="Campos que lleno yo"
        description="Desmarca los obligatorios que llena otra área (soporte, QA, analítica). No aparecerán en recordatorios, en el formulario ni al finalizar."
      >
        {required.length === 0 ? (
          <p className="hint">Aparecerán cuando se carguen tus tickets.</p>
        ) : (
          <>
            <div className="row">
              <span className="hint" style={{ flex: 1 }}>
                {required.filter(applies).length} de {required.length} campos te aplican
              </span>
              <Button variant="ghost" size="sm" onClick={() => set("ignoredFields", [])}>
                Marcar todos
              </Button>
            </div>
            <div className="checklist">
              {required.map((name) => (
                <label key={name} className="check-row">
                  <Checkbox checked={applies(name)} onChange={(v) => toggleApplies(name, v)} />
                  <span className={applies(name) ? undefined : "muted"} style={applies(name) ? undefined : { textDecoration: "line-through" }}>
                    {name}
                  </span>
                </label>
              ))}
            </div>
          </>
        )}
      </Section>

      <Section title="Jornada laboral" description="Los recordatorios solo llegan dentro de la jornada. Al llegar la hora de salida el cronómetro se pausa solo.">
        <div className="schedule" role="table" aria-label="Horario por día">
          {s.schedule.map((d, i) => (
            <div key={DAYS[i]} className="schedule__row" role="row" data-off={!d.enabled}>
              <label className="check-row" role="cell">
                <Checkbox checked={d.enabled} onChange={(v) => setDay(i, { enabled: v })} />
                <span>{DAYS[i]}</span>
              </label>
              <Input
                type="time"
                aria-label={`Entrada ${DAYS[i]}`}
                disabled={!d.enabled}
                value={d.start}
                onChange={(e) => setDay(i, { start: e.target.value })}
              />
              <span className="muted" aria-hidden>
                a
              </span>
              <Input
                type="time"
                aria-label={`Salida ${DAYS[i]}`}
                disabled={!d.enabled}
                value={d.end}
                onChange={(e) => setDay(i, { end: e.target.value })}
              />
            </div>
          ))}
        </div>
        <SwitchRow
          id="autoshift"
          label="Pausar al fin de turno"
          description="Pausa solo en la app: no llena Inicio/Fin de pausa ni el motivo en ClickUp. Si reanudas después de la salida, cuenta como tiempo extra y no se vuelve a pausar."
          checked={s.autoPauseShift}
          onChange={(v) => set("autoPauseShift", v)}
        />
      </Section>

      <Section title="Avisos" description="Notificaciones del sistema por cambios que hacen otras personas en ClickUp.">
        <SwitchRow id="n-assigned" label="Ticket nuevo asignado" checked={s.notify.assigned} onChange={(v) => setNotify("assigned", v)} />
        <SwitchRow
          id="n-status"
          label="Cambio de estado"
          description="Reabierto, devuelto por QA, en revisión… (no avisa los que pone la app)."
          checked={s.notify.status}
          onChange={(v) => setNotify("status", v)}
        />
        <SwitchRow
          id="n-comments"
          label="Comentarios nuevos"
          description="De otras personas en tus tickets abiertos."
          checked={s.notify.comments}
          onChange={(v) => setNotify("comments", v)}
        />
        <SwitchRow
          id="n-due"
          label="Vencimientos"
          description={`«${s.fields.slaDue}» y «${s.fields.deliveryDue}».`}
          checked={s.notify.due}
          onChange={(v) => setNotify("due", v)}
        />
        <Field label="Avisar con anticipación (horas)" htmlFor="n-hours">
          <Input
            id="n-hours"
            type="number"
            min={1}
            style={{ maxWidth: 120 }}
            disabled={!s.notify.due}
            value={s.notify.dueHours}
            onChange={(e) => setNotify("dueHours", int(e.target.value, 1))}
          />
        </Field>
      </Section>

      <Section
        title="Repositorios"
        description="Carpetas de tus proyectos con Git. Al llenar los campos de rama y merge request, la app sugiere la rama actual de cada una."
      >
        <Field label="Carpetas (una por línea)" htmlFor="repos" hint="Ejemplo: /home/usuario/proyectos/plataforma">
          <Textarea
            id="repos"
            rows={3}
            value={s.repoPaths.join("\n")}
            onChange={(e) => set("repoPaths", e.target.value.split("\n"))}
          />
        </Field>
      </Section>

      <Section title="Recordatorios" description="Notificación del sistema con los tickets en curso que tienen campos sin llenar.">
        <Field label="Cada (minutos)" htmlFor="rem" hint="0 = desactivados">
          <Input id="rem" type="number" min={0} style={{ maxWidth: 120 }} value={s.reminderMinutes} onChange={(e) => set("reminderMinutes", int(e.target.value, 0))} />
        </Field>
        <SwitchRow
          id="popup"
          label="Traer el panel al frente"
          description="Además de la notificación."
          checked={s.reminderPopup}
          onChange={(v) => set("reminderPopup", v)}
        />
        <Field label="Recargar tickets cada (minutos)" htmlFor="poll">
          <Input id="poll" type="number" min={1} style={{ maxWidth: 120 }} value={s.pollMinutes} onChange={(e) => set("pollMinutes", int(e.target.value, 1))} />
        </Field>
      </Section>

      <Section
        title="Resumen diario por correo"
        description="Sin costo: tu correo corporativo, Gmail u Outlook. En Gmail usa una «contraseña de aplicación» (myaccount.google.com › Seguridad)."
      >
        <SwitchRow id="mail-on" label="Enviar el resumen cada día" checked={s.email.enabled} onChange={(v) => setEmail("enabled", v)} />
        <div className="grid-3">
          <Field label="Servidor SMTP" htmlFor="host">
            <Input id="host" value={s.email.smtpHost} onChange={(e) => setEmail("smtpHost", e.target.value)} />
          </Field>
          <Field label="Seguridad" htmlFor="sec">
            <Select
              id="sec"
              value={s.email.security}
              onChange={(v) => {
                const security = v as "starttls" | "tls";
                setS({ ...s, email: { ...s.email, security, smtpPort: security === "tls" ? 465 : 587 } });
              }}
              options={[
                { value: "starttls", label: "STARTTLS · 587" },
                { value: "tls", label: "TLS · 465" },
              ]}
            />
          </Field>
          <Field label="Puerto" htmlFor="port">
            <Input id="port" type="number" value={s.email.smtpPort} onChange={(e) => setEmail("smtpPort", int(e.target.value, 1))} />
          </Field>
        </div>
        <div className="grid-2">
          <Field label="Usuario" htmlFor="user">
            <Input id="user" autoComplete="off" value={s.email.username} onChange={(e) => setEmail("username", e.target.value)} />
          </Field>
          <Field label="Contraseña de aplicación" htmlFor="pass" hint={hasSmtp ? "Guardada en el llavero." : undefined}>
            <Input
              id="pass"
              type="password"
              autoComplete="new-password"
              placeholder={hasSmtp ? "••••••••" : ""}
              value={smtpPassword}
              onChange={(e) => setSmtpPassword(e.target.value)}
              onBlur={storeSmtp}
            />
          </Field>
          <Field label="Enviar a" htmlFor="to" hint="Varios: sepáralos con coma.">
            <Input id="to" value={s.email.to} onChange={(e) => setEmail("to", e.target.value)} />
          </Field>
          <Field label="Hora de envío" htmlFor="at">
            <Input id="at" type="time" value={s.email.sendAt} onChange={(e) => setEmail("sendAt", e.target.value)} />
          </Field>
        </div>
        <SwitchRow id="weekdays" label="Solo de lunes a viernes" checked={s.email.weekdaysOnly} onChange={(v) => setEmail("weekdaysOnly", v)} />
        <div>
          <Button variant="outline" loading={sending} disabled={!s.email.to} onClick={testEmail}>
            Enviar resumen ahora
          </Button>
        </div>
      </Section>

      <Section title="Avanzado" description="Nombres de los campos en ClickUp. Ya coinciden con «Tickets Activos»; cámbialos solo si alguien los renombra.">
        <div>
          <Button variant="ghost" onClick={() => setAdvanced(!advanced)} aria-expanded={advanced}>
            <ChevronDown style={{ transform: advanced ? "rotate(180deg)" : undefined, transition: "transform 180ms" }} />
            {advanced ? "Ocultar nombres de campos" : "Mostrar nombres de campos"}
          </Button>
        </div>
        {advanced && (
          <>
            <div className="grid-2">
              {(
                [
                  ["startDate", "Fecha de inicio"],
                  ["endDate", "Fecha de fin"],
                  ["pauseReason", "Motivo de pausa"],
                  ["pauseState", "Estado de pausa"],
                  ["reopenedDate", "Fecha de reabierto"],
                  ["reopenedReason", "Motivo de reabierto"],
                  ["estimate", "Estimación en horas"],
                  ["slaDue", "Fecha límite SLA"],
                  ["deliveryDue", "Fecha estimada de entrega"],
                  ["evidence", "Evidencias de pruebas"],
                ] as const
              ).map(([k, label]) => (
                <Field key={k} label={label} htmlFor={`fn-${k}`}>
                  <Input id={`fn-${k}`} value={s.fields[k]} onChange={(e) => setField(k, e.target.value)} />
                </Field>
              ))}
            </div>
            <p className="hint">Desde la cuarta pausa, el inicio y el fin se registran como comentario en el ticket.</p>
            <div className="grid-3">
              {s.fields.pauseStarts.map((name, i) => (
                <Field key={`ps${i}`} label={`Inicio pausa ${i + 1}`} htmlFor={`ps${i}`}>
                  <Input id={`ps${i}`} value={name} onChange={(e) => setField("pauseStarts", s.fields.pauseStarts.map((x, j) => (j === i ? e.target.value : x)))} />
                </Field>
              ))}
              {s.fields.pauseEnds.map((name, i) => (
                <Field key={`pe${i}`} label={`Fin pausa ${i + 1}`} htmlFor={`pe${i}`}>
                  <Input id={`pe${i}`} value={name} onChange={(e) => setField("pauseEnds", s.fields.pauseEnds.map((x, j) => (j === i ? e.target.value : x)))} />
                </Field>
              ))}
            </div>
          </>
        )}
      </Section>

      {dirty && (
        <div className="savebar">
          <span className="hint" style={{ flex: 1 }}>
            Tienes cambios sin guardar.
          </span>
          <Button variant="ghost" onClick={() => setS(JSON.parse(saved))}>
            Descartar
          </Button>
          <Button variant="primary" onClick={save}>
            guardar ajustes
          </Button>
        </div>
      )}
    </div>
  );
}
