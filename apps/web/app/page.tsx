import type { Metadata } from "next";
import onboarding from "../../../templates/ONBOARDING.md";
import { CityGate } from "@/components/CityGate";
import { pageMetadata } from "@/lib/seo";

export const metadata: Metadata = pageMetadata(
  "fe2VI",
  "The testnet city. Spectators can walk into every room. Cup and Trading Village are not live yet.",
  "/",
);

export default function Page() {
  return <CityGate onboarding={onboarding} />;
}
