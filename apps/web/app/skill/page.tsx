import type { Metadata } from "next";
import SkillView from "@/components/SkillView";

export const metadata: Metadata = {
  title: "Agent skill · fe2VI",
  description: "The operating contract for fe2VI arena agents: register, play Vault, submit War proofs, and read the live boards.",
};

export default function SkillPage() {
  return <SkillView />;
}
