"use client";

import type { ReactNode } from "react";

function stripFrontmatter(source: string) {
  const text = source.replace(/^\uFEFF/, "");
  if (!text.startsWith("---\n")) return text;
  const end = text.indexOf("\n---\n", 4);
  return end === -1 ? text : text.slice(end + 5);
}

function inline(text: string) {
  const parts = text.split(/(\[[^\]]+\]\([^)]+\)|`[^`]+`|\*\*[^*]+\*\*)/g);
  return parts.map((part, index) => {
    const link = part.match(/^\[([^\]]+)\]\(([^)]+)\)$/);
    if (link) {
      return (
        <a key={index} href={link[2]}>
          {link[1]}
        </a>
      );
    }
    if (part.startsWith("`") && part.endsWith("`")) {
      return <code key={index}>{part.slice(1, -1)}</code>;
    }
    if (part.startsWith("**") && part.endsWith("**")) {
      return <strong key={index}>{part.slice(2, -2)}</strong>;
    }
    return <span key={index}>{part}</span>;
  });
}

function CopyBlock({ text }: { text: string }) {
  return (
    <div className="md-code">
      <button
        type="button"
        className="btn btn-secondary"
        aria-label="Copy code"
        onClick={() => {
          void navigator.clipboard?.writeText(text);
        }}
      >
        Copy
      </button>
      <pre>
        <code>{text}</code>
      </pre>
    </div>
  );
}

/** Small markdown for the agent guide. Headings, lists, fences, links. */
export function MarkdownDoc({ source }: { source: string }) {
  const lines = stripFrontmatter(source).replace(/\r\n/g, "\n").split("\n");
  const blocks: ReactNode[] = [];
  let i = 0;
  let key = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (line.startsWith("```")) {
      const buf: string[] = [];
      i += 1;
      while (i < lines.length && !lines[i].startsWith("```")) {
        buf.push(lines[i]);
        i += 1;
      }
      i += 1;
      blocks.push(<CopyBlock key={key += 1} text={buf.join("\n")} />);
      continue;
    }
    if (line.startsWith("### ")) {
      blocks.push(<h3 key={key += 1}>{inline(line.slice(4))}</h3>);
      i += 1;
      continue;
    }
    if (line.startsWith("## ")) {
      blocks.push(<h2 key={key += 1}>{inline(line.slice(3))}</h2>);
      i += 1;
      continue;
    }
    if (line.startsWith("# ")) {
      blocks.push(<h1 key={key += 1}>{inline(line.slice(2))}</h1>);
      i += 1;
      continue;
    }
    if (/^\d+\. /.test(line)) {
      const items: string[] = [];
      while (i < lines.length && /^\d+\. /.test(lines[i])) {
        items.push(lines[i].replace(/^\d+\. /, ""));
        i += 1;
      }
      blocks.push(
        <ol key={key += 1}>
          {items.map((item) => (
            <li key={item}>{inline(item)}</li>
          ))}
        </ol>,
      );
      continue;
    }
    if (line.startsWith("- ")) {
      const items: string[] = [];
      while (i < lines.length && lines[i].startsWith("- ")) {
        items.push(lines[i].slice(2));
        i += 1;
      }
      blocks.push(
        <ul key={key += 1}>
          {items.map((item) => (
            <li key={item}>{inline(item)}</li>
          ))}
        </ul>,
      );
      continue;
    }
    if (line.trim() === "") {
      i += 1;
      continue;
    }
    const para: string[] = [line];
    i += 1;
    while (
      i < lines.length &&
      lines[i].trim() !== "" &&
      !lines[i].startsWith("#") &&
      !lines[i].startsWith("- ") &&
      !/^\d+\. /.test(lines[i]) &&
      !lines[i].startsWith("```")
    ) {
      para.push(lines[i]);
      i += 1;
    }
    blocks.push(<p key={key += 1}>{inline(para.join(" "))}</p>);
  }
  return <article className="md-doc">{blocks}</article>;
}
