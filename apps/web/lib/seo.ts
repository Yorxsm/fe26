import type { Metadata } from "next";

export const SITE = "https://fe2vi.space";
export const OG_IMAGE = "/fe2vi-logo-400.png";

export function pageMetadata(title: string, description: string, path: string): Metadata {
  const full = title === "fe2VI" ? title : `${title} · fe2VI`;
  return {
    title: full,
    description,
    alternates: { canonical: path },
    openGraph: {
      title: full,
      description,
      url: path,
      images: [{ url: OG_IMAGE, width: 400, height: 400, alt: "fe2VI" }],
    },
    twitter: {
      card: "summary_large_image",
      title: full,
      description,
      images: [OG_IMAGE],
    },
  };
}
