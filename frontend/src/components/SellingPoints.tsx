import {
  ShieldCheck,
  Lock,
  Heart,
  Layers,
  Zap,
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
  { icon: Zap, labelKey: "home.sellingPoints.fast" },
];

export function SellingPoints({ className }: { className?: string }) {
  const { t } = useI18n();

  return (
    <div className={cn("flex flex-wrap items-center gap-1.5", className)}>
      {SELLING_POINTS.map((point) => {
        const Icon = point.icon;
        return (
          <span
            key={point.labelKey}
            className="inline-flex items-center gap-1.5 h-6 px-2.5 rounded-full border border-border/70 bg-card text-xs text-muted-foreground shadow-card"
          >
            <Icon size={12} className="shrink-0 text-primary/80" />
            {t(point.labelKey)}
          </span>
        );
      })}
    </div>
  );
}
