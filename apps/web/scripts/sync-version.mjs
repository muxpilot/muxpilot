// sync-version.mjs — copy the crate's version into a committed TS constant so the
// site's version badge always matches what's actually released.
//
// Vercel uploads only apps/web, so the site cannot read ../../crates/.../Cargo.toml
// at build time on the server. The shared helper bakes the version into
// app/version.ts when the crate is reachable (local builds / deploys) and reports
// `source-absent` on Vercel, where the committed value ships. Wired as a
// `prebuild` step and run before deploy.
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { syncVersion } from "@npm-factory/site-chrome/sync-version";

const here = dirname(fileURLToPath(import.meta.url));

const result = syncVersion({
  source: {
    kind: "cargo-toml",
    path: join(here, "..", "..", "..", "crates", "muxpilot", "Cargo.toml"),
  },
  out: join(here, "..", "app", "version.ts"),
  exportName: "MUXPILOT_VERSION",
  sourceLabel: "crates/muxpilot/Cargo.toml",
});

if (result.state === "unreadable") {
  console.error(`sync-version: ${result.path}: ${result.reason}`);
  process.exit(1);
}

console.log(
  result.state === "written"
    ? `sync-version: wrote app/version.ts (v${result.version})`
    : "sync-version: crate Cargo.toml not found; keeping committed version",
);
