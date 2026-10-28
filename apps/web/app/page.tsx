import { readFileSync } from "node:fs";
import path from "node:path";
import { CityGate } from "@/components/CityGate";

export default function Page() {
  const onboarding = readFileSync(path.join(process.cwd(), "../../templates/ONBOARDING.md"), "utf8");
  return <CityGate onboarding={onboarding} />;
}
