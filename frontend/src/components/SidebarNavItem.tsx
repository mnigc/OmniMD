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
        "relative flex items-center gap-2.5 px-3 py-2 rounded-lg text-sm transition-all duration-150",
        collapsed && "justify-center px-0",
        active
          ? "bg-primary/12 text-primary font-medium shadow-[inset_0_1px_0_0_hsl(0_0%_100%/0.04)]"
          : "text-muted-foreground hover:bg-muted/70 hover:text-foreground"
      )}
    >
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