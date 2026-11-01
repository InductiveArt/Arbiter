function verse(count: number): string {
  let total = 0;
  for (let step = 0; step < count; step += 1) {
    total += step * step;
  }
  if (total > 100) {
    return "high " + String(total);
  }
  return "low " + String(total);
}

function anthem(voices: number): number {
  let sum = 0;
  let loud = 0;
  for (let voice = 0; voice < voices; voice += 1) {
    if (voice % 2 === 0) {
      sum += voice;
    } else {
      loud += voice * 2;
    }
  }
  return sum + loud;
}

console.log(verse(3), anthem(4));
