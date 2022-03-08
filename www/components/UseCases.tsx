import CardNoLink from './CardNoLink';
import {faDna, faUserSecret, faSeedling} from '@fortawesome/free-solid-svg-icons'
import {useRouter} from "next/router";

import Section from "./Section";

const UseCases = () => {
    const {basePath} = useRouter();
    return (
        <Section id="use-cases" style="bg-gray-600" title="What you can do" subtitle="with Synth">
            <div className="grid lg:grid-cols-3 gap-16 lg:gap-6">
                <CardNoLink
                    header="Anonymize"
                    copy="Use Synth to generate correct, anonymized data that looks and quacks like production."
                    title="Anonymize sensitive production data."
                    path={`${basePath}/images/anon.png`}
                />
                <CardNoLink
                    header="Seed"
                    copy="Generate test data fixtures for your development, testing and continuous integration."
                    title="Seed development and environments and CI"
                    path={`${basePath}/images/seed.png`}
                />
                <CardNoLink
                    header="Synthesize"
                    copy="Generate data that tells the story you want to tell. Specify constraints, relations and all your semantics."
                    title="Create realistic data to your specifications"
                    path={`${basePath}/images/synthesize.png`}
                />
            </div>
        </Section>
    )
}

export default UseCases