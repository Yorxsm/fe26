import { NextResponse } from "next/server";
import type { NextRequest } from "next/server";

async function digest(value: string) {
  const buf = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(value));
  return [...new Uint8Array(buf)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

function launchHost(host: string) {
  return host === "fe2vi.space" || host === "www.fe2vi.space";
}

export async function middleware(request: NextRequest) {
  const { pathname } = request.nextUrl;
  const host = (request.headers.get("host") ?? "").split(":")[0];
  const onCloudflare = Boolean(request.headers.get("cf-ray"));
  if (onCloudflare && launchHost(host)) {
    const asset =
      pathname.startsWith("/_next") ||
      pathname.startsWith("/brand") ||
      pathname.startsWith("/models") ||
      pathname.startsWith("/fonts") ||
      pathname === "/icon.png" ||
      pathname.startsWith("/opengraph") ||
      pathname === "/favicon.ico";
    if (!asset && pathname !== "/") {
      const url = request.nextUrl.clone();
      url.pathname = "/";
      url.search = "";
      return NextResponse.redirect(url);
    }
    return NextResponse.next();
  }
  if (!pathname.startsWith("/admin")) return NextResponse.next();
  const token = process.env.ADMIN_TOKEN;
  if (!token) return new NextResponse(null, { status: 404 });
  if (pathname === "/admin/login") return NextResponse.next();
  const cookie = request.cookies.get("fe2vi_admin")?.value;
  if (cookie && cookie === (await digest(token))) return NextResponse.next();
  const url = request.nextUrl.clone();
  url.pathname = "/admin/login";
  url.search = "";
  return NextResponse.redirect(url);
}

export const config = {
  matcher: ["/((?!_next/static|_next/image).*)"],
};
