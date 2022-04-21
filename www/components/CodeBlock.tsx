import { PrismLight as SyntaxHighlighter } from "react-syntax-highlighter";
import rust from "react-syntax-highlighter/dist/cjs/languages/prism/rust";
import oneDark from "react-syntax-highlighter/dist/cjs/styles/prism/one-dark";
import { useWindowSize } from "react-use";
import Copy from "./Copy";
import HeightMagic from "./HeightMagic";

SyntaxHighlighter.registerLanguage("rust", rust);

cargo.displayName = "cargo";
cargo.aliases = [];

function cargo(Prism: any) {
  Prism.languages.cargo = {
    builtin: /\b(?:Packaging|Archiving|Compiling|Finished)\b/,
  };
}

SyntaxHighlighter.registerLanguage("cargo", cargo);

interface Props {
  readonly language: string;
  readonly code: string;
  readonly showLineNumbers?: boolean;
}

export default function CodeBlock({ code, language, showLineNumbers }: Props) {
  useWindowSize();

  return (
    <div className="relative my-2 rounded bg-[#282C34] p-4 shadow-lg">
      <Copy code={code}></Copy>

      <HeightMagic>
        <SyntaxHighlighter
          className="!m-0 overflow-scroll !p-0"
          language={language}
          style={oneDark}
          showLineNumbers={showLineNumbers}
          lineNumberStyle={{
            width: "3.25em",
            position: "sticky",
            left: 0,
            background: "#282C34",
          }}
        >
          {code}
        </SyntaxHighlighter>
      </HeightMagic>
    </div>
  );
}
