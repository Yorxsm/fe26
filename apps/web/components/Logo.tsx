type LogoProps = {
  variant?: "hero" | "nav";
  className?: string;
};

/** Cut-out of public/fe2vi-logo-400.png. Hero is 320px on desktop and 220px below. */
export function Logo({ variant = "nav", className = "" }: LogoProps) {
  const hero = variant === "hero";
  return (
    <img
      className={`${hero ? "logo-hero" : "logo-nav"} ${className}`.trim()}
      alt="fe2VI"
      src="/brand/fe2vi-logo@1x.webp"
      srcSet="/brand/fe2vi-logo@1x.webp 1x, /brand/fe2vi-logo@2x.webp 2x"
      width={hero ? 320 : 51}
      height={hero ? 177 : 28}
    />
  );
}
