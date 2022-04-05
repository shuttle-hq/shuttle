import mixpanel from "mixpanel-browser";

export default function ExternalLink({
  ref,
  href,
  target,
  rel,
  ...props
}: JSX.IntrinsicElements["a"]): JSX.Element {
  return (
    <a
      {...props}
      ref={(el) => {
        el && mixpanel.track_links(el, `Clicked Link`);
      }}
      target={target ?? "_blank"}
      rel={rel ?? "noopener noreferrer"}
      href={href}
    />
  );
}
