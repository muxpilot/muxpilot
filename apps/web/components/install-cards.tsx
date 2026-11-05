type Install = {
  tag: string;
  title: string;
  blurb: React.ReactNode;
  prompt: string;
  command: string;
};

const INSTALLS: Install[] = [
  {
    tag: "brew",
    title: "Homebrew",
    blurb: "macOS or Linux, straight from the tap.",
    prompt: "$",
    command: "brew install muxpilot/tap/muxpilot",
  },
  {
    tag: "cargo",
    title: "Cargo",
    blurb: "From crates.io with Rust 1.75 or newer.",
    prompt: "$",
    command: "cargo install muxpilot",
  },
  {
    tag: "npm",
    title: "npm",
    blurb: "Prebuilt binary, fetched through Node.",
    prompt: "$",
    command: "npm install -g muxpilot",
  },
  {
    tag: "tmux",
    title: "tmux plugin",
    blurb: (
      <>
        Add to <code>~/.tmux.conf</code>, then run TPM.
      </>
    ),
    prompt: "tpm",
    command: "set -g @plugin 'muxpilot/muxpilot'",
  },
];

/** The four install routes of the landing page, one card each. */
export function InstallCards() {
  return (
    <div className="grid3 not-prose">
      {INSTALLS.map((install) => (
        <div className="card" key={install.tag}>
          <div className="ico pill">{install.tag}</div>
          <h3>{install.title}</h3>
          <p>{install.blurb}</p>
          <div className="cmd">
            <span className="p">{install.prompt}</span> {install.command}
          </div>
        </div>
      ))}
    </div>
  );
}
