import type { Metadata } from "next";
import { themeInitScript } from "@npm-factory/site-chrome";
import "@npm-factory/site-chrome/styles.css";
import { THEME_STORAGE_KEY } from "./theme";

export const metadata: Metadata = {
  title: {
    template: "%s · MuxPilot",
    default: "MuxPilot — agent-aware tmux workspace picker",
  },
  description:
    "A fast Rust picker for tmux sessions, tmuxinator layouts, working directories, and the coding agents running inside them.",
};

// Runs before first paint to stamp the saved theme onto <html> so there is no
// flash of the wrong palette. Falls back to the OS preference when unset.
const THEME_INIT = themeInitScript(THEME_STORAGE_KEY);

export default function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en">
      <head>
        <script dangerouslySetInnerHTML={{ __html: THEME_INIT }} />
      </head>
      <body>{children}</body>
    </html>
  );
}
