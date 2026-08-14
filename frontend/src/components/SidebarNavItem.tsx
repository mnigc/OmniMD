import { cn } from "../lib/utils";

interface SidebarNavItemProps {
  icon: React.ReactNode;
  label: string;
  active: boolean;
  onClick: () => void;
}

export function SidebarNavItem({
  icon,
  label,
  active,
  onClick,
}: SidebarNavItemProps) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "relative flex items-center gap-2 px-3 py-2 rounded-md text-sm transition-colors",
        active
          ? "bg-primary/10 text-primary font-medium"
          : "text-muted-foreground hover:bg-muted hover:text-foreground"
      )}
    >
      {active && (
        <span className="absolute left-0 top-1/2 -translate-y-1/2 h-4 w-0.5 rounded-full bg-primary" />
      )}
      {icon}
      <span className="truncate">{label}</span>
    </button>
  );
}