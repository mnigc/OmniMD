import { cn } from "../lib/utils";

interface SegmentedOption<T extends string> {
  value: T;
  icon: React.ReactNode;
  label: string;
}

interface SegmentedControlProps<T extends string> {
  options: SegmentedOption<T>[];
  value: T;
  onChange: (value: T) => void;
  size?: "sm" | "md";
  className?: string;
}

export function SegmentedControl<T extends string>({
  options,
  value,
  onChange,
  size = "md",
  className,
}: SegmentedControlProps<T>) {
  return (
    <div
      className={cn(
        "flex items-center gap-0.5 rounded-md border border-border bg-secondary/50 p-0.5",
        className
      )}
    >
      {options.map((opt) => (
        <button
          key={opt.value}
          onClick={() => onChange(opt.value)}
          title={opt.label}
          className={cn(
            "flex items-center justify-center rounded-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
            size === "sm" ? "h-7 w-8" : "h-8 w-9",
            value === opt.value
              ? "bg-background text-foreground shadow-sm ring-1 ring-border"
              : "text-muted-foreground hover:text-foreground"
          )}
        >
          {opt.icon}
        </button>
      ))}
    </div>
  );
}