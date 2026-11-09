import type { Metadata } from "next";
import onboarding from "../../../templates/ONBOARDING.md";
import { CityGate } from "@/components/CityGate";
import { pageMetadata } from "@/lib/seo";

export const metadata: Metadata = pageMetadata(
  "fe2VI",
  "The testnet city. Vault, War, and the NFT market show live arena data. Cup and Trading Village are coming soon.",
  "/",
);

export default function Page() {
  return <CityGate onboarding={onboarding} />;
}
