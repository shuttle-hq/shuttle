import { useApiKeyModalState } from "./ApiKeyModal";
import { useUser } from "@auth0/nextjs-auth0";
import { gtagEvent } from "../lib/gtag";

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
          gtagEvent({
            action: "login_click",
            category: "Login",
            label: "Existing Session Login",
            // todo: track api-key?
            // value: api-key,
          });

          setOpen(true);
        }}
      >
        {label}
      </button>
    );
  }

  return (
    <a
      href="/login"
      className={className}
      onClick={() => {
        gtagEvent({
          action: "new_login_click",
          category: "Login",
          label: "New Session Login",
          // todo: track api-key?
          // value: api-key,
        });
      }}
    >
      {label}
    </a>
  );
}
