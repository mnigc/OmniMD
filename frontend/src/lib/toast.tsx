import { useEffect, useRef, useState } from "react";
import { AlertCircle, CheckCircle2, Info } from "lucide-react";

export type ToastVariant = "success" | "error" | "info";

type ToastPayload = {
  message: string;
  duration: number;
  id: number;
  variant: ToastVariant;
};

const EVENT = "omnimd-toast";

/** Max simultaneously visible toasts; older ones are dropped when exceeded so
 *  a burst of failures (e.g. batch enqueue) can't flood the screen. */
const MAX_VISIBLE_TOASTS = 5;

let nextId = 0;

export function showToast(
  message: string,
  duration = 2000,
  variant: ToastVariant = "success",
): void {
  window.dispatchEvent(
    new CustomEvent<ToastPayload>(EVENT, {
      detail: { message, duration, id: nextId++, variant },
    }),
  );
}

const VARIANT_STYLES: Record<ToastVariant, string> = {
  success: "bg-emerald-600 text-white",
  error: "bg-destructive text-destructive-foreground",
  info: "bg-primary text-primary-foreground",
};

function ToastIcon({ variant }: { variant: ToastVariant }) {
  if (variant === "error") return <AlertCircle size={14} />;
  if (variant === "info") return <Info size={14} />;
  return <CheckCircle2 size={14} />;
}

export function ToastPortal() {
  const [toasts, setToasts] = useState<ToastPayload[]>([]);
  const timersRef = useRef<Set<ReturnType<typeof setTimeout>>>(new Set());

  useEffect(() => {
    const dismiss = (id: number) => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    };
    const handler = (e: CustomEvent<ToastPayload>) => {
      const toast = e.detail;
      // Each toast keeps its own timer; keep only the newest MAX_VISIBLE_TOASTS.
      setToasts((prev) => [...prev, toast].slice(-MAX_VISIBLE_TOASTS));
      const timer = setTimeout(() => {
        timersRef.current.delete(timer);
        dismiss(toast.id);
      }, toast.duration);
      timersRef.current.add(timer);
    };
    window.addEventListener(EVENT, handler as EventListener);
    return () => {
      window.removeEventListener(EVENT, handler as EventListener);
      for (const timer of timersRef.current) clearTimeout(timer);
      timersRef.current.clear();
    };
  }, []);

  if (toasts.length === 0) return null;

  return (
    <div className="fixed bottom-6 left-1/2 -translate-x-1/2 z-[9999] pointer-events-none flex flex-col items-center gap-2">
      {toasts.map((toast) => (
        <div
          key={toast.id}
          className={`flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium shadow-lg ${VARIANT_STYLES[toast.variant]}`}
        >
          <ToastIcon variant={toast.variant} />
          <span>{toast.message}</span>
        </div>
      ))}
    </div>
  );
}
