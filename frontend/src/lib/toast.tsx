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
  const [toast, setToast] = useState<ToastPayload | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    const handler = (e: CustomEvent<ToastPayload>) => {
      if (timerRef.current) clearTimeout(timerRef.current);
      setToast(e.detail);
      timerRef.current = setTimeout(() => setToast(null), e.detail.duration);
    };
    window.addEventListener(EVENT, handler as EventListener);
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
      window.removeEventListener(EVENT, handler as EventListener);
    };
  }, []);

  if (!toast) return null;

  return (
    <div className="fixed bottom-6 left-1/2 -translate-x-1/2 z-[9999] pointer-events-none">
      <div
        className={`flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium shadow-lg ${VARIANT_STYLES[toast.variant]}`}
      >
        <ToastIcon variant={toast.variant} />
        <span>{toast.message}</span>
      </div>
    </div>
  );
}
