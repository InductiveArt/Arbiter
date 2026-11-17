function chorus(times: number): string {
  let tally = 0;
  for (let round = 0; round < times; round += 1) {
    tally += round * round;
  }
  if (tally > 100) {
    return "loud " + String(tally);
  }
  return "soft " + String(tally);
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

console.log(chorus(3), anthem(4));
