type LogoProps = {
  variant?: "hero" | "nav";
  className?: string;
};

/**
 * Hero is the full mark at native resolution, capped in CSS at 280px.
 * Nav is the fe2V letters only, so 36px of height is the letter height.
 */
export function Logo({ variant = "nav", className = "" }: LogoProps) {
  const hero = variant === "hero";
  if (hero) {
    return (
      <img
        className={`logo-hero ${className}`.trim()}
        alt="fe2VI"
        src="/brand/fe2vi-logo@1x.webp"
        width={361}
        height={197}
      />
    );
  }
  return (
    <img
      className={`logo-nav ${className}`.trim()}
      alt="fe2VI"
      src="/brand/fe2vi-logo@1x.webp"
      width={361}
      height={197}
    />
  );
}
