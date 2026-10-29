import { fake } from "@demo/ui/testkit";
import { chunk } from "lodash";

console.log(fake(), chunk([1], 1));
