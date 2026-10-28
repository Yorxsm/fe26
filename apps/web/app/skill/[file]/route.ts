import onboarding from "../../../../../templates/ONBOARDING.md";

export async function GET(_request: Request, context: { params: Promise<{ file: string }> }) {
  const { file } = await context.params;
  if (file !== "SKILL.md") return new Response(null, { status: 404 });
  return new Response(onboarding, {
    headers: { "content-type": "text/markdown; charset=utf-8" },
  });
}
