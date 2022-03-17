import * as React from "react";

let componentCounter = 0;

function getNewComponentId(): number {
  componentCounter++;

  return componentCounter;
}

export function useComponentId(): number {
  const [componentId] = React.useState<number>(getNewComponentId);

  return componentId;
}

export function useId(): string {
  const label = useComponentId();

  return `label-${label}`;
}
