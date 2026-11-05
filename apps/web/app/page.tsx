import { mdxComponentsFor } from "@byoklab/site";
import type { Metadata } from "next";

import { FeatureList } from "../components/features";
import { InstallCards } from "../components/install-cards";
import { LandingFooter, LandingHeader } from "../components/landing-chrome";
import { THEME_MIGRATE } from "../components/theme-migrate";
import { MUXPILOT_VERSION } from "../lib/version";
import { site } from "./_site/runtime";

// kind "app": the landing page is the site's own (the generator writes it for docs and landing sites only).
// The title and description are the ones the site has always had; the canonical URL, Open Graph and
// robots come from the generated root layout.
export const metadata: Metadata = {
  title: { absolute: "MuxPilot — agent-aware tmux workspace picker" },
  description: site.config.description
};

const { Video } = mdxComponentsFor(site) as { Video: (props: { name: string; label: string }) => React.ReactNode };

export default function Home() {
  return (
    <div className="mp">
      <script dangerouslySetInnerHTML={{ __html: THEME_MIGRATE }} />
      <LandingHeader />
      <main className="wrap">
        <section className="hero">
          <div>
            <p className="eyebrow">Agent-aware tmux control</p>
            <h1>Pilot every tmux session from one menu.</h1>
            <p className="lede">
              A fast Rust picker for tmux sessions, tmuxinator layouts, working directories, and the coding agents
              running inside them.
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
          <div className="demo" aria-label="MuxPilot picker">
            <div className="demo-bar">
              <span className="nm">muxpilot</span>
              <span className="conn">
                <i /> live
              </span>
            </div>
            <img className="demo-media" src={`/media/picker.png?v=${MUXPILOT_VERSION}`} alt="MuxPilot picker listing tmux workspaces" />
          </div>
        </section>

        <section className="band" id="get-started">
          <p className="sec-label">Get started</p>
          <h2 className="sec-title">Install it your way — one binary, no runtime.</h2>
          <InstallCards />
          <p className="band-text">
            Then run <code>muxpilot</code> in any pane — or bind it to a tmux popup (<a href="#plugin">below</a>).
            Confirm your setup any time with <code>muxpilot doctor</code>, or follow the{" "}
            <a href="/docs/installation">installation guide</a>.
          </p>
        </section>

        <section className="band" id="features">
          <p className="sec-label">Why MuxPilot</p>
          <h2 className="sec-title">One menu over everything you launch.</h2>
          <FeatureList />
          <figure className="demo" style={{ marginTop: 30 }}>
            <Video name="picker" label="MuxPilot picker demo" />
          </figure>
        </section>

        <section className="band" id="windows">
          <p className="sec-label">Drill in</p>
          <h2 className="sec-title">Expand a session into its windows.</h2>
          <p className="band-text" style={{ marginTop: 0, marginBottom: 24 }}>
            Press <code>l</code> or Space on a running session to unfold it into a tree of its windows, each with its
            pane count, agents, and last activity. Open any window directly with Enter; <code>h</code> collapses the
            session again.
          </p>
          <figure className="demo">
            <img className="demo-media" src={`/media/tree.png?v=${MUXPILOT_VERSION}`} width={1180} height={640} alt="A MuxPilot session expanded into a tree of its windows" />
            <figcaption className="demo-cap">l / Space expands a session into its windows; h collapses it</figcaption>
          </figure>
        </section>

        <section className="band" id="plugin">
          <p className="sec-label">Live in tmux</p>
          <h2 className="sec-title">Bind it to a key, launch it in a popup.</h2>
          <div className="cmd">
            <span className="p">tmux.conf</span> bind-key C-j display-popup -E -w 80% -h 70% &quot;muxpilot&quot;
          </div>
          <p className="band-text">
            Install through TPM with <code>set -g @plugin &apos;muxpilot/muxpilot&apos;</code>, or wire the popup
            binding by hand. See the <a href="/docs/tmux-plugin">tmux plugin guide</a>.
          </p>
        </section>
      </main>
      <LandingFooter />
    </div>
  );
}
