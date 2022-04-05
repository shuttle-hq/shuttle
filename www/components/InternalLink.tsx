import Link, { LinkProps } from "next/link";
import { useRouter } from "next/router";

export default function InternalLink({
  href,
  as,
  replace,
  scroll,
  shallow,
  passHref,
  prefetch,
  locale,
  ...props
}: JSX.IntrinsicElements["a"] & LinkProps): JSX.Element {
  const router = useRouter();
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
          if (router.pathname === href) {
            e.preventDefault();

            document.body.scrollIntoView({
              behavior: "smooth",
            });

            setTimeout(() => {
              router.replace(href);
            }, 350);
          } else if (href.startsWith("#")) {
            e.preventDefault();

            document.querySelector(href).scrollIntoView({
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
