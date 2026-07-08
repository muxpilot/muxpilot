import { existsSync } from "node:fs";
import { join } from "node:path";
import { SiteHeader } from "./components/SiteHeader";
import { SiteFooter } from "./components/SiteFooter";
import { DemoClip } from "./components/DemoClip";
import { FeatureHighlight, type Feature } from "./components/FeatureHighlight";

// The feature showcase. To add a feature later: drop a screenshot in
// public/media, then append one object here — the highlight renders itself and
// alternates sides automatically. `spotlight` rects are in % of the image.
const STATE_JSON = `{
  "schema_version": 1,
  "source": "tmux",
  "current_session": "billing-portal-2",
  "sessions": [
    {
      "name": "billing-portal-2",
      "windows": [
        {
          "id": "@20",
          "index": 0,
          "name": "logs",
          "active": true,
          "panes": [
            {
              "id": "%40",
              "active": true,
              "role": "agent",
              "current_command": "claude",
              "agent": {
                "kind": "claude",
                "status": "working",
                "source": "hook",
                "confidence": 95,
                "attention": false
              }
            }
          ]
        }
      ]
    }
  ]
}`;

const FEATURES: Feature[] = [
  {
    id: "picker",
    label: "The picker",
    title: "A native tmux popup — not an fzf wrapper.",
    blurb: (
      <>
        Fuzzy filter, grouped <strong>running</strong> and{" "}
        <strong>configured</strong> targets, live details, and a preview pane —
        all rendered by MuxPilot itself, driven entirely from the keyboard.
      </>
    ),
    media: {
      kind: "shot",
      src: "/media/picker.png",
      alt: "MuxPilot native picker listing tmux workspaces",
      ratio: "1180 / 640",
      spotlight: {
        top: 85.5,
        left: 4,
        width: 85,
        height: 8.5,
        label: "keyboard-driven actions",
      },
    },
  },
  {
    id: "agents",
    label: "Agent awareness",
    title: "See which sessions have an agent working.",
    blurb: (
      <>
        MuxPilot reads agent hooks when present and falls back to process and
        pane signals — surfacing a live <code>◍</code> count and status
        (<code>agent</code> / <code>active</code>) next to every session.
      </>
    ),
    media: {
      kind: "shot",
      src: "/media/picker.png",
      alt: "Agent count and status column in the MuxPilot picker",
      ratio: "1180 / 640",
      spotlight: {
        top: 23,
        left: 60.5,
        width: 22.5,
        height: 57.5,
        label: "◍ live agent count + status",
        anchor: "tr",
      },
    },
  },
  {
    id: "tree",
    label: "Window tree",
    title: "Unfold any session into its windows.",
    blurb: (
      <>
        Press <code>l</code> or <code>Space</code> to expand a running session
        into a tree of its windows — pane counts, agents, and activity per
        window — then jump straight to the one you want.
      </>
    ),
    media: {
      kind: "shot",
      src: "/media/tree.png",
      alt: "A MuxPilot session expanded into a tree of its windows",
      ratio: "1180 / 640",
      spotlight: {
        top: 41,
        left: 2.5,
        width: 95,
        height: 45,
        label: "windows, unfolded inline",
        anchor: "bl",
      },
    },
  },
  {
    id: "scriptable",
    label: "Scriptable",
    title: "Every view has a --json twin.",
    blurb: (
      <>
        Pipe <code>muxpilot state --json</code> into status bars, sidebars, and
        your own tooling. Structured, versioned, and agent-aware — the same data
        the picker draws.
      </>
    ),
    media: {
      kind: "code",
      command: "muxpilot state --json",
      source: STATE_JSON,
    },
  },
];

// Server component: checks at render time whether the VHS pipeline has already
// dropped a rendered demo into public/media. If so we embed the looping video;
// otherwise we show a labelled poster telling maintainers how to regenerate it.
function HeroDemo() {
  const publicDir = join(process.cwd(), "public", "media");
  const mp4 = existsSync(join(publicDir, "picker.mp4"));
  const gif = existsSync(join(publicDir, "picker.gif"));
  const poster = existsSync(join(publicDir, "picker.png"));

  return (
    <div className="demo" aria-label="MuxPilot picker demo">
      <div className="demo-bar">
        <span className="nm">muxpilot</span>
        <span className="conn">
          <i /> live
        </span>
      </div>
      {mp4 ? (
        <video
          className="demo-media"
          autoPlay
          loop
          muted
          playsInline
          poster={poster ? "/media/picker.png" : undefined}
        >
          <source src="/media/picker.mp4" type="video/mp4" />
          {gif ? (
            <img className="demo-media" src="/media/picker.gif" alt="MuxPilot picker demo" />
          ) : null}
        </video>
      ) : gif ? (
        <img className="demo-media" src="/media/picker.gif" alt="MuxPilot picker demo" />
      ) : (
        <div className="demo-poster">
          Demo video not rendered yet.
          <br />
          Run <code>make -C marketing regen</code> to build it.
        </div>
      )}
    </div>
  );
}

export default function Home() {
  return (
    <>
      <SiteHeader />
      <main className="wrap">
        <section className="hero">
          <div>
            <p className="eyebrow">Agent-aware tmux control</p>
            <h1>Pilot every tmux session from one menu.</h1>
            <p className="lede">
              A fast Rust picker for tmux sessions, tmuxinator layouts, working
              directories, and the coding agents running inside them.
            </p>
            <div className="actions">
              <a className="btn primary" href="/docs/installation">
                Install MuxPilot
              </a>
              <a className="btn ghost" href="/docs/introduction">
                Read the docs
              </a>
            </div>
            <div className="cmd">
              <span className="p">$</span> cargo install --path crates/muxpilot
            </div>
          </div>
          <HeroDemo />
        </section>

        <section className="band" id="features">
          <p className="sec-label">Why MuxPilot</p>
          <h2 className="sec-title">One menu over everything you launch.</h2>
          <div className="feat-list">
            {FEATURES.map((feature, i) => (
              <FeatureHighlight
                key={feature.id}
                feature={feature}
                reversed={i % 2 === 1}
              />
            ))}
          </div>
        </section>

        <section className="band" id="windows">
          <p className="sec-label">Drill in</p>
          <h2 className="sec-title">Expand a session into its windows.</h2>
          <p className="lede" style={{ marginBottom: 24 }}>
            Press <code>l</code> or Space on a running session to unfold it into
            a tree of its windows, each with its pane count, agents, and last
            activity. Open any window directly with Enter; <code>h</code>{" "}
            collapses the session again.
          </p>
          <DemoClip
            name="tree"
            caption="l / Space expands a session into its windows; h collapses it"
          />
        </section>

        <section className="band" id="plugin">
          <p className="sec-label">Live in tmux</p>
          <h2 className="sec-title">Bind it to a key, launch it in a popup.</h2>
          <div className="cmd">
            <span className="p">tmux.conf</span> bind-key C-j display-popup -E
            -w 80% -h 70% &quot;muxpilot&quot;
          </div>
          <p className="lede" style={{ marginTop: 20 }}>
            Install through TPM with{" "}
            <code style={{ fontFamily: "var(--font-mono)" }}>
              set -g @plugin &apos;muxpilot/muxpilot&apos;
            </code>
            , or wire the popup binding by hand. See the{" "}
            <a href="/docs/tmux-plugin">tmux plugin guide</a>.
          </p>
        </section>
      </main>
      <SiteFooter />
    </>
  );
}
