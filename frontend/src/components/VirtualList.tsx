import { useRef, type ReactNode } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { cn } from "../lib/utils";

interface VirtualListProps<T> {
  items: T[];
  renderItem: (item: T, index: number) => ReactNode;
  /** Initial row-height estimate; actual heights are measured. */
  estimateSize?: number;
  overscan?: number;
  gap?: number;
  className?: string;
  itemKey?: (item: T, index: number) => string | number;
}

/**
 * Lightweight windowed list: renders only the rows in (and near) the viewport,
 * so folders with thousands of documents stay smooth. Row heights are measured
 * after mount, so variable-height items (errors, multi-line titles) work too.
 */
export function VirtualList<T>({
  items,
  renderItem,
  estimateSize = 64,
  overscan = 8,
  gap = 0,
  className,
  itemKey,
}: VirtualListProps<T>) {
  const parentRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => estimateSize,
    overscan,
    gap,
    getItemKey: itemKey ? (index) => itemKey(items[index], index) : undefined,
  });

  return (
    <div ref={parentRef} className={cn("overflow-auto", className)}>
      <div
        style={{
          height: `${virtualizer.getTotalSize()}px`,
          width: "100%",
          position: "relative",
        }}
      >
        {virtualizer.getVirtualItems().map((virtualItem) => (
          <div
            key={virtualItem.key}
            data-index={virtualItem.index}
            ref={virtualizer.measureElement}
            style={{
              position: "absolute",
              top: 0,
              left: 0,
              width: "100%",
              transform: `translateY(${virtualItem.start}px)`,
            }}
          >
            {renderItem(items[virtualItem.index], virtualItem.index)}
          </div>
        ))}
      </div>
    </div>
  );
}
