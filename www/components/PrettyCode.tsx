type PrettyCodeProps = {
    code: string,
    className?: string,
    lineNumbers?: boolean,
    fixed?: boolean,
    lang?: string
}

const PrettyCode = ({code, className, lineNumbers, fixed, lang}: PrettyCodeProps) => {
    const numLines = code
        .split("\n").map((_, index) => {
            return (index + 1).toString();
        })
        .join("\n");
    const language = lang ? lang : "language-javascript";
    const lineOpacity = lineNumbers ? "opacity-50" : "invisible";
    return (

        <div className={`flex flex-row ${className}`}>
            <div>
                <pre>
                    <code className={`language-markup ${lineOpacity}`}>
                        {
                            numLines
                        }
                    </code>
                </pre>
            </div>
            <div className="flex-grow overflow-x-scroll sm:overflow-x-hidden">
                <pre>
                        <code className={`${language}`}>
                                {code}
                        </code>
                </pre>
            </div>
        </div>
    );
}

export default PrettyCode;