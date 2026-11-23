/* Componentes base al estilo shadcn/ui: Radix para el comportamiento y la
   accesibilidad (teclado, foco, ARIA) y CSS nativo para el aspecto. */

import { forwardRef, useCallback, useEffect, useState } from "react";
import type { ButtonHTMLAttributes, InputHTMLAttributes, ReactNode, TextareaHTMLAttributes } from "react";
import * as RDialog from "@radix-ui/react-dialog";
import * as RSelect from "@radix-ui/react-select";
import * as RSwitch from "@radix-ui/react-switch";
import * as RCheckbox from "@radix-ui/react-checkbox";
import * as RTooltip from "@radix-ui/react-tooltip";
import { Check, ChevronDown, LoaderCircle, X } from "lucide-react";

const cx = (...c: (string | false | null | undefined)[]) => c.filter(Boolean).join(" ");

// ---------------------------------------------------------------- Button

type Variant = "primary" | "outline" | "ghost" | "destructive" | "hero" | "hero-ghost";
type Size = "sm" | "md" | "lg";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;
  loading?: boolean;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = "outline", size = "md", loading, className, children, disabled, ...rest },
  ref,
) {
  return (
    <button
      ref={ref}
      type="button"
      className={cx("btn", `btn--${variant}`, size !== "md" && `btn--${size}`, className)}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      {...rest}
    >
      {loading && <LoaderCircle className="spin" aria-hidden />}
      {children}
    </button>
  );
});

/** Botón solo con ícono: siempre lleva nombre accesible y tooltip. */
export function IconButton({
  label,
  shortcut,
  variant = "ghost",
  size = "md",
  className,
  ...rest
}: ButtonProps & { label: string; shortcut?: string }) {
  return (
    <Tip label={label} shortcut={shortcut}>
      <Button variant={variant} size={size} aria-label={label} className={cx("btn--icon", className)} {...rest} />
    </Tip>
  );
}

// ---------------------------------------------------------------- Tooltip

export function TooltipProvider({ children }: { children: ReactNode }) {
  return (
    <RTooltip.Provider delayDuration={350} skipDelayDuration={150}>
      {children}
    </RTooltip.Provider>
  );
}

export function Tip({ label, shortcut, children }: { label: ReactNode; shortcut?: string; children: ReactNode }) {
  return (
    <RTooltip.Root>
      <RTooltip.Trigger asChild>{children}</RTooltip.Trigger>
      <RTooltip.Portal>
        <RTooltip.Content className="tooltip" sideOffset={6} collisionPadding={8}>
          {label}
          {shortcut && <span style={{ marginLeft: 8, opacity: 0.6 }}>{shortcut}</span>}
        </RTooltip.Content>
      </RTooltip.Portal>
    </RTooltip.Root>
  );
}

// ---------------------------------------------------------------- Field / inputs

export function Field({
  label,
  hint,
  error,
  htmlFor,
  children,
}: {
  label?: ReactNode;
  hint?: ReactNode;
  error?: string | null;
  htmlFor?: string;
  children: ReactNode;
}) {
  return (
    <div className="field">
      {label && (
        <label className="label" htmlFor={htmlFor}>
          {label}
        </label>
      )}
      {children}
      {error ? <span className="hint hint--error">{error}</span> : hint && <span className="hint">{hint}</span>}
    </div>
  );
}

export const Input = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(function Input(
  { className, ...rest },
  ref,
) {
  return <input ref={ref} className={cx("input", className)} {...rest} />;
});

export const Textarea = forwardRef<HTMLTextAreaElement, TextareaHTMLAttributes<HTMLTextAreaElement>>(function Textarea(
  { className, ...rest },
  ref,
) {
  return <textarea ref={ref} className={cx("textarea", className)} {...rest} />;
});

// ---------------------------------------------------------------- Select

export interface Option {
  value: string;
  label: string;
}

export function Select({
  id,
  value,
  onChange,
  options,
  placeholder = "Selecciona…",
  ariaLabel,
}: {
  id?: string;
  value: string;
  onChange: (v: string) => void;
  options: Option[];
  placeholder?: string;
  ariaLabel?: string;
}) {
  return (
    <RSelect.Root value={value || undefined} onValueChange={onChange}>
      <RSelect.Trigger id={id} className="select-trigger" aria-label={ariaLabel}>
        <span className="truncate">
          <RSelect.Value placeholder={placeholder} />
        </span>
        <RSelect.Icon>
          <ChevronDown />
        </RSelect.Icon>
      </RSelect.Trigger>
      <RSelect.Portal>
        <RSelect.Content className="select-content" position="popper" sideOffset={4}>
          <RSelect.Viewport>
            {options.map((o) => (
              <RSelect.Item key={o.value} value={o.value} className="select-item">
                <RSelect.ItemIndicator className="select-indicator">
                  <Check />
                </RSelect.ItemIndicator>
                <RSelect.ItemText>{o.label}</RSelect.ItemText>
              </RSelect.Item>
            ))}
          </RSelect.Viewport>
        </RSelect.Content>
      </RSelect.Portal>
    </RSelect.Root>
  );
}

// ---------------------------------------------------------------- Switch / Checkbox

export function Switch({
  id,
  checked,
  onChange,
  ariaLabel,
}: {
  id?: string;
  checked: boolean;
  onChange: (v: boolean) => void;
  ariaLabel?: string;
}) {
  return (
    <RSwitch.Root id={id} className="switch" checked={checked} onCheckedChange={onChange} aria-label={ariaLabel}>
      <RSwitch.Thumb className="switch-thumb" />
    </RSwitch.Root>
  );
}

/** Fila de ajuste: texto a la izquierda, interruptor a la derecha; toda la fila es clicable. */
export function SwitchRow({
  id,
  label,
  description,
  checked,
  onChange,
}: {
  id: string;
  label: string;
  description?: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <div className="setting-row">
      <Switch id={id} checked={checked} onChange={onChange} />
      <label htmlFor={id}>
        <span>{label}</span>
        {description && <span className="hint">  # {description}</span>}
      </label>
    </div>
  );
}

export function Checkbox({ id, checked, onChange }: { id?: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <RCheckbox.Root id={id} className="checkbox" checked={checked} onCheckedChange={(v) => onChange(v === true)}>
      <RCheckbox.Indicator>
        <Check strokeWidth={3} />
      </RCheckbox.Indicator>
    </RCheckbox.Root>
  );
}

// ---------------------------------------------------------------- Dialog

export function Dialog({
  open,
  onOpenChange,
  title,
  description,
  children,
  footer,
  wide,
}: {
  open: boolean;
  onOpenChange: (v: boolean) => void;
  title: ReactNode;
  description?: ReactNode;
  children?: ReactNode;
  footer?: ReactNode;
  wide?: boolean;
}) {
  return (
    <RDialog.Root open={open} onOpenChange={onOpenChange}>
      <RDialog.Portal>
        <RDialog.Overlay className="overlay" />
        <RDialog.Content className={cx("dialog", wide && "dialog--wide")} aria-describedby={undefined}>
          <div className="dialog__header">
            <RDialog.Title className="dialog__title">{title}</RDialog.Title>
            {description && <RDialog.Description className="dialog__desc">{description}</RDialog.Description>}
          </div>
          {children && <div className="dialog__body">{children}</div>}
          {footer && <div className="dialog__footer">{footer}</div>}
          <RDialog.Close asChild>
            <Button variant="ghost" size="sm" className="btn--icon dialog__close" aria-label="Cerrar">
              <X />
            </Button>
          </RDialog.Close>
        </RDialog.Content>
      </RDialog.Portal>
    </RDialog.Root>
  );
}

// ---------------------------------------------------------------- Toast

export interface ToastState {
  message: string;
  action?: { label: string; run: () => void };
}

/** Un aviso a la vez, en la parte inferior; desaparece solo a los 5 s. */
export function useToast() {
  const [toast, setToast] = useState<ToastState | null>(null);
  useEffect(() => {
    if (!toast) return;
    const id = window.setTimeout(() => setToast(null), 5000);
    return () => window.clearTimeout(id);
  }, [toast]);
  const notify = useCallback((message: string, action?: ToastState["action"]) => setToast({ message, action }), []);
  const view = toast ? (
    <div className="toast" role="status">
      <span style={{ flex: 1 }}>{toast.message}</span>
      {toast.action && (
        <Button
          variant="ghost"
          size="sm"
          onClick={() => {
            toast.action?.run();
            setToast(null);
          }}
        >
          {toast.action.label}
        </Button>
      )}
      <Button variant="ghost" size="sm" className="btn--icon" aria-label="Cerrar aviso" onClick={() => setToast(null)}>
        <X />
      </Button>
    </div>
  ) : null;
  return { notify, view };
}

export function Spinner({ size = 20 }: { size?: number }) {
  return <LoaderCircle className="spin" width={size} height={size} aria-label="Cargando" />;
}

/** Reloj HH:MM:SS con los separadores aparte (laten mientras corre). */
export function Clock({ text, state, className }: { text: string; state: string; className: string }) {
  const [h, m, sec] = text.split(":");
  return (
    <span className={className} data-state={state} role="timer" aria-label={`Tiempo trabajado ${text}`}>
      {h}
      <span className="clock__sep">:</span>
      {m}
      <span className="clock__sep">:</span>
      {sec}
    </span>
  );
}
