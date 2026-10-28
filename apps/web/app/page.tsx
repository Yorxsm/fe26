import onboarding from "../../../templates/ONBOARDING.md";
import { CityGate } from "@/components/CityGate";

export default function Page() {
  return <CityGate onboarding={onboarding} />;
}
