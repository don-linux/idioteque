import { describe, expect, it } from "vitest";
import { displayUrl, normalizeUrlInput } from "./browser-url";

describe("normalizeUrlInput", () => {
  it("returns null for empty or whitespace", () => {
    expect(normalizeUrlInput("")).toBeNull();
    expect(normalizeUrlInput("   ")).toBeNull();
  });

  it("keeps a value that already has a scheme", () => {
    expect(normalizeUrlInput("https://example.com")).toBe("https://example.com");
    expect(normalizeUrlInput("http://localhost:8080/x")).toBe("http://localhost:8080/x");
    expect(normalizeUrlInput("about:blank")).toBe("about:blank");
    expect(normalizeUrlInput("about:")).toBe("about:");
    expect(normalizeUrlInput("file:///tmp/a.md")).toBe("file:///tmp/a.md");
    expect(normalizeUrlInput("mailto:dev@example.com")).toBe("mailto:dev@example.com");
  });

  it("prepends https:// to localhost, IPv4, and dotted hosts", () => {
    expect(normalizeUrlInput("localhost")).toBe("https://localhost");
    expect(normalizeUrlInput("localhost:3000/app")).toBe("https://localhost:3000/app");
    expect(normalizeUrlInput("127.0.0.1")).toBe("https://127.0.0.1");
    expect(normalizeUrlInput("192.168.1.10:8080")).toBe("https://192.168.1.10:8080");
    expect(normalizeUrlInput("example.com")).toBe("https://example.com");
    expect(normalizeUrlInput("sub.example.com/path?q=1")).toBe(
      "https://sub.example.com/path?q=1",
    );
  });

  it("does not invent a search for a bare word or a phrase with spaces", () => {
    expect(normalizeUrlInput("idioteque")).toBeNull();
    expect(normalizeUrlInput("hello world")).toBeNull();
    expect(normalizeUrlInput("example.com has spaces")).toBeNull();
  });
});

describe("displayUrl", () => {
  it("shows about:blank as an empty bar", () => {
    expect(displayUrl("about:blank")).toBe("");
    expect(displayUrl("about:")).toBe("");
  });

  it("hides the https:// prefix and a trailing slash", () => {
    expect(displayUrl("https://example.com/")).toBe("example.com");
    expect(displayUrl("https://example.com/foo/")).toBe("example.com/foo");
    expect(displayUrl("https://localhost:3000")).toBe("localhost:3000");
  });

  it("leaves other schemes intact", () => {
    expect(displayUrl("http://example.com/")).toBe("http://example.com");
    expect(displayUrl("file:///tmp/a.md")).toBe("file:///tmp/a.md");
  });
});
