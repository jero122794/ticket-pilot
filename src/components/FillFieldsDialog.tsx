import { useEffect, useState } from "react";
import { ArrowUpRight, Ban } from "lucide-react";

import { api, asAppError } from "../lib/api";
import type { EditableField, FieldInput, FillForm, GitHint } from "../lib/api";
import { Button, Checkbox, Dialog, Field, Input, Select, Spinner } from "../ui";
import { EvidenceEditor } from "./Evidence";

type Value = string | boolean | string[];

interface Props {
  taskId: string | null;
  /** Abierto al finalizar: hay que llenar todo y, al guardar, el ticket se finaliza. */
  finish?: boolean;
  onClose: () => void;
  notify: (msg: string) => void;
}

/** Ms → "2026-11-23T08:30" para <input type="datetime-local">. */
function toLocalInput(ms: number): string {
  return new Date(ms - new Date(ms).getTimezoneOffset() * 60000).toISOString().slice(0, 16);
}

/** Campos que se pueden llenar con la rama o el merge request del repositorio local. */
const gitKind = (name: string): "branch" | "mr" | null =>
  /merge request|\bmr\b|pull request/i.test(name) ? "mr" : /branch|rama/i.test(name) ? "branch" : null;

const isEmpty = (v: Value | undefined) => v === undefined || v === "" || (Array.isArray(v) && v.length === 0);

export default function FillFieldsDialog({ taskId, finish = false, onClose, notify }: Props) {
  const [form, setForm] = useState<FillForm | null>(null);
  const [values, setValues] = useState<Record<string, Value>>({});
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [hints, setHints] = useState<GitHint[]>([]);

  const load = (id: string) =>
    api
      .fillForm(id)
      .then(setForm)
      .catch((e) => setError(asAppError(e).message));

  useEffect(() => {
    setForm(null);
    setValues({});
    setError(null);
    if (taskId) load(taskId);
  }, [taskId]);

  const set = (id: string, v: Value) => setValues((prev) => ({ ...prev, [id]: v }));

  // Rama y MR: se leen de los repositorios configurados en Ajustes.
  useEffect(() => {
    setHints([]);
    if (form && form.fields.some((f) => gitKind(f.name))) api.gitSuggestions(form.label).then(setHints).catch(() => {});
  }, [form?.taskId]); // eslint-disable-line react-hooks/exhaustive-deps

  function gitChips(f: EditableField) {
    const kind = gitKind(f.name);
    if (!kind || hints.length === 0) return null;
    return (
      <div className="chips" role="group" aria-label={`Sugerencias para ${f.name}`} style={{ marginTop: 6 }}>
        {hints.slice(0, 4).map((h) => {
          const value = kind === "mr" ? h.mrUrl : h.branchUrl ?? h.branch;
          if (!value) return null;
          return (
            <button key={`${h.repo}-${h.branch}`} type="button" className="chip" onClick={() => set(f.id, value)} title={value}>
              {h.matches ? "✓ " : ""}
              {kind === "mr" ? "MR de " : "rama "}
              {h.repo}:{h.branch}
            </button>
          );
        })}
      </div>
    );
  }

  async function notMine(f: EditableField) {
    try {
      await api.setFieldApplies(f.name, false);
      setForm((prev) => prev && { ...prev, fields: prev.fields.filter((x) => x.id !== f.id) });
      notify(`«${f.name}» ya no se te pedirá. Puedes reactivarlo en Ajustes.`);
    } catch (e) {
      setError(asAppError(e).message);
    }
  }

  async function save() {
    if (!form) return;
    const inputs: FieldInput[] = form.fields
      .filter((f) => f.supported && !isEmpty(values[f.id]))
      .map((f) => {
        const v = values[f.id];
        let value: FieldInput["value"] = v;
        if (f.kind === "date") value = new Date(v as string).getTime();
        if (f.kind === "number" || f.kind === "currency" || f.kind === "users") value = Number(v);
        return { fieldId: f.id, name: f.name, kind: f.kind, value };
      });
    setSaving(true);
    setError(null);
    try {
      if (inputs.length) await api.saveFields(form.taskId, inputs);
      if (finish) {
        await api.stop(form.taskId);
        notify(`${form.label} finalizado.`);
      } else {
        notify(`${inputs.length} ${inputs.length === 1 ? "campo guardado" : "campos guardados"} en ${form.label}.`);
      }
      onClose();
    } catch (e) {
      const err = asAppError(e);
      if (err.code === "missing_fields") {
        // Aún falta algo (p. ej. un campo que solo se llena en ClickUp): se recarga el formulario.
        setValues({});
        setError(`Todavía faltan: ${err.fields.join(", ")}.`);
        await load(form.taskId);
      } else setError(err.message);
    } finally {
      setSaving(false);
    }
  }

  const filled = form ? form.fields.filter((f) => f.supported && !isEmpty(values[f.id])).length : 0;
  const unsupported = form ? form.fields.filter((f) => !f.supported) : [];
  const total = form ? form.fields.length : 0;
  // Al finalizar, todo es obligatorio: los editables llenos y ninguno pendiente en ClickUp.
  const ready = finish ? !!form && filled === total - unsupported.length && unsupported.length === 0 : filled > 0;

  function control(f: EditableField) {
    const id = `f-${f.id}`;
    const v = values[f.id];
    switch (f.kind) {
      case "text":
        return form ? (
          <EvidenceEditor id={id} taskId={form.taskId} rows={3} value={(v as string) ?? ""} onChange={(x) => set(f.id, x)} />
        ) : null;
      case "number":
      case "currency":
        return <Input id={id} type="number" inputMode="decimal" value={(v as string) ?? ""} onChange={(e) => set(f.id, e.target.value)} />;
      case "date":
        return (
          <div className="row">
            <Input id={id} type="datetime-local" value={(v as string) ?? ""} onChange={(e) => set(f.id, e.target.value)} />
            <Button variant="outline" onClick={() => set(f.id, toLocalInput(Date.now()))}>
              Ahora
            </Button>
          </div>
        );
      case "drop_down":
        return (
          <Select
            id={id}
            value={(v as string) ?? ""}
            onChange={(x) => set(f.id, x)}
            options={f.options.map((o) => ({ value: o.id, label: o.name }))}
          />
        );
      case "labels": {
        const sel = (v as string[]) ?? [];
        return (
          <div className="chips" id={id} role="group" aria-label={f.name}>
            {f.options.map((o) => (
              <button
                key={o.id}
                type="button"
                className="chip"
                aria-pressed={sel.includes(o.id)}
                onClick={() => set(f.id, sel.includes(o.id) ? sel.filter((x) => x !== o.id) : [...sel, o.id])}
              >
                {o.name}
              </button>
            ))}
          </div>
        );
      }
      case "checkbox":
        return (
          <label className="check-row">
            <Checkbox id={id} checked={!!v} onChange={(x) => set(f.id, x)} /> Sí
          </label>
        );
      case "users":
        return (
          <Select
            id={id}
            value={(v as string) ?? ""}
            onChange={(x) => set(f.id, x)}
            placeholder="Elige una persona"
            options={[
              ...(form?.me ? [{ value: String(form.me), label: "Yo" }] : []),
              ...(form?.members ?? []).filter((m) => m.id !== String(form?.me)).map((m) => ({ value: m.id, label: m.name })),
            ]}
          />
        );
      default:
        return f.supported ? (
          <Input id={id} value={(v as string) ?? ""} onChange={(e) => set(f.id, e.target.value)} />
        ) : (
          <span className="hint">Este tipo de campo se completa en ClickUp.</span>
        );
    }
  }

  return (
    <Dialog
      wide
      open={!!taskId}
      onOpenChange={(o) => !o && onClose()}
      title={finish ? "completa para finalizar" : "completar campos"}
      description={
        !form
          ? "Cargando el ticket desde ClickUp…"
          : finish
            ? `${form.label} ${form.name}. Llena los campos obligatorios para poder finalizarlo.`
            : `${form.label} ${form.name}`
      }
      footer={
        <>
          {form && (
            <Button variant="ghost" onClick={() => api.openUrl(form.url)} style={{ marginRight: "auto" }}>
              <ArrowUpRight /> Abrir en ClickUp
            </Button>
          )}
          {finish && unsupported.length > 0 && (
            <Button onClick={() => form && (setError(null), load(form.taskId))}>ya los llené, revisar</Button>
          )}
          <Button variant="outline" onClick={onClose}>
            {finish ? "seguir trabajando" : "cancelar"}
          </Button>
          <Button variant="primary" loading={saving} disabled={!ready} onClick={save}>
            {finish
              ? total === 0
                ? "finalizar"
                : `guardar y finalizar (${filled}/${total})`
              : filled
                ? `guardar ${filled} ${filled === 1 ? "campo" : "campos"}`
                : "guardar"}
          </Button>
        </>
      }
    >
      <div className="stack" style={{ gap: 20 }}>
        {error && (
          <div className="mini-alert" role="alert">
            {error}
          </div>
        )}
        {!form && !error && (
          <div style={{ display: "grid", placeItems: "center", padding: 32 }}>
            <Spinner />
          </div>
        )}
        {form && form.fields.length === 0 && (
          <p className="muted">
            {finish ? "Todos los campos obligatorios están llenos. Ya puedes finalizar." : "Este ticket no tiene campos pendientes que te apliquen."}
          </p>
        )}
        {finish && unsupported.length > 0 && (
          <p className="hint">
            {unsupported.map((f) => f.name).join(", ")} solo se {unsupported.length === 1 ? "llena" : "llenan"} en ClickUp. Ábrelo,
            complétalo y pulsa «ya los llené, revisar».
          </p>
        )}
        {form?.fields.map((f) => (
          <div key={f.id} className="row" style={{ alignItems: "flex-start", gap: 12 }}>
            <div style={{ flex: 1, minWidth: 0 }}>
              <Field label={f.name} htmlFor={`f-${f.id}`}>
                {control(f)}
              </Field>
              {gitChips(f)}
            </div>
            <Button variant="ghost" size="sm" style={{ marginTop: 22, color: "var(--muted-foreground)" }} onClick={() => notMine(f)}>
              <Ban /> No me aplica
            </Button>
          </div>
        ))}
      </div>
    </Dialog>
  );
}
