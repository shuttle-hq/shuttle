import classnames from "classnames";
import { Switch } from "@headlessui/react";
import { useLocalStorage, useMedia } from "react-use";
import { useEffect } from "react";

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
    <Switch
      checked={isDarkTheme}
      onChange={() => {
        updateTheme(theme === "dark" ? "light" : "dark");
      }}
      className={
        "relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent bg-slate-300 transition-colors duration-200 ease-in-out dark:bg-gray-500"
      }
    >
      <span className="sr-only">Use setting</span>
      <span
        className={
          "pointer-events-none relative inline-block h-5 w-5 translate-x-0 transform rounded-full bg-slate-200 text-slate-700 shadow ring-0 transition duration-200 ease-in-out dark:translate-x-5 dark:bg-dark-700 dark:text-dark-300"
        }
      >
        <span
          className={
            "absolute inset-0 flex h-full w-full items-center justify-center opacity-100 transition-opacity duration-200 ease-in dark:opacity-0 dark:duration-100 dark:ease-out"
          }
          aria-hidden="true"
        >
          <svg
            xmlns="http://www.w3.org/2000/svg"
            className="h-6 w-6"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            strokeWidth={2}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z"
            />
          </svg>
        </span>
        <span
          className={
            "absolute inset-0 flex h-full w-full items-center justify-center opacity-0 transition-opacity duration-100 ease-out dark:opacity-100 dark:duration-200 dark:ease-in"
          }
          aria-hidden="true"
        >
          <svg
            xmlns="http://www.w3.org/2000/svg"
            className="h-6 w-6"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            strokeWidth={2}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z"
            />
          </svg>
        </span>
      </span>
    </Switch>
  );
}
