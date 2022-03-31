import { useRouter } from "next/router";
import { useApiKeyModalState } from "./ApiKeyModal";
import { useUser } from "@auth0/nextjs-auth0";
import Link from "next/link";
import InternalLink from "./InternalLink";

const navigation = [
  { name: "Solutions", href: "#" },
  // { name: "Pricing", href: "#" },
  { name: "Docs", href: "#" },
  { name: "Company", href: "#" },
];

export default function Header() {
  const { basePath } = useRouter();
  const [open, setOpen] = useApiKeyModalState();
  const { user, error, isLoading } = useUser();

  return (
    <header className="bg-dark-700 sticky top-0 z-20 border-b border-gray-400">
      <nav className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8" aria-label="Top">
        <div className="w-full py-3 flex items-center justify-between">
          <div className="flex items-center">
            <Link href="/">
              <a>
                <div className="flex m-auto relative">
                  <img
                    className="h-8 w-auto"
                    src={`${basePath}/images/logo.png`}
                    alt="Shuttle"
                  />
                  <span className="bg-brand-orange1 text-dark-700 font-bold absolute scale-[.45] top-[-18px] right-[-19px] text-base px-[10px] py-[2px] rounded">
                    ALPHA
                  </span>
                </div>
              </a>
            </Link>
            <div className="hidden ml-10 space-x-8 lg:block">
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
          </div>
          <div className="ml-10 space-x-4">
            {user && (
              <button
                className="text-gray-200 hover:text-white inline-block py-2 px-4 border border-current rounded text-base font-medium "
                onClick={() => setOpen(true)}
              >
                Log In
              </button>
            )}

            {!user && (
              <a
                className="text-gray-200 hover:text-white inline-block py-2 px-4 border border-current rounded text-base font-medium "
                href="/login"
              >
                Log In
              </a>
            )}
          </div>
        </div>
        <div className="py-4 flex flex-wrap justify-center space-x-6 lg:hidden">
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
