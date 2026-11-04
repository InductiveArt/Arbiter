import { fake } from "@demo/ui/testkit";
import { chunk } from "lodash";
import { expect } from "vitest";
import { it } from "vitest";

console.log(fake(), chunk([1], 1));

it("a tautology asserts nothing", () => {
  expect(1).toBe(1);
});

it.skip("a skipped claim is a suppression", () => {
  expect(fake()).toBe("fake");
});

const named = "a name the tree cannot read";
it(named, () => {
  expect(fake()).toBe("fake");
});
