import Section from './Section'

const HowItWorks = () => {
    return (
        <Section id="how-it-works" style={`bg-gray-600`} title="How Synth does it all" subtitle="for you">
            <div className="grid lg:grid-cols-2 lg:gap-10">
                <div className="grid my-auto grid-rows-2 gap-10 text-lg">
                    <div>
                        Synth allows data to be expressed as code, in a simple intuitive schema which defines constraints, relations and semantics in data generations. The schema is written in simple JSON files which can be checked into version control, reviewed for correctness and shared with external parties.
                    </div>
                    <div className="text-gray-400 font-medium">
                        Synth integrates with relational and NoSQL databases, enabling data to be imported and create the scaffolding of the schema for you.
                    </div>
                </div>
                <div className="h-96 bg-black">
                    An infographic here
                </div>
            </div>
        </Section>
    );
}

export default HowItWorks;