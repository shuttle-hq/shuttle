import { ClipboardCheckIcon, ClipboardIcon } from "@heroicons/react/outline";
import { useEffect, useState } from "react";
import { useCopyToClipboard, useWindowSize } from "react-use";
import { gtagEvent } from "../lib/gtag";

interface Props {
  readonly code: string;
  readonly name?: string;
}

export default function Copy({ code, name }: Props) {
  const [copyToClipboardState, copyToClipboard] = useCopyToClipboard();
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    let timeout = setTimeout(() => {
      setCopied(false);
    }, 1500);

    return () => void clearTimeout(timeout);
  }, [copied]);

  return (
    <button
      type="button"
      className="absolute right-2 top-2 inline-flex items-center rounded border border-transparent bg-dark-800 px-3 py-2 text-sm font-medium leading-4 text-white shadow-sm hover:bg-dark-700"
      onClick={() => {
        gtagEvent({
          action: "copy_example_code",
          category: "Code",
          label: "Copied Code",
          value: name,
        });
        copyToClipboard(code);
        setCopied(true);
      }}
    >
      {copied ? (
        <>
          <ClipboardCheckIcon
            className="-ml-0.5 mr-2 h-4 w-4"
            aria-hidden="true"
          />
          Copied
        </>
      ) : (
        <>
          <ClipboardIcon className="-ml-0.5 mr-2 h-4 w-4" aria-hidden="true" />
          Copy
        </>
      )}
    </button>
  );
}
