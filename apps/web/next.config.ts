import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  reactStrictMode: true,
  transpilePackages: [
    "@near-wallet-selector/core",
    "@near-wallet-selector/modal-ui",
    "@near-wallet-selector/my-near-wallet",
    "three",
    "@react-three/fiber",
    "@react-three/drei",
    "postprocessing",
  ],
};

export default nextConfig;
