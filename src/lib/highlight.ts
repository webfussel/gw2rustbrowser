import type { HLJSApi } from "highlight.js";

/** highlight.js language per text category (the last folder of a text file). */
const languages: Record<string, string> = {
  HTML: "xml",
  XML: "xml",
  JavaScript: "javascript",
  CSS: "css",
  JSON: "json",
  PHP: "php",
  Documentation: "markdown",
  ActionScript: "actionscript",
};

/** Language of a text file from its category path, if it is code or markup. */
export function languageFor(path: string[] | undefined): string | null {
  return (path && languages[path[path.length - 1]]) ?? null;
}

let core: Promise<HLJSApi> | null = null;

// highlight.js is loaded only when a code file is shown.
function load(): Promise<HLJSApi> {
  core ??= Promise.all([
    import("highlight.js/lib/core"),
    import("highlight.js/lib/languages/xml"),
    import("highlight.js/lib/languages/javascript"),
    import("highlight.js/lib/languages/css"),
    import("highlight.js/lib/languages/json"),
    import("highlight.js/lib/languages/php"),
    import("highlight.js/lib/languages/markdown"),
    import("highlight.js/lib/languages/actionscript"),
  ]).then(([{ default: hljs }, xml, javascript, css, json, php, markdown, actionscript]) => {
    hljs.registerLanguage("xml", xml.default);
    hljs.registerLanguage("javascript", javascript.default);
    hljs.registerLanguage("css", css.default);
    hljs.registerLanguage("json", json.default);
    hljs.registerLanguage("php", php.default);
    hljs.registerLanguage("markdown", markdown.default);
    hljs.registerLanguage("actionscript", actionscript.default);
    return hljs;
  });
  return core;
}

/** The text as HTML with highlight.js token spans (input is escaped). */
export async function highlight(text: string, language: string): Promise<string> {
  const hljs = await load();
  return hljs.highlight(text, { language, ignoreIllegals: true }).value;
}
