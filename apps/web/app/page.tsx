import { headers } from "next/headers";
import onboarding from "../../../templates/ONBOARDING.md";
import { CityGate } from "@/components/CityGate";
import { LaunchIntro } from "@/components/LaunchIntro";

export default async function Page() {
  const headerList = await headers();
  const host = headerList.get("host")?.split(":")[0] ?? "";
  const onCloudflare = Boolean(headerList.get("cf-ray"));
  if (onCloudflare && (host === "fe2vi.space" || host === "www.fe2vi.space")) return <LaunchIntro />;
  return <CityGate onboarding={onboarding} />;
}
