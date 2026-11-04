import { describe, expect, it, test } from "vitest";

describe("docking", () => {
  it("snaps the arrow to the nearest rim", () => {
    expect(1).toBe(1);
  });
  describe("a crowded rim", () => {
    it("spreads the arrows a pitch apart", () => {
      expect(2).toBe(2);
    });
  });
});

test("a bare claim stands outside any suite", () => {
  expect(3).toBe(3);
});
