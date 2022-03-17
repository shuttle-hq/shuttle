import { useRouter } from "next/router";
import { useApiKeyModalState } from "./ApiKeyModal";
import { useUser } from "@auth0/nextjs-auth0";

export default function Header() {
  const { basePath } = useRouter();
  const [open, setOpen] = useApiKeyModalState();
  const { user, error, isLoading } = useUser();


  return (
    <div className="p-3 flex justify-end">
      {user && (
        <button
          className="text-gray-200 hover:text-white border-2 border-current box-border font-bold py-3 px-8 rounded transition"
          onClick={() => setOpen(true)}
        >
          View Api Key
        </button>
      )}

      {!user && (
        <a
          className="text-gray-200 hover:text-white border-2 border-current font-bold py-3 px-8 rounded transition"
          href="/api-key"
        >
          Log In
        </a>
      )}
    </div>
  );
}
