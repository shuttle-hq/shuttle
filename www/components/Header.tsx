import { useRouter } from "next/router";
import { useApiKeyModalState } from "./ApiKeyModal";
import { useUser } from "@auth0/nextjs-auth0";
import InternalLink from "./InternalLink";
import { SHUTTLE_DOCS_URL } from "../lib/constants";
import ExternalLink from "./ExternalLink";

const navigation = [
  { name: "Features", href: "#features", internal: true },
  { name: "Examples", href: "#examples", internal: true },
  { name: "Docs", href: SHUTTLE_DOCS_URL, internal: false },
  { name: "Blog", href: "/blog", internal: true },
];

export default function Header() {
  const { basePath } = useRouter();
  const [open, setOpen] = useApiKeyModalState();
  const { user, error, isLoading } = useUser();

  return (
    <header className="sticky top-0 z-20 border-b border-gray-400 bg-dark-700">
      <nav className="mx-auto max-w-6xl px-4 sm:px-6 lg:px-8" aria-label="Top">
        <div className="flex w-full items-center justify-between py-3">
          <div className="flex items-center">
            <InternalLink href="/">
              <div className="relative m-auto flex">
                <img
                  className="h-8 w-auto"
                  src={`${basePath}/images/logo.png`}
                  alt="Shuttle"
                />
                <span className="absolute top-[-18px] right-[-19px] scale-[.45] rounded bg-brand-orange1 px-[10px] py-[2px] text-base font-bold text-dark-700">
                  ALPHA
                </span>
              </div>
            </InternalLink>
            <div className="ml-10 hidden space-x-8 lg:block">
              {navigation.map((link) =>
                link.internal ? (
                  <InternalLink
                    key={link.name}
                    href={link.href}
                    className="text-base font-medium text-white hover:text-indigo-50"
                  >
                    {link.name}
                  </InternalLink>
                ) : (
                  <ExternalLink
                    key={link.name}
                    href={link.href}
                    className="text-base font-medium text-white hover:text-indigo-50"
                  >
                    {link.name}
                  </ExternalLink>
                )
              )}
            </div>
          </div>
          <div className="ml-10 space-x-4">
            {user && (
              <button
                className="inline-block rounded border border-current py-2 px-4 text-base font-medium text-gray-200 hover:text-white "
                onClick={() => setOpen(true)}
              >
                Log In
              </button>
            )}

            {!user && (
              <a
                className="inline-block rounded border border-current py-2 px-4 text-base font-medium text-gray-200 hover:text-white "
                href="/login"
              >
                Log In
              </a>
            )}
          </div>
        </div>
        <div className="flex flex-wrap justify-center space-x-6 py-4 lg:hidden">
          {navigation.map((link) => (
            <InternalLink
              key={link.name}
              href={link.href}
              className="text-base font-medium text-white hover:text-indigo-50"
            >
              {link.name}
            </InternalLink>
          ))}
        </div>
      </nav>
    </header>
  );
}
