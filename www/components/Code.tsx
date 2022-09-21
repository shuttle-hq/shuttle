import { faClipboard } from "@fortawesome/free-regular-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import ReactTooltip from "react-tooltip";
import NoSsr from "./NoSsr";
import { gtagEvent } from "../lib/gtag";
type CodeProps = {
  readonly id: string;
  readonly code: string;
};

const copyToClipboard = (code: string, id: string) => {
  gtagEvent({
    action: "copy_install_script",
    category: "Code",
    label: "Copied Install Script",
    value: id,
  });

  navigator.clipboard.writeText(code);
};

export default function Code({ code, id }: CodeProps) {
  return (
    <div
      className="cursor-pointer text-slate-700 dark:text-dark-200"
      data-tip
      data-for={id}
      data-event="click"
      data-event-off="click"
      data-delay-hide="2000"
    >
      <pre
        className={`group flex justify-between gap-4 rounded bg-slate-300 p-4 dark:bg-gray-500`}
      >
        <code>
          <span className="select-none">$ </span>
          {code}
        </code>
        <FontAwesomeIcon
          className="h-6 opacity-0 transition-opacity group-hover:opacity-100"
          icon={faClipboard}
        />
      </pre>
      <NoSsr>
        <ReactTooltip
          id={id}
          place="top"
          effect="float"
          afterShow={() => copyToClipboard(code, id)}
        >
          <b>Copied to clipboard!</b>
        </ReactTooltip>
      </NoSsr>
    </div>
  );
}
