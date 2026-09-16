import { useEffect, useRef, type ReactNode } from "react";
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
  // Signature of the current items' keys: the scroll reset below must fire
  // only when the LIST itself is replaced (folder switch, tab change, search
  // cleared), not when a caller clones the array for an in-place field update
  // (e.g. marking a document's openedAt) — that used to yank the scroll back
  // to the top on every item click.
  const keysSignatureRef = useRef<string | null>(null);
  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => estimateSize,
    overscan,
    gap,
    getItemKey: itemKey ? (index) => itemKey(items[index], index) : undefined,
  });

  // 列表整体更换（换文件夹、切 tab、清空搜索）时复位滚动：旧偏移量在
  // 变短的列表上会被 clamp，出现白屏或定位错乱。以 itemKey 序列判断列表
  // 是否真的换了，键集合不变的原地字段更新不复位。
  useEffect(() => {
    const signature = items
      .map((item, index) => (itemKey ? itemKey(item, index) : index))
      .join("\n");
    if (signature === keysSignatureRef.current) return;
    keysSignatureRef.current = signature;
    if (parentRef.current) parentRef.current.scrollTop = 0;
  }, [items, itemKey]);

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
