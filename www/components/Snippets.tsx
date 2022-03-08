import {Children, ReactElement, useEffect, useState} from "react";
import Prism from 'prismjs'
import {FontAwesomeIcon} from "@fortawesome/react-fontawesome";
import {faCircle, faPlayCircle, faRedo} from "@fortawesome/free-solid-svg-icons";
import AccentButton from "./AccentButton";
import PrettyCode from "./PrettyCode";

import {PlaygroundError, pgGenerate} from "../lib/playground";

type SnippetsProps = {
    children?: ReactElement[]
}

type SnippetProps = {
    title: string,
    label: string,
    code: object
}

const Snippet = ({title, label, code}: SnippetProps) => {
    return <></>;
}

const Snippets = ({children}: SnippetsProps) => {
    const [active, setActive] = useState(0);

    let length = 0;
    let code;
    let title;
    let tabs = Array();
    Children.forEach(children, (child, index) => {
        if (index == active) {
            title = child.props.title;
            code = child.props.code;
            tabs.push(
                <div
                    className="font-medium cursor-pointer flex justify-center rounded-lg p-1 border-gray-300 bg-gray-300 text-gray-800 border-2"
                    onClick={() => {
                        setActive(index);
                        setOutput("");
                    }}
                >
                    {child.props.label}
                </div>
            );
        } else {
            tabs.push(
                <div
                    className="font-medium cursor-pointer rounded-lg p-1 hover:bg-gray-400 hover:text-gray-800 transition border-gray-400 border-2 flex justify-center"
                    onClick={() => {
                        setActive(index);
                        setOutput("");
                    }}>
                    {child.props.label}
                </div>
            );
        }
        length++;
    });

    const [output, setOutput] = useState("");

    useEffect(() => {
        Prism.highlightAll();
    });

    const codeString = JSON.stringify(code, null, 2);

    return <div className="flex lg:flex-row flex-col">
        <div className="flex-grow rounded bg-gray-600 lg:mr-16">
            <div
                className="font-semibold bg-gray-600 rounded-lg rounded-br-none rounded-bl-none py-2 pl-4 pr-2 border-2 border-t-0 border-r-0 border-l-0 border-gray-500 text-gray-300"
            >
                <div className="flex flex-row">
                    <div className="flex-grow my-auto">
                        <FontAwesomeIcon className="inline-block h-3.5 pb-1 pr-1" style={{"color": "#6422f1"}}
                                         icon={faCircle}/>
                        <FontAwesomeIcon className="inline-block h-3.5 pb-1 pr-1" style={{"color": "#f46591"}}
                                         icon={faCircle}/>
                        <FontAwesomeIcon className="inline-block h-3.5 pb-1 pr-4" style={{"color": "#2bd38d"}}
                                         icon={faCircle}/>
                        {title}
                    </div>
                    <div
                        className="my-auto"
                        onClick={
                            () => {
                                pgGenerate(codeString, 4)
                                    .then((generated) => setOutput(JSON.stringify(generated, null, 2)))
                                    .catch((err) => console.log("failed to run playground query: " + err));
                            }
                        }
                    >
                        <AccentButton compact>
                            <FontAwesomeIcon icon={ output ? faRedo : faPlayCircle } className="h-4 pr-2 pt-0.5"/>
                            { output ? "Re-run" : "Run" }
                        </AccentButton>
                    </div>
                </div>
            </div>
            {
                ! output && <PrettyCode code={codeString} className={output && "border-b-2 border-gray-500"} lineNumbers/>
            }
            {
                output && <PrettyCode code={output} lineNumbers fixed/>
            }
        </div>
        <div className="lg:pt-0 pt-16 text-gray-300 lg:w-80">
            <div className={`grid grid-rows-${length} gap-2`}>
                {tabs}
            </div>
        </div>
    </div>;
}

export {Snippet, Snippets}