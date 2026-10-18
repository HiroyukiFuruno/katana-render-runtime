import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { createContext, runInContext } from "node:vm";
import { MermaidRuntimeScripts } from "./official-renderer-i18n";

const NORMALIZER_SOURCES = MermaidRuntimeScripts.paths().map((filePath) =>
  readFileSync(filePath, "utf8"),
);

function requireStringResult(value: string | null | undefined): string {
  if (typeof value !== "string") {
    throw new TypeError("Expected a string result from the isolated runtime");
  }
  return value;
}

function requireStringOrNullResult(value: string | null | undefined): string | null {
  if (value === null) return null;
  if (typeof value !== "string") {
    throw new TypeError("Expected a string or null result from the isolated runtime");
  }
  return value;
}

function createNormalizer() {
  const context = createContext({});
  for (const source of NORMALIZER_SOURCES) runInContext(source, context);

  return (functionName: string, svg: string, source?: string): string | null => {
    context.svgInput = svg;
    context.requestInput = source === undefined ? undefined : { source };
    const args = source === undefined ? "svgInput" : "svgInput, requestInput";
    return requireStringOrNullResult(runInContext(`${functionName}(${args})`, context));
  };
}

const C4_TYPES = ["C4Context", "C4Container", "C4Component", "C4Dynamic", "C4Deployment"];
const OTHER_TYPES = [
  "flowchart",
  "classDiagram",
  "er",
  "stateDiagram",
  "requirement",
  "journey",
  "pie",
  "venn",
  "architecture",
  "radar",
  "gitGraph",
  "kanban",
  "sankey",
  "mindmap",
  "block",
  "timeline",
];

function svgWithFontSize(fontSize: string) {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="413" height="287" viewBox="-17 23 413 287" aria-roledescription="c4"><text x="137" y="241" style="font-size: ${fontSize};">Natural independent observer</text><rect x="9" y="17" width="31" height="43"/></svg>`;
}

test("C4 diagrams normalize general font units and preserve unrelated SVG geometry", () => {
  const normalize = createNormalizer();
  const input = svgWithFontSize("14");
  const expected = `<svg xmlns="http://www.w3.org/2000/svg" width="413" height="287" viewBox="-17 23 413 287" aria-roledescription="c4"><text x="137" y="241" style="font-size: 14px;">Natural independent observer</text><rect x="9" y="17" width="31" height="43"/></svg>`;

  for (const type of C4_TYPES) {
    expect(normalize("katanaNormalizeC4ReviewSvgInternal", input, type)).toBe(expected);
  }
});

test("existing pixel units remain idempotent without changing arbitrary geometry", () => {
  const normalize = createNormalizer();
  const input = svgWithFontSize("14px");

  for (const type of C4_TYPES) {
    expect(normalize("katanaNormalizeC4ReviewSvgInternal", input, type)).toBe(input);
  }
});

test("C4 Container review normalization preserves the independent Mermaid viewBox", () => {
  const normalize = createNormalizer();
  const rawViewBox = "0 -70 1248 861.1848754882812";
  const input = `<svg id="katana-mermaid-svg-container" viewBox="${rawViewBox}" aria-roledescription="c4"><g class="node c4-shape"></g></svg>`;

  expect(
    normalize(
      "katanaNormalizeC4RuntimeContainerReviewSvg",
      input,
      'C4Container\nContainerDb(db, "DB", "SQL")',
    ),
  ).toContain(`viewBox="${rawViewBox}"`);

  for (const type of ["C4Context", "C4Component", "C4Dynamic", "C4Deployment"]) {
    expect(normalize("katanaNormalizeC4RuntimeContainerReviewSvg", input, `${type}\nA()`)).toBe(
      input,
    );
  }
});

test("non-C4 diagram types stay outside the C4 font-unit gate", () => {
  const normalize = createNormalizer();

  for (const type of OTHER_TYPES) {
    const input = svgWithFontSize("14").replace(
      'aria-roledescription="c4"',
      `aria-roledescription="${type}"`,
    );

    expect(normalize("katanaNormalizeC4ReviewSvgInternal", input, type)).toBeNull();
    expect(normalize("katanaNormalizeC4ReviewSvg", input, type)).toBe(input);
  }
});

test("sequence review normalization preserves vendor geometry and independent root dimensions", () => {
  const normalize = createNormalizer();
  const cases = [
    {
      rootAttributes: 'width="450" height="281" viewBox="-50 -10 450 311"',
      body: '<rect class="actor actor-bottom" height="75" y="186"/><text class="actor actor-box" y="223"/><line class="messageLine0" y1="120" y2="120"/><text class="messageText" y="135"/><line class="messageLine1" y1="166" y2="166"/><rect height="777"/>',
    },
    {
      rootAttributes: 'width="501" height="373" viewBox="-50 -10 501 403"',
      body: '<rect class="actor actor-bottom" height="75" y="278"/><text class="actor actor-box" y="315"/><line class="messageLine1" y1="120" y2="120"/><text class="messageText" y="135"/><line class="messageLine0" y1="166" y2="166"/><text class="messageText" y="181"/><line class="messageLine1" y1="212" y2="212"/><text class="messageText" y="227"/><line class="messageLine0" y1="258" y2="258"/><rect height="777"/>',
    },
    {
      rootAttributes: 'height="120" viewBox="0 0 200 100"',
      body: '<line class="messageLine0" y1="10" y2="10"/><text class="messageText" y="20"/>',
    },
    {
      rootAttributes: 'height="50%" viewBox="0 0 200 100"',
      body: '<line class="messageLine0" y1="10" y2="10"/>',
    },
    {
      rootAttributes: 'height="auto" viewBox="0 0 200 100"',
      body: '<line class="messageLine0" y1="10" y2="10"/>',
    },
    {
      rootAttributes: 'viewBox="0 0 200 100"',
      body: '<line class="messageLine0" y1="10" y2="10"/>',
    },
    {
      rootAttributes: 'height="120" viewBox="0 0 200 100"',
      body: '<rect class="actor actor-bottom" y="280"/><text class="actor actor-box" y="280"/>',
    },
  ];

  for (const { rootAttributes, body } of cases) {
    const input = `<svg id="katana-mermaid-svg-sequence-geometry" ${rootAttributes} aria-roledescription="sequence">${body}</svg>`;
    const normalized = requireStringResult(
      normalize("katanaNormalizeReviewFeedbackSvg", input, "sequenceDiagram\nA->>B: message"),
    );
    expect(normalized).toBe(input);
  }
});

test("sequence-shaped classes in non-sequence SVGs remain unchanged", () => {
  const normalize = createNormalizer();
  const withMessageLine =
    '<svg id="katana-mermaid-svg-non-sequence" height="300" viewBox="0 0 200 320" aria-roledescription="flowchart"><line class="messageLine0" y1="123" y2="123"/></svg>';
  const noMessages =
    '<svg id="katana-mermaid-svg-empty-sequence" height="300" viewBox="0 0 200 320" aria-roledescription="sequence"><rect class="actor actor-bottom" y="280"/><text class="messageText" y="90"/></svg>';

  expect(
    normalize("katanaNormalizeReviewFeedbackSvg", withMessageLine, "flowchart LR\nA-->B"),
  ).toBe(withMessageLine);
  expect(
    normalize("katanaNormalizeReviewFeedbackSvg", noMessages, "sequenceDiagram\nA->>B: message"),
  ).toBe(noMessages);
});

test("mindmap normalization preserves raw geometry and edge metadata", () => {
  const normalize = createNormalizer();
  const input =
    '<svg viewBox="5 5 528.9063720703125 403.0663146972656" aria-roledescription="mindmap">' +
    '<g class="node mindmap-node" transform="translate(251.2355, 210.8662)"></g>' +
    '<path data-edge="true" d="M266.164,212.331L324.111,218.017" data-points="raw-points"/>' +
    "</svg>";
  expect(normalize("katanaNormalizeDiagramSpecificSvg", input, "mindmap")).toBe(input);
});

function htmlParserResult(expression: string, input: string, diagramType = "block"): string {
  const context = createContext({ document: {}, input, __katanaMermaidDiagramType: diagramType });
  for (const name of ["browser_globals.js", "dom_node.js", "dom_html.js"]) {
    const source = readFileSync(
      new URL(
        `../../crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime/${name}`,
        import.meta.url,
      ),
      "utf8",
    );
    runInContext(source, context);
  }
  return requireStringResult(runInContext(expression, context));
}

test("HTML parsing decodes equivalent NBSP references once for every diagram", () => {
  for (const diagramType of ["block", "classDiagram", "c4", "flowchart"]) {
    for (const input of ["&nbsp;", "&#160;", "&#xA0;", "&#XA0;", "\u00A0"]) {
      expect(htmlParserResult("parseInnerHtml(input)[0].textContent", input, diagramType)).toBe(
        "\u00A0",
      );
    }
  }
});

test("HTML parsing preserves escaped references after a single decode", () => {
  for (const input of ["&amp;nbsp;", "&#38;nbsp;", "&#x26;nbsp;"]) {
    expect(htmlParserResult("parseInnerHtml(input)[0].textContent", input)).toBe("&nbsp;");
  }
  expect(htmlParserResult("parseInnerHtml(input)[0].textContent", "&amp;amp;nbsp;")).toBe(
    "&amp;nbsp;",
  );
  expect(htmlParserResult("decodeHtmlEntities(input)", "&lt;&gt;&amp;&quot;&#39;&#xa;&#10;")).toBe(
    "<>&\"'\n\n",
  );
  expect(htmlParserResult("parseInnerHtml(input)[0].textContent", "&missing; &#x110000;")).toBe(
    "&missing; &#x110000;",
  );
});

test("HTML attributes decode once while direct text nodes preserve literal input", () => {
  for (const reference of ["&nbsp;", "&#160;", "&#xA0;", "\u00A0"]) {
    const input = `<span data-value="${reference}">value</span>`;
    expect(htmlParserResult('parseInnerHtml(input)[0].getAttribute("data-value")', input)).toBe(
      "\u00A0",
    );
  }
  for (const reference of ["&amp;nbsp;", "&#38;nbsp;", "&#x26;nbsp;"]) {
    const input = `<span data-value="${reference}">value</span>`;
    expect(htmlParserResult('parseInnerHtml(input)[0].getAttribute("data-value")', input)).toBe(
      "&nbsp;",
    );
  }
  for (const input of ["&nbsp;", "&#160;", "&#xA0;", "&amp;nbsp;", "\u00A0"]) {
    expect(htmlParserResult("new KatanaTextNode(input).textContent", input)).toBe(input);
  }
});

test("HTML parsing preserves special prototype property names as literal text and attributes", () => {
  for (const diagramType of ["block", "classDiagram", "c4", "flowchart"]) {
    for (const input of ["&constructor;", "&toString;", "&valueOf;", "&__proto__;"]) {
      expect(htmlParserResult("parseInnerHtml(input)[0].textContent", input, diagramType)).toBe(
        input,
      );
      const markup = `<span data-value="${input}">value</span>`;
      expect(
        htmlParserResult(
          'parseInnerHtml(input)[0].getAttribute("data-value")',
          markup,
          diagramType,
        ),
      ).toBe(input);
    }
  }
});

const DRAWIO_SERIALIZATION_CASES = [
  [
    "innerHTML",
    "zw&amp;ouml;lf &amp;mdash; &amp;Zeta;&amp;alpha;",
    "zw&ouml;lf &mdash; &Zeta;&alpha;",
    "zw&amp;ouml;lf &amp;mdash; &amp;Zeta;&amp;alpha;",
  ],
  ["innerHTML", "zw&ouml;lf &mdash; &Zeta;&alpha;", "zwölf — Ζα", "zwölf — Ζα"],
  [
    "textContent",
    "zw&ouml;lf &mdash; &Zeta;&alpha;",
    "zw&ouml;lf &mdash; &Zeta;&alpha;",
    "zw&amp;ouml;lf &amp;mdash; &amp;Zeta;&amp;alpha;",
  ],
] as const;

function serializationResult(expression: string, input: string): string {
  const context = createContext({ input });
  for (const name of [
    "browser_globals.js",
    "dom_node.js",
    "dom_html.js",
    "dom_node_html.js",
    "dom_layout.js",
    "dom_layout_prototype.js",
    "dom_document.js",
  ]) {
    const source = readFileSync(
      new URL(
        `../../crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime/${name}`,
        import.meta.url,
      ),
      "utf8",
    );
    runInContext(source, context);
  }
  return requireStringResult(runInContext(expression, context));
}

test("innerHTML escapes own literal text once and preserves TextNode serialization", () => {
  for (const [input, serialized] of [
    ["a&<b> &ouml;", "a&amp;&lt;b&gt; &amp;ouml;"],
    ["<![CDATA[a & b < c]]>", "&lt;![CDATA[a &amp; b &lt; c]]&gt;"],
  ] as const) {
    const result = serializationResult(
      `(() => {
      const div = document.createElement("div"); div.textContent = input;
      const text = document.createTextNode(input);
      return JSON.stringify([div.textContent, div.innerHTML, div.outerHTML, text.outerHTML]);
    })()`,
      input,
    );
    expect(result).toBe(
      JSON.stringify([input, serialized, `<div>${serialized}</div>`, serialized]),
    );
  }
});

test("innerHTML retains own text before appended br and hr for all Drawio entity cases", () => {
  for (const [property, input, text, serialized] of DRAWIO_SERIALIZATION_CASES) {
    const result = serializationResult(
      `(() => {
      const div = document.createElement("div"); div.${property} = input;
      div.setAttribute("data-entity-text", div.textContent);
      div.appendChild(document.createElement("br")); div.appendChild(document.createElement("hr"));
      return JSON.stringify([div.textContent, div.innerHTML, div.outerHTML]);
    })()`,
      input,
    );
    expect(result).toBe(
      JSON.stringify([
        text,
        `${serialized}<br/><hr/>`,
        `<div data-entity-text="${serialized}">${serialized}<br/><hr/></div>`,
      ]),
    );
  }
});

test("innerHTML serializes own and child text in order without duplicating or double escaping", () => {
  const result = serializationResult(
    `(() => {
    const div = document.createElement("div"); div.textContent = input;
    div.appendChild(document.createTextNode(" child&"));
    const span = document.createElement("span"); span.textContent = " nested<"; div.appendChild(span);
    const before = div.innerHTML; const text = div.textContent;
    div.innerHTML = before;
    return JSON.stringify([text, before, div.textContent, div.innerHTML]);
  })()`,
    "own&ouml;",
  );
  expect(result).toBe(
    JSON.stringify([
      "own&ouml; child& nested<",
      "own&amp;ouml; child&amp;<span> nested&lt;</span>",
      "own&ouml; child& nested<",
      "own&amp;ouml; child&amp;<span> nested&lt;</span>",
    ]),
  );
});

test("innerHTML preserves own text and style through shallow and deep element clones", () => {
  const result = serializationResult(
    `(() => {
    const div = document.createElement("div"); div.textContent = input;
    div.style.setProperty("color", "red");
    div.appendChild(document.createElement("br")); div.appendChild(document.createElement("hr"));
    const shallow = div.cloneNode(); const deep = div.cloneNode(true);
    const style = document.createElement("style"); style.textContent = input;
    return JSON.stringify([shallow.innerHTML, deep.innerHTML, deep.outerHTML, style.outerHTML]);
  })()`,
    "a&<b>",
  );
  expect(result).toBe(
    JSON.stringify([
      "a&amp;&lt;b&gt;",
      "a&amp;&lt;b&gt;<br/><hr/>",
      '<div style="color: red;">a&amp;&lt;b&gt;<br/><hr/></div>',
      "<style>a&amp;&lt;b&gt;</style>",
    ]),
  );
});

test("block normalization preserves literal SVG text and actual NBSP without HTML reinterpretation", () => {
  const normalize = createNormalizer();
  for (const diagramType of ["block", "flowchart", "classDiagram"]) {
    for (const text of [
      "&amp;nbsp;",
      "&amp;amp;nbsp;",
      "&#38;nbsp;",
      "&amp;#160;",
      "&amp;#xA0;",
      "&amp;ouml;",
      "&amp;amp;ouml;",
      "\u00a0",
      "&#160;",
      "&#xA0;",
    ]) {
      const input = `<svg aria-roledescription="${diagramType}" viewBox="-7 9 137 53"><text x="17" y="29">${text}</text><rect x="11" y="13" width="23" height="31"/><path d="M19,23L37,41"/></svg>`;
      expect(normalize("katanaNormalizeBlockSvg", input)).toBe(input);
      expect(
        normalize("katanaNormalizeBlockSvg", input, "block-beta\nrenamed_id[changed label]"),
      ).toBe(input);
    }
  }
});
