import { useEffect, useRef, useState } from "react";
import { CheckCircle2 } from "lucide-react";
import { useI18n } from "../i18n";

type ToastPayload = {
  message: string;
  duration: number;
  id: number;
};

const EVENT = "omnimd-toast";

let nextId = 0;

export function showToast(message: string, duration = 2000): void {
  window.dispatchEvent(
    new CustomEvent<ToastPayload>(EVENT, {
      detail: { message, duration, id: nextId++ },
    }),
  );
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
      <div className="flex items-center gap-2 px-4 py-2 rounded-lg bg-emerald-600 text-white text-sm font-medium shadow-lg">
        <CheckCircle2 size={14} />
        <span>{toast.message}</span>
      </div>
    </div>
  );
}