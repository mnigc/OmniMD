# Split Divider Drag Fix

## Root cause

`ConvertPage.tsx` `SplitDivider` component has an early-return in its `useEffect`:

```ts
useEffect(() => {
  if (!dragging.current) return;   // <-- dragging.current is false on mount
  const handleMove = ...
  document.addEventListener("pointermove", handleMove, ...);
  ...
}, [onResize]);
```

The effect runs on mount when `dragging.current` is still `false`, so it bails out and never registers the `pointermove`/`pointerup` listeners. When the user presses down, no handler exists — drag never starts.

## Fix (1 edit in `frontend/src/pages/ConvertPage.tsx`, SplitDivider useEffect)

1. Remove the `if (!dragging.current) return;` guard at the top of the effect.
2. The inner handlers already check `dragging.current` per-event, so no listener leak.
3. Also remove the stale `document.body.style.setProperty("--omnimd-split-ratio", ...)` line inside `handleMove` — `onResize` already persists to localStorage; this body property is never read by anything.

## Validation

1. Open ConvertPage in split mode with a file loaded.
2. Hover the divider → it highlights.
3. Press and drag left/right → both panels resize smoothly.
4. Drag off the narrow divider (global listeners) → still works.
5. Refresh → ratio restored from localStorage.