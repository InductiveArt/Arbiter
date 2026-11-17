async function lazy(): Promise<string> {
  const { Face } = await import("./card");
  return Face("x");
}

const where = "./card";
void import(where);
void lazy();
