export function GET() {
  return new Response(null, {
    status: 308,
    headers: { location: "https://api.fe2vi.space/openapi.json" },
  });
}
