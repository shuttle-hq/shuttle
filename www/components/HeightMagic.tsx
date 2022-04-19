import { ReactNode, useState } from "react";

interface Props {
  readonly children?: ReactNode | undefined;
}

export default function HeightMagic({ children }: Props) {
  const [height, setHeight] = useState(0);

  return (
    <div
      className="overflow-hidden transition-[height]"
      style={{ height: `${height}px` }}
    >
      <div
        ref={(el) => {
          setHeight(el?.getBoundingClientRect().height ?? 0);
        }}
      >
        {children}
      </div>
    </div>
  );
}
