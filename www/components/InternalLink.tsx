import Link, { LinkProps } from "next/link";
import { useRouter } from "next/router";
import mixpanel from "mixpanel-browser";

interface Props {
  readonly mixpanelEvent?: string;
}

export default function InternalLink({
  href,
  as,
  replace,
  scroll,
  shallow,
  passHref,
  prefetch,
  locale,
  mixpanelEvent,
  ...props
}: JSX.IntrinsicElements["a"] & LinkProps & Props): JSX.Element {
  const router = useRouter();

  if (!href) {
    return <span {...props} />;
  }

  return (
    <Link
      href={href}
      as={as}
      replace={replace}
      scroll={scroll}
      shallow={shallow}
      passHref={passHref}
      prefetch={prefetch}
      locale={locale}
    >
      <a
        {...props}
        onClick={(e) => {
          mixpanelEvent && mixpanel.track(mixpanelEvent);

          if (router.pathname === href) {
            e.preventDefault();

            document.body.scrollIntoView({
              behavior: "smooth",
            });

            setTimeout(() => {
              router.replace(href);
            }, 350);
          } else if (href.startsWith(router.pathname + "#")) {
            e.preventDefault();

            document
              .querySelector(href.slice(router.pathname.length))
              .scrollIntoView({
                behavior: "smooth",
              });

            setTimeout(() => {
              router.replace(href);
            }, 350);
          }
        }}
      />
    </Link>
  );
}
