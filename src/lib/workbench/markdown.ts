import { marked } from "marked";
import markedKatex from "marked-katex-extension";

const renderer = new marked.Renderer();

renderer.link = (token) => {
  const href = token.href ?? "";
  const label = token.text ?? href;
  return `<a href="${href}" target="_blank" rel="noopener noreferrer">${label}</a>`;
};

renderer.image = (token) => {
  const src = token.href ?? "";
  const alt = token.text ?? "";
  return `<img src="${src}" alt="${alt}" loading="lazy" decoding="async" />`;
};

marked.use({ renderer });
marked.use(markedKatex({
  throwOnError: false,
  output: "htmlAndMathml",
  displayMode: false,
  trust: false,
  strict: "warn",
  macros: {}
}));

const SAFE_TAGS = new Set(["A", "ABBR", "B", "BLOCKQUOTE", "BR", "CODE", "DEL", "DETAILS", "EM", "FIGURE", "H1", "H2", "H3", "H4", "H5", "H6", "HR", "I", "IMG", "KBD", "LI", "MARK", "OL", "P", "PRE", "S", "SAMP", "SMALL", "SPAN", "STRONG", "SUB", "SUP", "TABLE", "TBODY", "TD", "TH", "THEAD", "TR", "UL"]);

function sanitizeHtml(html: string): string {
  const container = document.createElement("div");
  container.innerHTML = html;

  container.querySelectorAll("script, style, iframe, object, embed, svg, math, link, template").forEach((node) => node.remove());

  container.querySelectorAll("*").forEach((element) => {
    if (!SAFE_TAGS.has(element.tagName)) {
      const children = Array.from(element.childNodes);
      if (children.length > 0) {
        element.replaceWith(...children);
      } else {
        element.remove();
      }
      return;
    }

    for (const attribute of Array.from(element.attributes)) {
      const name = attribute.name.toLowerCase();
      if (name.startsWith("on")) element.removeAttribute(attribute.name);
      if (name === "href" || name === "src") {
        const value = attribute.value.trim();
        if (!/^https?:\/\//i.test(value)) element.removeAttribute(attribute.name);
      }
    }
  });

  return container.innerHTML;
}

export async function renderProblemMarkdown(source: string): Promise<string> {
  if (!source.trim()) return "";
  const html = await marked.parse(source, { breaks: true, gfm: true });
  return sanitizeHtml(html);
}
