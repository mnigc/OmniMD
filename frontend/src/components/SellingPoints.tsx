import {
  ShieldCheck,
  Lock,
  Heart,
  Layers,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { useI18n } from "../i18n";
import { cn } from "../lib/utils";

interface SellingPoint {
  icon: LucideIcon;
  labelKey: string;
}

const SELLING_POINTS: SellingPoint[] = [
  { icon: ShieldCheck, labelKey: "home.sellingPoints.local" },
  { icon: Lock, labelKey: "home.sellingPoints.privacy" },
  { icon: Heart, labelKey: "home.sellingPoints.free" },
  { icon: Layers, labelKey: "home.sellingPoints.batch" },
];

export function SellingPoints({ className }: { className?: string }) {
  const { t } = useI18n();

  return (
    <div className={cn("flex flex-wrap items-center justify-center gap-1.5", className)}>
      {SELLING_POINTS.map((point) => {
        const Icon = point.icon;
        return (
          <span
            key={point.labelKey}
            className={cn(
              "inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full",
              "text-[11px] font-medium text-muted-foreground",
              "bg-muted/40 border border-border/60",
              "transition-colors hover:bg-muted hover:text-foreground"
            )}
          >
            <Icon size={11} className="shrink-0 opacity-70" />
            {t(point.labelKey)}
          </span>
        );
      })}
    </div>
  );
}
