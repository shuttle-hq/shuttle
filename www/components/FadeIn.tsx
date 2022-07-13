import { ReactNode, useState } from "react";
import {
  motion,
  useViewportScroll,
  useTransform,
  HTMLMotionProps,
  useMotionTemplate,
} from "framer-motion";

export default function FadeIn({
  children,
  ...props
}: HTMLMotionProps<"div">): JSX.Element {
  const [height, setHeight] = useState<null | number>(null);
  const [top, setTop] = useState<null | number>(null);
  const { scrollY } = useViewportScroll();
  const val = useTransform(scrollY, (scrollY) => {
    if (top == null || height == null) {
      return 0;
    }

    const offset = innerHeight + scrollY - top;
    const h = height * 1.5;

    if (offset < 0) return 0;
    if (offset >= h) return 1;

    return Math.max(0, Math.min(1, offset / h));
  });

  const offsetTop = useTransform(val, [0, 0.5, 0.5, 1], [40, 40, 40, 0]);
  const scale = useTransform(val, [0, 1], [0.8, 1]);
  const opacity = useTransform(val, [0, 0.5, 0.5, 1], [0.6, 0.6, 0.6, 1]);

  return (
    <motion.div
      ref={(el) => {
        if (el != null) {
          const rect = el.getBoundingClientRect();

          setTop(rect.top + window.scrollY);
          setHeight(rect.height);
        }
      }}
      style={{
        scale,
        opacity,
        y: offsetTop,
      }}
      {...props}
    >
      {children}
    </motion.div>
  );
}
