type SynthProps = {
    style?: string
}

const Synth = ({style}: SynthProps) => {
    const renderedStyle = style ? style : "text-brand-600 font-semibold";
    return (
        <span className={renderedStyle}>Synth</span>
    )
}

export default Synth;