import { useEffect, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';

/** The Windows shell reads the native title and icon. Only controls are drawn here. */
export function WindowBar() {
  const [maximized, setMaximized] = useState(false);
  useEffect(() => {
    const win = getCurrentWindow();
    let mounted = true;
    const refresh = async () => {
      const value = await win.isMaximized();
      if (mounted) setMaximized(value);
    };
    void refresh();
    const subscription = win.onResized(refresh);
    return () => {
      mounted = false;
      void subscription.then((unlisten) => unlisten());
    };
  }, []);
  const win = getCurrentWindow();
  const restore = maximized ? 'Restore down' : 'Maximize';
  return (
    <div
      className="window-bar"
      data-tauri-drag-region
      role="group"
      aria-label="Window controls"
    >
      <button
        type="button"
        title="Minimize"
        aria-label="Minimize"
        onClick={() => void win.minimize()}
      >
        <svg viewBox="0 0 16 16" aria-hidden="true">
          <path d="M3 8.5h10" />
        </svg>
      </button>
      <button
        type="button"
        title={restore}
        aria-label={restore}
        onClick={() => void win.toggleMaximize()}
      >
        <svg viewBox="0 0 16 16" aria-hidden="true">
          {maximized ? (
            <path d="M5 5V3h8v8h-2M3 5h8v8H3z" />
          ) : (
            <rect x="3.5" y="3.5" width="9" height="9" />
          )}
        </svg>
      </button>
      <button
        type="button"
        className="window-close"
        title="Close"
        aria-label="Close"
        onClick={() => void win.close()}
      >
        <svg viewBox="0 0 16 16" aria-hidden="true">
          <path d="m3.5 3.5 9 9m0-9-9 9" />
        </svg>
      </button>
    </div>
  );
}
