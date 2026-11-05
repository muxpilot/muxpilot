"use client";

import { useTheme } from "next-themes";
import { useEffect, useState } from "react";

// The generator's theme provider (next-themes) now stores the choice under "muxpilot-theme"
// (site.chrome.tsx themeStorageKey), so this toggle only calls setTheme.
export function ThemeToggle() {
  const { resolvedTheme, setTheme } = useTheme();
  const [mounted, setMounted] = useState(false);

  useEffect(() => setMounted(true), []);

  // A stable glyph until mounted, so the server HTML and the first client render agree.
  const label = !mounted ? "◐" : resolvedTheme === "dark" ? "☀" : "☾";

  return (
    <button
      type="button"
      className="theme-toggle"
      onClick={() => setTheme(resolvedTheme === "dark" ? "light" : "dark")}
      aria-label="Toggle color theme"
      title="Toggle color theme"
    >
      {label}
    </button>
  );
}
