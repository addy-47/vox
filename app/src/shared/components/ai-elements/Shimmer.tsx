import React, { memo, useMemo, type CSSProperties, type ElementType } from "react";
import { motion } from "framer-motion";
import { cn } from "@/shared/lib/utils";

export interface TextShimmerProps {
  children: string;
  as?: ElementType;
  className?: string;
  duration?: number;
  spread?: number;
}

const ShimmerComponent: React.FC<TextShimmerProps> = ({
  children,
  as: Component = "span",
  className,
  duration = 2.2,
  spread = 2,
}) => {
  const dynamicSpread = useMemo(
    () => (children?.length ?? 0) * spread,
    [children, spread]
  );

  const MotionComponent = useMemo(() => motion.create(Component as keyof React.JSX.IntrinsicElements), [Component]);

  return (
    <MotionComponent
      animate={{ backgroundPosition: "0% center" }}
      initial={{ backgroundPosition: "100% center" }}
      transition={{
        duration,
        ease: "easeInOut",
        repeat: Number.POSITIVE_INFINITY,
      }}
      className={cn(
        "relative inline-block bg-[length:200%_100%,auto] bg-clip-text text-transparent select-none",
        "[--bg:linear-gradient(90deg,transparent_calc(50%-var(--spread)),white_50%,transparent_calc(50%+var(--spread)))]",
        className
      )}
      style={
        {
          "--spread": `${dynamicSpread}px`,
          backgroundImage:
            "var(--bg), linear-gradient(currentColor, currentColor)",
        } as CSSProperties
      }
    >
      {children}
    </MotionComponent>
  );
};

export const Shimmer = memo(ShimmerComponent);
Shimmer.displayName = "Shimmer";
