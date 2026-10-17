import { describe, expect, it } from "vitest";
import { parseSnippet } from "./snippet";

const S = "\u0002";
const E = "\u0003";

describe("parseSnippet", () => {
  it("returns one plain segment when there are no markers", () => {
    expect(parseSnippet("hello world")).toEqual([{ text: "hello world", highlight: false }]);
  });

  it("returns nothing for an empty string", () => {
    expect(parseSnippet("")).toEqual([]);
  });

  it("handles a highlight at the start", () => {
    expect(parseSnippet(`${S}Elf${E} king`)).toEqual([
      { text: "Elf", highlight: true },
      { text: " king", highlight: false },
    ]);
  });

  it("handles a highlight at the end", () => {
    expect(parseSnippet(`the ${S}king${E}`)).toEqual([
      { text: "the ", highlight: false },
      { text: "king", highlight: true },
    ]);
  });

  it("handles multiple highlights and ellipses", () => {
    expect(parseSnippet(`…the ${S}red${E} and ${S}blue${E} sea…`)).toEqual([
      { text: "…the ", highlight: false },
      { text: "red", highlight: true },
      { text: " and ", highlight: false },
      { text: "blue", highlight: true },
      { text: " sea…", highlight: false },
    ]);
  });

  it("highlights to the end when the end marker is missing", () => {
    expect(parseSnippet(`a ${S}b c`)).toEqual([
      { text: "a ", highlight: false },
      { text: "b c", highlight: true },
    ]);
  });

  it("ignores stray end markers", () => {
    expect(parseSnippet(`a${E}b`)).toEqual([
      { text: "a", highlight: false },
      { text: "b", highlight: false },
    ]);
  });
});
