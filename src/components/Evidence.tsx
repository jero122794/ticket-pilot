import { useRef, useState } from "react";
import type { ClipboardEvent } from "react";
import { ImagePlus } from "lucide-react";

import { api, asAppError } from "../lib/api";
import { Button, Spinner, Textarea } from "../ui";

const IMG = /https?:\/\/[^\s)\]]+?\.(?:png|jpe?g|gif|webp|bmp)(?:\?[^\s)\]]*)?/gi;

/** Enlaces de imágenes dentro de un texto (así guarda ClickUp los pantallazos pegados). */
export function imageUrls(text: string): string[] {
  return [...new Set(text.match(IMG) ?? [])];
}

/** Archivo → base64 sin el prefijo "data:…;base64,". */
function toBase64(file: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(String(r.result).split(",")[1] ?? "");
    r.onerror = () => reject(r.error);
    r.readAsDataURL(file);
  });
}

/** Miniaturas de los pantallazos de un texto. Si una imagen no carga, queda el enlace. */
export function Thumbs({ text }: { text: string }) {
  const urls = imageUrls(text);
  const [broken, setBroken] = useState<Set<string>>(new Set());
  if (urls.length === 0) return null;
  return (
    <div className="attachments" style={{ marginTop: 8 }}>
      {urls.map((u) => (
        <button key={u} type="button" className="attachment" onClick={() => api.openUrl(u)} title="Abrir imagen">
          {broken.has(u) ? (
            <span className="attachment__ext">imagen</span>
          ) : (
            <img src={u} alt="Evidencia" loading="lazy" onError={() => setBroken((b) => new Set(b).add(u))} />
          )}
          <span className="attachment__name truncate">{decodeURIComponent(u.split("/").pop() ?? "imagen")}</span>
        </button>
      ))}
    </div>
  );
}

/**
 * Texto largo con evidencias: los pantallazos se pegan con Ctrl+V o se adjuntan con
 * el botón. Se suben como adjuntos del ticket y su enlace queda en el texto, igual
 * que al pegarlos en ClickUp. No se guarda nada en el equipo.
 */
export function EvidenceEditor({
  id,
  taskId,
  value,
  onChange,
  rows = 4,
}: {
  id?: string;
  taskId: string;
  value: string;
  onChange: (v: string) => void;
  rows?: number;
}) {
  const [uploading, setUploading] = useState(0);
  const [err, setErr] = useState<string | null>(null);
  const file = useRef<HTMLInputElement>(null);
  const latest = useRef(value);
  latest.current = value;

  async function upload(files: File[]) {
    const images = files.filter((f) => f.type.startsWith("image/") || /\.(png|jpe?g|gif|webp|bmp|pdf)$/i.test(f.name));
    if (images.length === 0) return;
    setErr(null);
    setUploading((n) => n + images.length);
    for (const f of images) {
      try {
        const ext = f.type.split("/")[1] || "png";
        const name = f.name && f.name !== "image.png" ? f.name : `evidencia-${new Date().toISOString().slice(0, 19).replace(/[:T]/g, "-")}.${ext}`;
        const res = await api.uploadEvidence(taskId, name, f.type, await toBase64(f));
        const cur = latest.current;
        const next = `${cur}${cur && !cur.endsWith("\n") ? "\n" : ""}${res.url}\n`;
        latest.current = next;
        onChange(next);
      } catch (e) {
        setErr(`${f.name || "Pantallazo"}: ${asAppError(e).message}`);
      } finally {
        setUploading((n) => n - 1);
      }
    }
  }

  function onPaste(e: ClipboardEvent<HTMLTextAreaElement>) {
    const files = [...e.clipboardData.files];
    if (files.some((f) => f.type.startsWith("image/"))) {
      e.preventDefault();
      upload(files);
    }
  }

  return (
    <div className="evidence">
      <Textarea id={id} rows={rows} value={value} onChange={(e) => onChange(e.target.value)} onPaste={onPaste} />
      <div className="row" style={{ gap: 8, marginTop: 6 }}>
        <Button size="sm" onClick={() => file.current?.click()} disabled={uploading > 0}>
          <ImagePlus /> adjuntar pantallazo
        </Button>
        <span className="hint">o pégalo con Ctrl + V</span>
        {uploading > 0 && (
          <span className="row" style={{ gap: 6 }}>
            <Spinner size={14} /> subiendo {uploading}…
          </span>
        )}
        <input
          ref={file}
          type="file"
          accept="image/*,.pdf"
          multiple
          hidden
          onChange={(e) => {
            upload([...(e.target.files ?? [])]);
            e.target.value = "";
          }}
        />
      </div>
      {err && (
        <span className="hint hint--error" role="alert">
          {err}
        </span>
      )}
      <Thumbs text={value} />
    </div>
  );
}
