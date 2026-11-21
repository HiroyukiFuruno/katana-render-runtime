import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { DOMParser, XMLSerializer, type Element as XmlElement } from "@xmldom/xmldom";
import { type ClassReferenceLanguage, inspectClassReference } from "./class-reference-integrity";

const fixtures: Record<ClassReferenceLanguage, string> = {
  ja: "../../tests/fixtures/mermaid/structural-regression/ja/03-02-class-diagram.svg",
  en: "../../tests/fixtures/mermaid/structural-regression/en/03-02-class-diagram-inheritance.svg",
};
const current = (language: ClassReferenceLanguage) =>
  readFileSync(new URL(fixtures[language], import.meta.url), "utf8");
const elements = (root: XmlElement, tag: string) => Array.from(root.getElementsByTagName(tag));
const group = (element: XmlElement, className: string) =>
  (element.getAttribute("class") ?? "").split(/\s+/).includes(className);

function mutate(change: (root: XmlElement) => void): string {
  const document = new DOMParser().parseFromString(current("en"), "image/svg+xml");
  const root = document.documentElement as XmlElement;
  change(root);
  return new XMLSerializer().serializeToString(root);
}

function rejects(svg: string, reason: string) {
  const result = inspectClassReference(svg, "en");
  expect(result.accepted).toBe(false);
  expect(result.reasons.join("\n")).toContain(reason);
}

for (const language of ["ja", "en"] as const) {
  test(`${language} 公式継承fixtureは4box・全5member/6method・3hollow親edgeを持つ`, () => {
    const result = inspectClassReference(current(language), language);
    expect(result.reasons).toEqual([]);
    expect(result.accepted).toBe(true);
    expect(result.nodes).toHaveLength(4);
    expect(result.nodes.flatMap((node) => node.anchors)).toHaveLength(15);
    expect(result.edges).toBe(3);
  });
}

test("同じ壊れたreferenceとcandidateは各々rejectされる", () => {
  const broken = mutate((root) => {
    const nodes = elements(root, "g").filter((element) => group(element, "node"));
    for (const node of nodes) node.setAttribute("transform", "translate(0,0)");
  });
  rejects(broken, "invalid:node-overlap");
  expect([broken, broken].map((svg) => inspectClassReference(svg, "en").accepted)).toEqual([
    false,
    false,
  ]);
});

test("memberを1つ失った同寸法SVGをrejectする", () => {
  const broken = mutate((root) => {
    const members = elements(root, "g").find((element) =>
      group(element, "members-group"),
    ) as XmlElement;
    const text = elements(members, "text")[0] as XmlElement;
    text.parentNode?.removeChild(text);
  });
  rejects(broken, "invalid:content:members-group");
});

test("node outerboxが重なるworld配置をrejectする", () => {
  rejects(
    mutate((root) => {
      const nodes = elements(root, "g").filter((element) => group(element, "node"));
      nodes[1]?.setAttribute("transform", nodes[0]?.getAttribute("transform") ?? "");
    }),
    "invalid:node-overlap",
  );
});

test("親のhollowmarkerを塗りつぶすとrejectする", () => {
  rejects(
    mutate((root) => {
      const edge = elements(root, "path").find(
        (element) => element.getAttribute("data-edge") === "true",
      ) as XmlElement;
      const id = edge.getAttribute("marker-start")?.match(/#([^)]*)/)?.[1];
      const marker = elements(root, "marker").find(
        (element) => element.getAttribute("id") === id,
      ) as XmlElement;
      const shape = Array.from(marker.childNodes).find(
        (child) => child.nodeType === 1,
      ) as XmlElement;
      shape.setAttribute("style", "fill:red !important;stroke-width:2");
    }),
    "invalid:marker-not-hollow",
  );
});

test("親edgeを1本失うとrejectする", () => {
  rejects(
    mutate((root) => {
      const edge = elements(root, "path").find(
        (element) => element.getAttribute("data-edge") === "true",
      ) as XmlElement;
      edge.parentNode?.removeChild(edge);
    }),
    "invalid:edge-count",
  );
});

test("text anchorがboxを出るとrejectする", () => {
  rejects(
    mutate((root) => {
      const label = elements(root, "g").find((element) =>
        group(element, "label-group"),
      ) as XmlElement;
      label.setAttribute("transform", "translate(100000,0)");
    }),
    "invalid:text-anchor-outside",
  );
});

test("累積worldtranslateとpositive scaleは図の相対契約を維持する", () => {
  const svg = mutate((root) => {
    root.setAttribute("transform", "translate(120,-50) scale(2,3)");
  });
  expect(inspectClassReference(svg, "en").reasons).toEqual([]);
  const local = inspectClassReference(current("en"), "en").nodes[0]?.box;
  const world = inspectClassReference(svg, "en").nodes[0]?.box;
  expect(world?.left).toBe((local?.left ?? 0) * 2 + 120);
  expect(world?.top).toBe((local?.top ?? 0) * 3 - 50);
});

test("未知transformは未対応としてrejectする", () => {
  rejects(
    mutate((root) => root.setAttribute("transform", "rotate(10)")),
    "unsupported:transform",
  );
});

test("未知arc edge schemaは未対応としてrejectする", () => {
  rejects(
    mutate((root) => {
      const edge = elements(root, "path").find(
        (element) => element.getAttribute("data-edge") === "true",
      ) as XmlElement;
      edge.setAttribute("d", "M0,0 A20,20 0 0,1 30,30");
    }),
    "unsupported:path-command:A",
  );
});

test("parent endpointがmarker実寸法から外れるとrejectする", () => {
  rejects(
    mutate((root) => {
      const edge = elements(root, "path").find(
        (element) => element.getAttribute("data-edge") === "true",
      ) as XmlElement;
      edge.setAttribute("transform", "translate(0,100000)");
    }),
    "invalid:edge-endpoint",
  );
});

test("child endpointはnode境界から離れるとrejectする", () => {
  rejects(
    mutate((root) => {
      const edge = elements(root, "path").find(
        (element) => element.getAttribute("data-edge") === "true",
      ) as XmlElement;
      edge.setAttribute("d", `${edge.getAttribute("d")} l0,1`);
    }),
    "invalid:edge-endpoint",
  );
});

test("method欠落をmemberとは独立にrejectする", () => {
  rejects(
    mutate((root) => {
      const methods = elements(root, "g").find((element) =>
        group(element, "methods-group"),
      ) as XmlElement;
      const text = elements(methods, "text")[0] as XmlElement;
      text.parentNode?.removeChild(text);
    }),
    "invalid:content:methods-group",
  );
});

test("隠されたmemberの文字列と座標が残ってもrejectする", () => {
  rejects(
    mutate((root) => {
      const members = elements(root, "g").find((element) =>
        group(element, "members-group"),
      ) as XmlElement;
      members.setAttribute("style", "display:none");
    }),
    "invalid:element-invisible",
  );
});

test("親nodeが子の下にある配置をrejectする", () => {
  rejects(
    mutate((root) => {
      const node = elements(root, "g").find((element) => group(element, "node")) as XmlElement;
      node.setAttribute("transform", "translate(0,100000)");
    }),
    "invalid:parent-not-above",
  );
});

test("未対応CSStransformと不正XMLを未対応でrejectする", () => {
  rejects(
    mutate((root) => root.setAttribute("style", "transform:rotate(0deg)")),
    "unsupported:css-transform",
  );
  rejects(current("en").replace("</svg>", "</g>"), "unsupported:xml");
});

test("軸に平行なworldmatrixと色変更は正しい構造を保つ", () => {
  const svg = mutate((root) => {
    root.setAttribute("transform", "matrix(2,0,0,3,120,-50)");
    for (const style of elements(root, "style"))
      style.textContent = (style.textContent ?? "").replace(/#[a-fA-F0-9]{6}\b/g, "#123456");
  });
  expect(inspectClassReference(svg, "en").reasons).toEqual([]);
});

for (const target of ["ancestor", "shape"] as const) {
  for (const style of ["display:none", "visibility:hidden", "opacity:0"]) {
    test(`outerbox ${target} ${style} をrejectする`, () => {
      rejects(
        mutate((root) => {
          const outer = elements(root, "g").find((element) =>
            group(element, "outer-path"),
          ) as XmlElement;
          const shape = Array.from(outer.childNodes).find(
            (child) => child.nodeType === 1,
          ) as XmlElement;
          (target === "ancestor" ? outer : shape).setAttribute("style", style);
        }),
        "invalid:element-invisible",
      );
    });
  }
}

for (const property of ["clip-path", "mask", "mask-image"] as const) {
  test(`outerbox ${property}による消失は未対応としてrejectする`, () => {
    rejects(
      mutate((root) => {
        const outer = elements(root, "g").find((element) =>
          group(element, "outer-path"),
        ) as XmlElement;
        const document = root.ownerDocument;
        if (!document) throw new Error("SVG document missing");
        const effect = document.createElementNS(
          "http://www.w3.org/2000/svg",
          property === "mask" ? "mask" : "clipPath",
        );
        effect.setAttribute("id", "empty-box-effect");
        root.appendChild(effect);
        outer.setAttribute("style", `${property}:url(#empty-box-effect)`);
      }),
      `unsupported:svg-tag:${property === "mask" ? "mask" : "clipPath"}`,
    );
  });
}

for (const style of [
  "stroke:none!important",
  "stroke:transparent!important",
  "stroke:rgba(20,30,40,0)!important",
  "stroke-width:0!important",
  "stroke-opacity:0!important",
]) {
  test(`inheritance edge ${style} は不可視としてrejectする`, () => {
    rejects(
      mutate((root) => {
        const edge = elements(root, "path").find(
          (element) => element.getAttribute("data-edge") === "true",
        ) as XmlElement;
        edge.setAttribute("style", style);
      }),
      "invalid:edge-invisible-stroke",
    );
  });
}

test("inheritance edge祖先が非表示ならrejectする", () => {
  rejects(
    mutate((root) => {
      const edge = elements(root, "path").find(
        (element) => element.getAttribute("data-edge") === "true",
      ) as XmlElement;
      (edge.parentNode as XmlElement).setAttribute("style", "display:none");
    }),
    "invalid:element-invisible",
  );
});

test("outerbox fillとstrokeが両方noneならgeometryが残ってもrejectする", () => {
  rejects(
    mutate((root) => {
      const outer = elements(root, "g").find((element) =>
        group(element, "outer-path"),
      ) as XmlElement;
      const shape = Array.from(outer.childNodes).find(
        (child) => child.nodeType === 1,
      ) as XmlElement;
      shape.setAttribute("style", "fill:none!important;stroke:none!important");
    }),
    "invalid:outer-box-invisible-paint",
  );
});

test("未知paint serverは可視と仮定しない", () => {
  rejects(
    mutate((root) => {
      const edge = elements(root, "path").find(
        (element) => element.getAttribute("data-edge") === "true",
      ) as XmlElement;
      edge.setAttribute("style", "stroke:url(#unsupported-paint)!important");
    }),
    "unsupported:paint",
  );
});

test("inheritance edgeにstroke指定がなければrejectする", () => {
  rejects(
    mutate((root) => {
      for (const style of elements(root, "style"))
        style.textContent = (style.textContent ?? "").replace(/(?<![\w-])stroke:[^;{}]*;?/g, "");
    }),
    "invalid:edge-invisible-stroke",
  );
});

for (const color of [
  "rgb(0deg,0,0)",
  "rgb(0,0,0/1)",
  "hsl(0,0,0)",
  "rgb(0 0 0 1)",
  "rgba(0,,0,1)",
  "rgb(0%,0,0)",
  "rgb(0 0 0 /)",
]) {
  test(`不正functional paint ${color}を可視と認定しない`, () => {
    rejects(
      mutate((root) => {
        const edge = elements(root, "path").find(
          (element) => element.getAttribute("data-edge") === "true",
        ) as XmlElement;
        edge.setAttribute("style", `stroke:none!important;stroke:${color}!important`);
      }),
      "unsupported:color",
    );
  });
}

for (const color of [
  "rgb(0,20,40)",
  "rgba(0,20,40,0.5)",
  "rgb(0 20 40 / 50%)",
  "rgb(0% 20% 40%)",
  "hsl(0,20%,40%)",
  "hsla(0deg,20%,40%,0.5)",
  "hsl(0deg 20% 40% / 50%)",
]) {
  test(`標準functional paint ${color}は特定色に縛らず可視`, () => {
    const svg = mutate((root) => {
      const edge = elements(root, "path").find(
        (element) => element.getAttribute("data-edge") === "true",
      ) as XmlElement;
      edge.setAttribute("style", `STROKE:${color}!IMPORTANT;STROKE-WIDTH:2`);
    });
    expect(inspectClassReference(svg, "en").reasons).toEqual([]);
  });
}

test("大文字DISPLAY:noneも外枠の非表示としてrejectする", () => {
  rejects(
    mutate((root) => {
      const outer = elements(root, "g").find((element) =>
        group(element, "outer-path"),
      ) as XmlElement;
      outer.setAttribute("style", "DISPLAY:none");
    }),
    "invalid:element-invisible",
  );
});

test("大文字STROKE:none!importantも継承線を不可視にする", () => {
  rejects(
    mutate((root) => {
      const edge = elements(root, "path").find(
        (element) => element.getAttribute("data-edge") === "true",
      ) as XmlElement;
      edge.setAttribute("style", "STROKE:none!important");
    }),
    "invalid:edge-invisible-stroke",
  );
});

test("outer path display:NoNeも正規CSS非表示としてrejectする", () => {
  rejects(
    mutate((root) => {
      const outer = elements(root, "g").find((element) =>
        group(element, "outer-path"),
      ) as XmlElement;
      (elements(outer, "path")[0] as XmlElement).setAttribute("style", "display:NoNe!important");
    }),
    "invalid:element-invisible",
  );
});

test("root filter:opacity(0)の描画効果は未対応としてrejectする", () => {
  rejects(
    mutate((root) => root.setAttribute("style", "filter:opacity(0)!important")),
    "unsupported:visibility-effect:filter",
  );
});

for (const method of ["inline", "stylesheet"] as const) {
  test(`継承edge marker-start:none ${method}の実効CSSをrejectする`, () => {
    rejects(
      mutate((root) => {
        const edge = elements(root, "path").find((element) =>
          element.hasAttribute("marker-start"),
        ) as XmlElement;
        if (method === "inline") edge.setAttribute("style", "marker-start:none!important");
        if (method === "stylesheet") {
          const style = elements(root, "style")[0] as XmlElement;
          style.textContent = `${style.textContent ?? ""} path { MARKER-START:none!important; }`;
        }
      }),
      "invalid:marker-reference",
    );
  });
}

test("全文text fillとstrokeがnoneなら文字消失をrejectする", () => {
  rejects(
    mutate((root) => {
      for (const text of elements(root, "text"))
        text.setAttribute("style", "fill:none!important;stroke:none!important");
    }),
    "invalid:text-invisible-paint",
  );
});

test("表示文字tspan全部のopacity:0をrejectする", () => {
  rejects(
    mutate((root) => {
      for (const span of elements(root, "tspan")) span.setAttribute("style", "opacity:0!important");
    }),
    "invalid:element-invisible",
  );
});

test("表示文字tspanのpaint消失をrejectする", () => {
  rejects(
    mutate((root) => {
      for (const span of elements(root, "tspan"))
        span.setAttribute("style", "fill:none!important;stroke:none!important");
    }),
    "invalid:text-invisible-paint",
  );
});

test("ancestor marker noneはedge自身のmarker指定を上書きしない", () => {
  const svg = mutate((root) => {
    const edge = elements(root, "path").find((element) =>
      element.hasAttribute("marker-start"),
    ) as XmlElement;
    (edge.parentNode as XmlElement).setAttribute("style", "marker-start:none!important");
  });
  expect(inspectClassReference(svg, "en").accepted).toBe(true);
});

for (const filter of [
  "drop-shadow(-3px 7px 4px #abcd)",
  "drop-shadow(0 0 transparent)",
  "drop-shadow(9px -2px 1px rgb(20 40 60 / 30%))",
]) {
  test(`sourceを残す標準filter ${filter}は固定値に縛らず支持する`, () => {
    const svg = mutate((root) => root.setAttribute("style", `filter:${filter}!important`));
    expect(inspectClassReference(svg, "en").accepted).toBe(true);
  });
}

for (const filter of [
  "url(#hidden)",
  "drop-shadow(1px 2px black) opacity(0)",
  "drop-shadow(1deg 2px black)",
  "drop-shadow(1px 2px -1px black)",
  "drop-shadow(1px 2px rgb(0deg,0,0))",
  "drop-shadow(1px 2px none)",
]) {
  test(`未対応または不正filter ${filter}はrejectする`, () => {
    rejects(
      mutate((root) => root.setAttribute("style", `filter:${filter}!important`)),
      "unsupported:",
    );
  });
}

test("invalid displayの後置でnoneを覆うCSSは未対応としてrejectする", () => {
  rejects(
    mutate((root) =>
      root.setAttribute("style", "display:none!important;display:made-up!important"),
    ),
    "unsupported:display-value",
  );
});

test("marker shorthandの非表示指定は未対応としてrejectする", () => {
  rejects(
    mutate((root) => {
      const edge = elements(root, "path").find((element) =>
        element.hasAttribute("marker-start"),
      ) as XmlElement;
      edge.setAttribute("style", "marker:none!important");
    }),
    "unsupported:marker-shorthand",
  );
});

for (const property of [
  "d",
  "x",
  "y",
  "dx",
  "dy",
  "width",
  "height",
  "cx",
  "cy",
  "r",
  "rx",
  "ry",
]) {
  test(`属性計測に対応しないCSS geometry ${property}はrejectする`, () => {
    rejects(
      mutate((root) => {
        const edge = elements(root, "path").find((element) =>
          element.hasAttribute("marker-start"),
        ) as XmlElement;
        edge.setAttribute(
          "style",
          `${property}:${property === "d" ? 'path("M0 0L0 0")' : "0px"}!important`,
        );
      }),
      `unsupported:css-geometry:${property}`,
    );
  });
}

test("stylesheetが属性path geometryを覆う場合もrejectする", () => {
  rejects(
    mutate((root) => {
      const style = elements(root, "style")[0] as XmlElement;
      style.textContent = `${style.textContent ?? ""} path { d:path("M0 0L0 0")!important; }`;
    }),
    "unsupported:css-geometry:d",
  );
});

for (const target of ["edge", "marker"] as const) {
  test(`${target}の全gapを含む非solid dashはrejectする`, () => {
    rejects(
      mutate((root) => {
        const edge = elements(root, "path").find((element) =>
          element.hasAttribute("marker-start"),
        ) as XmlElement;
        const markerId = edge.getAttribute("marker-start")?.match(/^url\(#([^()]+)\)$/)?.[1];
        const marker = elements(root, "marker").find(
          (element) => element.getAttribute("id") === markerId,
        ) as XmlElement;
        const element =
          target === "edge"
            ? edge
            : ([...elements(marker, "path"), ...elements(marker, "polygon")][0] as XmlElement);
        element.setAttribute(
          "style",
          "stroke-dasharray:0 100000!important;stroke-linecap:butt!important",
        );
      }),
      "unsupported:stroke-nonsolid-dash",
    );
  });
}

for (const dash of ["none", "NoNe", "0", "0,0", "0px 0%", "-0.0 0e0", "0 0 27 0", "9px 0%"]) {
  test(`dasharray ${dash}は標準solid規約として支持する`, () => {
    const svg = mutate((root) => {
      for (const edge of elements(root, "path").filter((element) =>
        element.hasAttribute("marker-start"),
      ))
        edge.setAttribute("style", `stroke-dasharray:${dash}!important`);
      for (const marker of elements(root, "marker"))
        for (const shape of [...elements(marker, "path"), ...elements(marker, "polygon")])
          shape.setAttribute("style", `stroke-dasharray:${dash}!important`);
    });
    expect(inspectClassReference(svg, "en").accepted).toBe(true);
  });
}

for (const declaration of [
  "animation:disappear 1s forwards!important",
  "transition:all 1s!important",
  "vector-effect:non-scaling-stroke!important",
  "font:0px sans-serif!important",
]) {
  test(`実適用の未対応宣言 ${declaration}はrejectする`, () => {
    rejects(
      mutate((root) => root.setAttribute("style", declaration)),
      "unsupported:",
    );
  });
}

test("未使用animationとgeometry selectorをfixture内容へ適用しない", () => {
  const svg = mutate((root) => {
    const style = elements(root, "style")[0] as XmlElement;
    style.textContent = `${style.textContent ?? ""} .unused-structure {animation:disappear 1s;height:0;}`;
  });
  expect(inspectClassReference(svg, "en").accepted).toBe(true);
});

test("非一致selectorの未知後半を早期一致失敗で無視しない", () => {
  rejects(
    mutate((root) => {
      const style = elements(root, "style")[0] as XmlElement;
      style.textContent = `${style.textContent ?? ""} .unused-structure:hover {opacity:0;}`;
    }),
    "unsupported:css-selector",
  );
});

test("SVG animateMotionは未対応schemaとしてrejectする", () => {
  rejects(
    current("en").replace("</svg>", '<animateMotion path="M0 0 L1 1"/></svg>'),
    "unsupported:svg-tag:animateMotion",
  );
});

test("stylesheet PIは未対応外部入力としてrejectする", () => {
  rejects(
    `<?xml-stylesheet type="text/css" href="hidden.css"?>${current("en")}`,
    "unsupported:xml-processing-instruction",
  );
});

test("foreign namespaceのpathは見た目の名前で受け入れない", () => {
  rejects(
    mutate((root) => {
      const edge = elements(root, "path").find((element) =>
        element.hasAttribute("marker-start"),
      ) as XmlElement;
      const document = root.ownerDocument;
      if (!document) throw new Error("fixture document absent");
      const foreign = document.createElementNS("urn:other-svg", "path");
      for (const attribute of Array.from(edge.attributes))
        foreign.setAttribute(attribute.name, attribute.value);
      edge.parentNode?.replaceChild(foreign, edge);
    }),
    "unsupported:svg-namespace",
  );
});

test("未知paint属性を無視しない", () => {
  rejects(
    mutate((root) => root.setAttribute("paint-order", "markers stroke fill")),
    "unsupported:svg-attribute:paint-order",
  );
});

test("全文font-size:0もpaintがあるだけでは合格にしない", () => {
  rejects(
    mutate((root) => {
      for (const text of elements(root, "text"))
        text.setAttribute("style", "font-size:0!important");
    }),
    "unsupported:font-size-positive",
  );
});

for (const property of ["clip-path", "mask", "mask-image"]) {
  test(`CSS ${property}の参照は未対応effectとしてrejectする`, () => {
    rejects(
      mutate((root) => root.setAttribute("style", `${property}:url(#missing-effect)!important`)),
      `unsupported:visibility-effect:${property}`,
    );
  });
}

function nodeCascadeSvg(stylesheet: (id: string) => string, inline?: string): string {
  return mutate((root) => {
    const node = elements(root, "g").find((element) => group(element, "node")) as XmlElement;
    const id = node.getAttribute("id") ?? "";
    expect(id.length).toBeGreaterThan(0);
    node.setAttribute("data-cascade", "#.#.#.#.#.#.#.#.#.#.#.");
    if (inline) node.setAttribute("style", inline);
    const style = elements(root, "style")[0] as XmlElement;
    style.textContent = `${style.textContent ?? ""} ${stylesheet(id)}`;
  });
}

for (const importance of ["", "!important"]) {
  test(`ID非表示は11classの後置表示より強い ${importance}`, () => {
    rejects(
      nodeCascadeSvg(
        (id) =>
          `#${id}{display:none${importance}}${".node".repeat(11)}{display:inline${importance}}`,
      ),
      "invalid:element-invisible",
    );
  });
}

test("inline重要非表示は多数IDの重要表示より強い", () => {
  rejects(
    nodeCascadeSvg(
      (id) => `${`#${id}`.repeat(11)}{display:inline!important}`,
      "display:none!important",
    ),
    "invalid:element-invisible",
  );
});

test("stylesheet重要非表示はinline通常表示より強い", () => {
  rejects(
    nodeCascadeSvg((id) => `#${id}{display:none!important}`, "display:inline"),
    "invalid:element-invisible",
  );
});

test("属性value内のIDとclass記号は詳細度へ加算しない", () => {
  rejects(
    nodeCascadeSvg(
      (id) =>
        `#${id}{display:none!important}[data-cascade="#.#.#.#.#.#.#.#.#.#.#."]{display:inline!important}`,
    ),
    "invalid:element-invisible",
  );
});

test("class非表示は多数tagの表示より強い", () => {
  rejects(
    mutate((root) => {
      const node = elements(root, "g").find((element) => group(element, "node")) as XmlElement;
      const document = root.ownerDocument;
      if (!document) throw new Error("test-svg-document");
      node.setAttribute("class", `${node.getAttribute("class") ?? ""} cascade-node`);
      for (let index = 0; index < 11; index += 1) {
        const wrapper = document.createElementNS(root.namespaceURI, "g");
        node.parentNode?.insertBefore(wrapper, node);
        wrapper.appendChild(node);
      }
      const style = elements(root, "style")[0] as XmlElement;
      style.textContent = `${style.textContent ?? ""} .cascade-node{display:none!important}${"g ".repeat(11)}g{display:inline!important}`;
    }),
    "invalid:element-invisible",
  );
});

for (const importance of ["", "!important"]) {
  test(`同じ詳細度では後置非表示が勝つ ${importance}`, () => {
    rejects(
      nodeCascadeSvg(
        (id) => `#${id}{display:inline${importance}}#${id}{display:none${importance}}`,
      ),
      "invalid:element-invisible",
    );
  });
  test(`同じ詳細度では後置表示が勝つ ${importance}`, () => {
    const result = inspectClassReference(
      nodeCascadeSvg(
        (id) => `#${id}{display:none${importance}}#${id}{display:inline${importance}}`,
      ),
      "en",
    );
    expect(result.reasons).toEqual([]);
    expect(result.accepted).toBe(true);
  });
}

test("inline通常表示は多数IDの通常非表示より強い", () => {
  const result = inspectClassReference(
    nodeCascadeSvg((id) => `${`#${id}`.repeat(11)}{display:none}`, "display:inline"),
    "en",
  );
  expect(result.reasons).toEqual([]);
  expect(result.accepted).toBe(true);
});

test("root疑似class非表示はtagの後置表示より強い", () => {
  rejects(
    mutate((root) => {
      const style = elements(root, "style")[0] as XmlElement;
      style.textContent = `${style.textContent ?? ""} :root{display:none!important}svg{display:inline!important}`;
    }),
    "invalid:element-invisible",
  );
});

function nodeCommentSvg(stylesheet: string, inline?: string): string {
  return mutate((root) => {
    const node = elements(root, "g").find((element) => group(element, "node")) as XmlElement;
    node.setAttribute("data-comment", "/*literal*/");
    if (inline) node.setAttribute("style", inline);
    const style = elements(root, "style")[0] as XmlElement;
    style.textContent = `${style.textContent ?? ""} ${stylesheet}`;
  });
}

test("引用属性value内のcomment風literalを削除せず非表示selectorを適用する", () => {
  rejects(
    nodeCommentSvg('.node[data-comment="/*literal*/"]{display:none!important}'),
    "invalid:element-invisible",
  );
});

for (const method of ["inline", "stylesheet"] as const) {
  test(`コメント境界でproperty識別子を結合しない ${method}`, () => {
    const value = "display:none!important;dis/**/play:inline!important";
    rejects(
      nodeCommentSvg(
        method === "stylesheet" ? `.node{${value}}` : "",
        method === "inline" ? value : undefined,
      ),
      "unsupported:css-comment",
    );
  });
  test(`コメント境界でvalue識別子を結合しない ${method}`, () => {
    const value = "display:none!important;display:in/**/line!important";
    rejects(
      nodeCommentSvg(
        method === "stylesheet" ? `.node{${value}}` : "",
        method === "inline" ? value : undefined,
      ),
      "unsupported:css-comment",
    );
  });
  test(`token外コメントも保守未対応として拒否する ${method}`, () => {
    const value = "/*before*/display/**/:/**/none/**/!important/*after*/";
    rejects(
      nodeCommentSvg(
        method === "stylesheet" ? `.node{${value}}` : "",
        method === "inline" ? value : undefined,
      ),
      "unsupported:css-comment",
    );
  });
}

test("selector内コメント境界でclass名を推測して結合しない", () => {
  rejects(nodeCommentSvg(".no/**/de{display:none!important}"), "unsupported:css-comment");
});

test("閉じないコメントはstylesheetを未対応として拒否する", () => {
  rejects(nodeCommentSvg(".node{display:none!important}/*unclosed"), "unsupported:css-comment");
});

test("引用文字列内の偽declarationを分割して採用しない", () => {
  rejects(
    nodeCommentSvg(
      '.node{display:none!important;font-family:"x;display:inline!important;--tail:y"}',
    ),
    "unsupported:css-string",
  );
});

test("escaped quote後のcomment風literalをcommentとして削除しない", () => {
  rejects(
    nodeCommentSvg('.node[data-comment="\\"/*literal*/"]{display:none!important}'),
    "unsupported:css-selector",
  );
});

test("compound selector内コメントを空白へ変換して非表示を見落とさない", () => {
  rejects(
    nodeCommentSvg(".node/**/[data-comment]{display:none!important}"),
    "unsupported:css-comment",
  );
});

test("quote内comment風literalを持つfont metadataはbytesを保持して扱う", () => {
  const result = inspectClassReference(
    nodeCommentSvg('.node{font-family:"font/*literal*/family"}'),
    "en",
  );
  expect(result.reasons).toEqual([]);
  expect(result.accepted).toBe(true);
});
