import { useLocalStorage, useMedia } from "react-use";
import { useEffect } from "react";
import { MoonIcon, SunIcon } from "@heroicons/react/solid";

type StorageTheme = "dark" | "light" | "system";

export default function ThemeSwitch() {
  const osTheme = useMedia("(prefers-color-scheme: dark)") ? "dark" : "light";
  const [storageTheme, setStorageTheme] = useLocalStorage<StorageTheme>(
    "app-theme",
    "system"
  );
  const theme = storageTheme === "system" ? osTheme : storageTheme;
  const isDarkTheme = theme === "dark";

  function updateTheme(theme: "dark" | "light") {
    setStorageTheme(theme === osTheme ? "system" : theme);
  }

  useEffect(() => {
    if (isDarkTheme) {
      document.body.classList.add("dark");
    } else {
      document.body.classList.remove("dark");
    }
  }, [isDarkTheme]);

  return (
    <button
      type="button"
      className="text-slate-600 hover:text-slate-900 dark:text-gray-200 hover:dark:text-white"
      onClick={() => {
        updateTheme(theme === "dark" ? "light" : "dark");
      }}
    >
      {theme === "dark" ? (
        <SunIcon className="h-5 w-5" />
      ) : (
        <MoonIcon className="h-5 w-5" />
      )}
    </button>
  );
}
