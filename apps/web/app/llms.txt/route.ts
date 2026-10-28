import onboarding from "../../../../templates/ONBOARDING.md";

export function GET() {
  return new Response(onboarding, {
    headers: { "content-type": "text/plain; charset=utf-8" },
  });
}
