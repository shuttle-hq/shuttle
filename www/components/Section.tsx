import {ReactChildren, ReactElement} from "react";
import {JSXElement} from "ast-types/gen/nodes";

type SectionProps = {
    style: string,
    id: string,
    title?: string,
    subtitle?: string,
    children: any
};

const Section = ({style, id, title, subtitle, children}: SectionProps) => {
    return (
        <div id={id} className={`w-full ${style}`}>
            <div className="container w-10/12 xl:w-8/12 xl:px-12 py-5 pt-16 sm:pt-24 pb-16 sm:pb-24 mx-auto">
                <div className="max-w-4xl pb-10">
                    {
                        title
                            ? <div className="text-5xl font-medium">
                                {title}
                                {
                                    subtitle != null
                                        ? <span className="text-gray-400"> {subtitle}</span>
                                        : <></>
                                }
                            </div>
                            : <></>
                    }
                </div>
                {children}
            </div>
        </div>
    )
}

export default Section;