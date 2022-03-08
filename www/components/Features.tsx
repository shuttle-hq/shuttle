import SmallCard from './SmallCard'
import Section from "./Section";

import {FontAwesomeIcon} from "@fortawesome/react-fontawesome";
import {
    faAlignCenter,
    faAsterisk,
    faCode,
    faExternalLinkAlt,
    faFileImport,
    faPastafarianism
} from "@fortawesome/free-solid-svg-icons";
import AccentButton from "./AccentButton";

const Features = () => {
    return (
        <Section id="features" style="bg-gray-600" title="Test against better data" subtitle="in less time">
            <div className="grid grid-rows-2 gap-16 lg:grid-cols-2 lg:gap-y-10 lg:gap-x-5">
                <SmallCard
                    icon={faCode}
                    head="Data as Code"
                    content="Synth uses a declarative configuration language that allows you to specify your entire data model as code."
                    link="/docs/getting_started/schema"
                />
                <SmallCard
                    icon={faFileImport}
                    head="Easy Imports"
                    content="Synth can import data straight from existing sources and automatically create accurate and versatile data models."
                    link="/docs/getting_started/command-line"
                />
                <SmallCard
                    icon={faPastafarianism}
                    head="Database Agnostic"
                    content="Synth supports semi-structured data and is database agnostic - playing nicely with SQL and NoSQL databases."
                    link="/blog/2021/03/09/postgres-data-gen"
                />
                <SmallCard
                    icon={faAlignCenter}
                    head="Semantic Data Types"
                    content="Synth supports generation for thousands of semantic types such as credit card numbers, email addresses and more."
                    link="/docs/content/string"
                />
            </div>
        </Section>
    )
}

export default Features;