interface PageHeaderProps {
  title: string;
  description?: string;
  actions?: React.ReactNode;
}

import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "./ui/tooltip";

export function PageHeader({ title, description, actions }: PageHeaderProps) {
  return (
    <div className="flex items-center justify-between gap-4 mb-6">
      <div className="min-w-0">
        <Tooltip>
          <TooltipTrigger asChild>
            <h1 className="text-xl font-semibold tracking-tight truncate cursor-help">
              {title}
            </h1>
          </TooltipTrigger>
          <TooltipContent side="bottom" align="start" className="max-w-xs text-xs">
            {title}
          </TooltipContent>
        </Tooltip>
        {description && (
          <p className="text-sm text-muted-foreground mt-1">{description}</p>
        )}
      </div>
      {actions && (
        <div className="shrink-0 flex items-center gap-2">{actions}</div>
      )}
    </div>
  );
}