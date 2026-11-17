import { Button, Chip, Widget } from "@demo/ui";
import { Card } from "./card";
import { probe } from "mockery";
import type { ProbeKind } from "typemock";

const kind: ProbeKind | null = null;
console.log(Button, Chip, Widget, Card("x"), probe, kind);
