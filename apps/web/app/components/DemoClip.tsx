// Embeds a rendered VHS demo clip with a caption, for use inside docs MDX.
// `name` matches the media basename produced by the marketing pipeline
// (e.g. "picker" -> /media/picker.mp4). No filesystem check here: if the clip
// has not been rendered yet the <video> simply shows nothing until
// `make -C marketing regen` produces it.
export function DemoClip({
  name,
  caption,
}: {
  name: string;
  caption?: string;
}) {
  return (
    <figure className="demo" style={{ margin: "0 0 24px" }}>
      <video className="demo-media" autoPlay loop muted playsInline>
        <source src={`/media/${name}.mp4`} type="video/mp4" />
        <img className="demo-media" src={`/media/${name}.gif`} alt={caption ?? name} />
      </video>
      {caption ? <figcaption className="demo-cap">{caption}</figcaption> : null}
    </figure>
  );
}
