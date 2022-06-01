import { useApiKeyModalState } from "./ApiKeyModal";
import { useUser } from "@auth0/nextjs-auth0";
import mixpanel from "mixpanel-browser";

export default function LoginButton() {
  const { user, error, isLoading } = useUser();
  const [open, setOpen] = useApiKeyModalState();

  const label = "Log In";
  const className =
    "inline-block w-full rounded border border-slate-900 bg-transparent py-1 px-4 text-center text-base font-medium text-slate-900 transition-colors hover:bg-slate-800 hover:text-slate-100 dark:border-white dark:text-white hover:dark:bg-white hover:dark:text-dark-700";

  if (user) {
    return (
      <button
        className={className}
        onClick={() => {
          mixpanel.track(label);

          setOpen(true);
        }}
      >
        {label}
      </button>
    );
  }

  return (
    <a
      className={className}
      href="/login"
      ref={(el) => {
        el && mixpanel.track_links(el, label);
      }}
    >
      {label}
    </a>
  );
}
