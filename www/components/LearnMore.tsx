import {faExternalLinkAlt} from "@fortawesome/free-solid-svg-icons";
import {FontAwesomeIcon} from "@fortawesome/react-fontawesome";

type LearnMoreProps = {
    text: string,
    arrow?: boolean
}

const LearnMore = ({text, arrow}) => {
    return (
        <div className="flex flex-row text-accent-1 pt-2">
            <div className="flex-grow block">
                {text}
            </div>
            <div>
                <FontAwesomeIcon
                    icon={faExternalLinkAlt}
                    className={`h-4 mb-1 transition ${arrow ? "opacity-80" : "opacity-0"} inline-block`}
                />
            </div>
        </div>
    );
}

export default LearnMore;