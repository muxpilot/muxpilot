import { existsSync } from "node:fs";
import { join } from "node:path";
import { SiteHeader } from "./components/SiteHeader";
import { SiteFooter } from "./components/SiteFooter";
import { DemoClip } from "./components/DemoClip";

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
        <span className="meta">demo · 8 sessions · 3 agents</span>
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
      <div className="demo-cap">
        Terminal capture regenerated via <code>make -C marketing regen</code>
      </div>
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
          <div className="grid3">
            <div className="card">
              <div className="ico">▸</div>
              <h3>Native picker</h3>
              <p>
                A responsive tmux popup with fuzzy filter, grouped targets, live
                details, and a preview pane — no fzf dependency required.
              </p>
            </div>
            <div className="card">
              <div className="ico">◍</div>
              <h3>Agent state</h3>
              <p>
                Reads agent hooks when present, and falls back to process and
                pane signals — so you can see which sessions have Claude working
                right now.
              </p>
            </div>
            <div className="card">
              <div className="ico">└─</div>
              <h3>Window tree</h3>
              <p>
                Expand any running session into a tree of its windows — pane
                counts, agents, and activity per window — and jump straight to
                the one you want.
              </p>
            </div>
            <div className="card">
              <div className="ico">{"{ }"}</div>
              <h3>Scriptable</h3>
              <p>
                Every view has a <code>--json</code> twin. Pipe{" "}
                <code>muxpilot state</code> into status bars, sidebars, and your
                own tooling.
              </p>
            </div>
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
              set -g @plugin &apos;yatsyk/muxpilot&apos;
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
