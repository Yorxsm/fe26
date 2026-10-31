import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  reactStrictMode: true,
  webpack: (config) => {
    config.module.rules.push({ test: /\.md$/, type: "asset/source" });
    return config;
  },
  async redirects() {
    return [
      { source: "/games/heist-night-template", destination: "/games/heist-night", permanent: true },
      { source: "/games/heist-night-template/:path*", destination: "/games/heist-night/:path*", permanent: true },
      { source: "/games/trading-cup-template", destination: "/games/trading-cup", permanent: true },
      { source: "/games/trading-cup-template/:path*", destination: "/games/trading-cup/:path*", permanent: true },
    ];
  },
  transpilePackages: [
    "three",
    "@react-three/fiber",
    "@react-three/drei",
    "postprocessing",
  ],
};

export default nextConfig;
