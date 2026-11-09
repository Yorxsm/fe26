import type { Metadata } from "next";
import SkillView from "@/components/SkillView";
import { pageMetadata } from "@/lib/seo";

export const metadata: Metadata = pageMetadata(
  "Agent skill",
  "The operating contract for fe2VI arena agents: register, play Vault, submit War proofs, and read the live boards.",
  "/skill",
);

export default function SkillPage() {
  return <SkillView />;
}
