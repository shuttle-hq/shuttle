import { faClipboard } from "@fortawesome/free-regular-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import ReactTooltip from "react-tooltip";
import NoSsr from "./NoSsr";

// Todo should rename this to something more descriptive

type CodeProps = {
  code: string;
  lang?: string;
};

const copyToClipboard = (code) => {
  navigator.clipboard.writeText(code);
};

const Code = ({ code, lang }: CodeProps) => {
  const language = lang ? lang : "language-javascript";
  return (
    <div
      className="flex flex-grow rounded-md cursor-pointer overflow-x-auto"
      data-tip
      data-for="copiedTip"
      data-event="click"
      data-event-off="click"
      data-delay-hide="2000"
    >
      <pre className={`rounded-md flex-grow flex justify-between ${lang}`}>
        <code className={`${language}`}>$ {code}</code>
        <FontAwesomeIcon
          className="h-6"
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

export default Code;
