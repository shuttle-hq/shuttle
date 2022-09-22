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
      // todo: add custom event for ext links? they are automatically tracked
      target={target ?? "_blank"}
      rel={rel ?? "noopener noreferrer"}
      href={href}
    />
  );
}
