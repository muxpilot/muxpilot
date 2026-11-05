import { defineSite } from "@byoklab/site-config";

import { MUXPILOT_VERSION } from "./lib/version";

export default defineSite({
  schemaVersion: 1,
  name: "MuxPilot",
  description:
    "A fast Rust picker for tmux sessions, tmuxinator layouts, working directories, and the coding agents running inside them.",
  siteUrl: "https://muxpilot.n.yatsyk.com",
  projectId: "muxpilot",
  kind: "docs",
  // A public site: no preset, so the public theme (and the Umami host below, which a preset would supply).
  index: true,
  links: {
    github: "muxpilot/muxpilot",
    extra: [
      // The old header showed the version next to the name; the generator has no badge, so it is a header link to the release notes.
      { label: `v${MUXPILOT_VERSION}`, href: "https://github.com/muxpilot/muxpilot/releases", placement: "header" },
      // The old footer line, same words; the generator offers links only, so it links to the licence.
      { label: "Open source · MIT licensed", href: "https://github.com/muxpilot/muxpilot/blob/main/LICENSE", placement: "footer" },
      { label: "crates.io", href: "https://crates.io/crates/muxpilot", placement: "footer" },
      { label: "npm", href: "https://www.npmjs.com/package/muxpilot", placement: "footer" }
    ]
  },
  // The old header: Docs, Features, GitHub (GitHub comes from links.github).
  nav: [
    { title: "Docs", href: "/docs/introduction" },
    { title: "Features", href: "/#features" }
  ],
  // Public ingest ids (Umami website "muxpilot", GlitchTip project "muxpilot").
  telemetry: {
    umami: "8b0ecc58-1379-48ba-9b41-8234357ec090",
    umamiHost: "https://stats.i.byokapi.com",
    glitchtip: "https://43b16cf41bec4058aad866f763639cac@errors.i.byokapi.com/15"
  },
  // The VHS recordings of marketing/regen.sh land in public/media (mp4, gif, png, plus the captions).
  sections: { media: { videos: "public/media" } }
});
