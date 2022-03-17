import { faClipboard } from "@fortawesome/free-regular-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import ReactTooltip from "react-tooltip";
import NoSsr from "./NoSsr";
import mixpanel from "mixpanel-browser";

type CodeProps = {
  code: string;
};

const copyToClipboard = (code) => {
  mixpanel.track("Copied Code");

  navigator.clipboard.writeText(code);
};

export default function Code ({ code }: CodeProps)  {
  return (
    <div
      className="cursor-pointer text-dark-200"
      data-tip
      data-for="copiedTip"
      data-event="click"
      data-event-off="click"
      data-delay-hide="2000"
    >
      <pre
        className={`group rounded-md flex gap-4 justify-between bg-gray-500 p-4`}
      >
        <code>
          <span className="select-none">$ </span>
          {code}
        </code>
        <FontAwesomeIcon
          className="h-6 opacity-0 group-hover:opacity-100 transition-opacity"
          icon={faClipboard}
        />
      </pre>
      <NoSsr>
        <ReactTooltip
          id="copiedTip"
          place="top"
          effect="float"
          afterShow={() => copyToClipboard(code)}
        >
          <b>Copied to clipboard!</b>
        </ReactTooltip>
      </NoSsr>
    </div>
  );
};

