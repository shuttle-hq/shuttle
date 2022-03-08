import ReactTooltip from 'react-tooltip';

// Todo should rename this to something more descriptive

type CodeProps = {
    code: string,
    lang?: string
}

const copyToClipboard = (code) => {
    navigator.clipboard.writeText(code);
}

const Code = ({code, lang}: CodeProps) => {
    const language = lang ? lang : "language-javascript";
    return (
        <div className="flex flex-grow rounded-md cursor-pointer overflow-x-auto" data-tip data-for="copiedTip" data-event="click" data-event-off="click" data-delay-hide='2000'>
                <pre className={`rounded-md flex-grow ${lang}`}>
                    <code className={`${language}`}>
                        $ {code}
                    </code>
                </pre>
                <ReactTooltip id="copiedTip" place="top" effect="float" afterShow={() => copyToClipboard(code)}>
                    <b>Copied to clipboard!</b>
                </ReactTooltip>
        </div>
    );
}

export default Code;