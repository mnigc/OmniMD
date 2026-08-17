import { cn } from "../lib/utils";
import { Tooltip, TooltipTrigger, TooltipContent } from "./ui/tooltip";

interface SidebarNavItemProps {
  icon: React.ReactNode;
  label: string;
  active: boolean;
  onClick: () => void;
  collapsed?: boolean;
}

export function SidebarNavItem({
  icon,
  label,
  active,
  onClick,
  collapsed,
}: SidebarNavItemProps) {
  const button = (
    <button
      onClick={onClick}
      aria-label={collapsed ? label : undefined}
      className={cn(
        "relative flex items-center gap-2 px-3 py-2 rounded-md text-sm transition-colors",
        collapsed && "justify-center px-0",
        active
          ? "bg-primary/10 text-primary font-medium"
          : "text-muted-foreground hover:bg-muted hover:text-foreground"
      )}
    >
      {active && (
        <span className="absolute left-0 top-1/2 -translate-y-1/2 h-4 w-0.5 rounded-full bg-primary" />
      )}
      {icon}
      <span className={cn("truncate", collapsed && "hidden")}>{label}</span>
    </button>
  );

  if (collapsed) {
    return (
      <Tooltip>
        <TooltipTrigger asChild>{button}</TooltipTrigger>
        <TooltipContent side="right">{label}</TooltipContent>
      </Tooltip>
    );
  }

  return button;
}