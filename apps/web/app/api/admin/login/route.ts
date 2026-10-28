import { NextResponse } from "next/server";

async function digest(value: string) {
  const buf = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(value));
  return [...new Uint8Array(buf)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

export async function POST(request: Request) {
  const token = process.env.ADMIN_TOKEN;
  if (!token) return new NextResponse(null, { status: 404 });
  const form = await request.formData();
  const given = String(form.get("token") ?? "");
  if (given !== token) return new NextResponse(null, { status: 404 });
  const response = NextResponse.redirect(new URL("/admin", request.url), 303);
  response.cookies.set("fe2vi_admin", await digest(token), {
    httpOnly: true,
    sameSite: "lax",
    secure: new URL(request.url).protocol === "https:",
    path: "/",
    maxAge: 60 * 60 * 12,
  });
  return response;
}
