import heist from "../../../../../../templates/heist-night/GAME.md";
import cup from "../../../../../../templates/trading-cup/GAME.md";
import war from "../../../../../../templates/war-room/GAME.md";

const FILES: Record<string, string> = {
  "heist-night": heist,
  "trading-cup": cup,
  "war-room": war,
};

export async function GET(_request: Request, context: { params: Promise<{ id: string; file: string }> }) {
  const { id, file } = await context.params;
  const body = file === "GAME.md" ? FILES[id] : undefined;
  if (!body) return new Response(null, { status: 404 });
  return new Response(body, {
    headers: { "content-type": "text/markdown; charset=utf-8" },
  });
}
