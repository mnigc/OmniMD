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
    <div className={cn("flex flex-wrap items-center gap-x-2 gap-y-1", className)}>
      {SELLING_POINTS.map((point, i) => {
        const Icon = point.icon;
        return (
          <span key={point.labelKey} className="inline-flex items-center gap-1">
            {i > 0 && <span className="opacity-40">·</span>}
            <span className="inline-flex items-center gap-1 text-xs text-muted-foreground">
              <Icon size={12} className="shrink-0 opacity-70" />
              {t(point.labelKey)}
            </span>
          </span>
        );
      })}
    </div>
  );
}
