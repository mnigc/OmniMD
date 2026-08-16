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
    <div className={cn("selling-points-wrapper flex flex-wrap items-center justify-center gap-2", className)}>
      {SELLING_POINTS.map((point) => {
        const Icon = point.icon;
        return (
          <span
            key={point.labelKey}
            className={cn(
              "golden-pill inline-flex items-center gap-1.5 px-3 py-1",
              "text-xs font-semibold text-amber-800 dark:text-amber-100",
            )}
          >
            <Icon size={12} className="shrink-0" />
            {t(point.labelKey)}
          </span>
        );
      })}
    </div>
  );
}
