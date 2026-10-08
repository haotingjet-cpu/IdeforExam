import { Marked } from "marked";
import markedKatex from "marked-katex-extension";

// 使用獨立的 Marked 實例，避免污染全域 marked 設定。
const md = new Marked({ gfm: true, breaks: true });

const escapeAttr = (value: string) =>
  value.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

md.use({
  renderer: {
    // 連結文字走 parseInline，讓 [**粗體**](url) 之類的巢狀語法能正常渲染。
    link(token) {
      const label = this.parser.parseInline(token.tokens);
      return `<a href="${escapeAttr(token.href ?? "")}" target="_blank" rel="noopener noreferrer">${label}</a>`;
    },
    image(token) {
      return `<img src="${escapeAttr(token.href ?? "")}" alt="${escapeAttr(token.text ?? "")}" loading="lazy" decoding="async" />`;
    }
  }
});

md.use(markedKatex({
  throwOnError: false,
  output: "html",     // 只輸出 HTML 版；MathML 會被 sanitizer 移除，留著只是白白變肥
  nonStandard: true,  // 允許 "令$n$個" 這種前後沒有空白的寫法（中文幾乎都是這樣）
  trust: false,
  strict: "ignore"
}));

const SAFE_TAGS = new Set([
  "a", "abbr", "b", "blockquote", "br", "code", "del", "details", "summary", "em", "figure", "figcaption",
  "h1", "h2", "h3", "h4", "h5", "h6", "hr", "i", "img", "kbd", "li", "mark", "ol", "p", "pre", "s", "samp",
  "small", "span", "strong", "sub", "sup", "table", "tbody", "td", "th", "thead", "tr", "ul"
]);
// KaTeX 的根號、長括號等符號是用 inline SVG 畫的，只在 .katex 內放行這幾個標籤。
const KATEX_SVG_TAGS = new Set(["svg", "path", "line"]);

function resolveUrl(value: string, baseUrl?: string): string | null {
  try {
    const url = new URL(value.trim(), baseUrl);
    return url.protocol === "http:" || url.protocol === "https:" ? url.href : null;
  } catch {
    return null;
  }
}

function sanitizeHtml(html: string, baseUrl?: string): string {
  const container = document.createElement("div");
  container.innerHTML = html;

  container.querySelectorAll("script, style, iframe, object, embed, math, link, template, foreignObject").forEach((node) => node.remove());

  for (const element of Array.from(container.querySelectorAll("*"))) {
    const tag = element.localName;
    const inMath = element.closest(".katex") !== null;
    if (!SAFE_TAGS.has(tag) && !(inMath && KATEX_SVG_TAGS.has(tag))) {
      element.replaceWith(...Array.from(element.childNodes));
      continue;
    }

    for (const attribute of Array.from(element.attributes)) {
      const name = attribute.name.toLowerCase();
      if (name.startsWith("on")) { element.removeAttribute(attribute.name); continue; }
      // KaTeX 靠 inline style 排版；其餘地方不允許 style，避免 url() 之類的外連。
      if (name === "style" && !inMath) { element.removeAttribute(attribute.name); continue; }
      if (name === "href" || name === "src" || name === "xlink:href") {
        const resolved = resolveUrl(attribute.value, baseUrl);
        if (resolved) element.setAttribute(attribute.name, resolved);
        else element.removeAttribute(attribute.name);
      }
    }
  }

  return container.innerHTML;
}

// baseUrl：題目網址，用來把題目中的相對路徑圖片／連結補成完整網址。
export async function renderProblemMarkdown(source: string, baseUrl?: string): Promise<string> {
  if (!source.trim()) return "";
  const html = await md.parse(source);
  return sanitizeHtml(html, baseUrl);
}