// The intent plane: each case a dropdown wearing its ladder, its flows and
// its service cone - the lens from a promise down into the declared world.
// Only declarations live here; the shell script defines DATA and the helpers.

function verifiedNeeds() {
  const tried = {};
  for (const trial of (DATA.receipts && DATA.receipts.trials) || [])
    for (const need of trial.needs) tried[trial.case + "|" + need] = trial.passed;
  return tried;
}
function refutedNeeds() {
  const refused = new Set();
  for (const trial of (DATA.receipts && DATA.receipts.trials) || [])
    if (!trial.passed) for (const need of trial.needs) refused.add(trial.case + "|" + need);
  return refused;
}

// Everything a service stands on, transitively: boot needs and runtime dials.
function standsOnSet(name) {
  const cone = new Set();
  const frontier = [name];
  while (frontier.length) {
    const place = frontier.pop();
    const service = DATA.plans.services.find(s => s.name === place);
    if (!service) continue;
    const below = [...service.needs,
      ...DATA.plans.channels.filter(c => c.from === place).map(c => c.to)];
    for (const under of below) if (!cone.has(under)) { cone.add(under); frontier.push(under); }
  }
  return cone;
}
function caseSeeds(unit) {
  return DATA.plans.services.filter(s => s.unit === unit).map(s => s.name);
}
function caseCone(unit) {
  const cone = new Set();
  for (const seed of caseSeeds(unit)) {
    cone.add(seed);
    for (const under of standsOnSet(seed)) cone.add(under);
  }
  return cone;
}
function fallenServices(name) {
  return DATA.plans.services
    .filter(s => s.name !== name && standsOnSet(s.name).has(name))
    .map(s => s.name);
}
function fallenCases(name) {
  return ((DATA.brief && DATA.brief.seated) || [])
    .filter(c => c.unit && caseCone(c.unit).has(name))
    .map(c => c.name);
}

// The case's support cone drawn: doors above, services by depth, volumes below.
function coneBoard(holder, unit) {
  const seeds = caseSeeds(unit);
  if (!seeds.length) return;
  const depth = {};
  seeds.forEach(s => { depth[s] = 0; });
  let frontier = [...seeds];
  while (frontier.length) {
    const next = [];
    for (const place of frontier) {
      const service = DATA.plans.services.find(s => s.name === place);
      if (!service) continue;
      const below = [...service.needs,
        ...DATA.plans.channels.filter(c => c.from === place).map(c => c.to)];
      for (const under of below) if (!(under in depth)) { depth[under] = depth[place] + 1; next.push(under); }
    }
    frontier = next;
  }
  const rows = [];
  for (const name in depth) (rows[depth[name]] = rows[depth[name]] || []).push(name);
  const colw = 175, rowh = 86, nodeh = 30, top = 46, left = 16;
  const pos = {};
  rows.forEach((row, r) => row.sort().forEach((name, c) => {
    pos[name] = { x: left + c * colw, y: top + r * rowh };
  }));
  const width = left * 2 + Math.max(...rows.map(r => r.length)) * colw;
  const height = top + rows.length * rowh + 40;
  const svg = svgel("svg", { width: Math.max(width, 360), height, class: "board" });
  for (const door of DATA.plans.doors) {
    const b = pos[door.to] || pos[door.via];
    if (!b) continue;
    const label = svgel("text", { x: b.x, y: 16, class: "worldlabel" });
    label.textContent = door.domain + ":" + door.port + " ->";
    svg.appendChild(label);
    svg.appendChild(svgel("path", { class: "edge door", d: `M ${b.x + 65} 20 L ${b.x + 65} ${b.y - 4}` }));
  }
  for (const service of DATA.plans.services) {
    const a = pos[service.name];
    if (!a) continue;
    for (const need of service.needs) {
      const b = pos[need];
      if (!b) continue;
      svg.appendChild(svgel("path", { class: "edge",
        d: `M ${a.x + 65} ${a.y + nodeh} C ${a.x + 65} ${a.y + nodeh + 24}, ${b.x + 65} ${b.y - 24}, ${b.x + 65} ${b.y - 2}` }));
    }
  }
  for (const channel of DATA.plans.channels) {
    const a = pos[channel.from], b = pos[channel.to];
    if (!a || !b) continue;
    svg.appendChild(svgel("path", { class: "edge channel",
      d: `M ${a.x + 105} ${a.y + nodeh / 2} C ${(a.x + b.x) / 2 + 95} ${a.y}, ${(a.x + b.x) / 2 + 95} ${b.y}, ${b.x + 105} ${b.y + nodeh / 2}` }));
  }
  for (const service of DATA.plans.services) {
    const a = pos[service.name];
    if (!a) continue;
    service.volumes.forEach((name, at) => {
      const x = a.x + 20 + at * 14, y = a.y + nodeh + 18;
      svg.appendChild(svgel("path", { class: "edge door", d: `M ${a.x + 65} ${a.y + nodeh} L ${x + 45} ${y}` }));
      const group = svgel("g", { class: "node volumebox" });
      group.appendChild(svgel("rect", { x, y, width: 110, height: 22 }));
      const label = svgel("text", { x: x + 8, y: y + 15 });
      const vol = DATA.plans.volumes.find(v => v.name === name);
      label.textContent = name + (vol && vol.backup ? "" : " (unbacked)");
      group.appendChild(label);
      svg.appendChild(group);
    });
  }
  for (const name in pos) {
    const p = pos[name];
    const group = svgel("g", { class: "node" });
    group.appendChild(svgel("rect", { x: p.x, y: p.y, width: 130, height: nodeh }));
    const label = svgel("text", { x: p.x + 10, y: p.y + 20 });
    label.textContent = name;
    group.appendChild(label);
    group.onclick = () => servicePanel(name);
    svg.appendChild(group);
  }
  holder.appendChild(svg);
}

// One flow as a strip: door, services and volumes left to right, the answer
// turning back, the forks listed where they branch off.
function flowStrip(flow) {
  const wrap = el("div", "flow");
  wrap.appendChild(el("div", "dim", "flow: " + flow.name));
  const strip = el("div", "flowstrip");
  const chipFor = name => {
    if (DATA.plans.volumes.some(v => v.name === name)) return el("span", "flowvol", name);
    const chip = el("a", "flowservice", name);
    chip.onclick = () => servicePanel(name);
    return chip;
  };
  const arrow = does => {
    const step = el("span", "flowarrow");
    step.appendChild(el("span", null, "→"));
    if (does) step.appendChild(el("i", "dim", does));
    return step;
  };
  for (const hop of flow.steps) {
    if (hop.back != null) { strip.appendChild(el("span", "flowback", "↩ " + hop.back)); continue; }
    if (hop.via) {
      strip.appendChild(el("span", "flowdoor", hop.via));
      strip.appendChild(arrow(hop.does));
      strip.appendChild(chipFor(hop.to));
    } else if (hop.from && hop.to) {
      strip.appendChild(arrow(hop.does));
      strip.appendChild(chipFor(hop.to));
    }
  }
  wrap.appendChild(strip);
  for (const fork of flow.forks)
    wrap.appendChild(el("div", "flowfork",
      "at " + fork.at + ", when " + fork.when + " ↩ " + fork.back));
  return wrap;
}

function docket(view) {
  if (!DATA.brief) { view.appendChild(el("div", "empty", "no brief written")); return; }
  const tried = verifiedNeeds();
  const seat = {};
  for (const one of DATA.brief.seated || []) seat[one.name] = one;
  const byCase = {};
  for (const need of DATA.brief.needs) (byCase[need.case] = byCase[need.case] || []).push(need);
  const flowsBy = {};
  for (const flow of DATA.brief.flows || []) (flowsBy[flow.case] = flowsBy[flow.case] || []).push(flow);
  const names = Object.keys(byCase);
  for (const name of names) {
    const needs = byCase[name];
    const home = seat[name] || {};
    const group = el("details", "group");
    if (names.length === 1) group.open = true;
    const summary = el("summary");
    summary.appendChild(el("b", null, name));
    const verified = needs.filter(n => tried[name + "|" + n.need] === true).length;
    const refuted = needs.filter(n => tried[name + "|" + n.need] === false).length;
    const unanswered = needs.filter(n => n.state === "unanswered").length;
    let roll = needs.length + " needs";
    if (verified) roll += " · " + verified + " verified";
    summary.appendChild(el("span", "dim", " " + roll));
    if (refuted) summary.appendChild(el("span", "bad", " " + refuted + " refuted"));
    if (unanswered) summary.appendChild(el("span", "bad", " " + unanswered + " unanswered"));
    const trouble = home.unit ? DATA.findings.filter(f => within(f, home.unit)).length : 0;
    if (trouble) summary.appendChild(el("span", "bad", " " + trouble + " findings in its territory"));
    group.appendChild(summary);
    const body = el("div", "body");
    if (home.serves) body.appendChild(el("div", "dim", "serves: " + home.serves));
    for (const need of needs) {
      const row = el("div");
      row.appendChild(el("span", "state-" + need.state, need.state.padEnd(11)));
      const outcome = tried[name + "|" + need.need];
      if (outcome === true) row.appendChild(el("span", "good", "verified  "));
      else if (outcome === false) row.appendChild(el("span", "bad", "refuted  "));
      row.appendChild(document.createTextNode(need.need));
      if (need.claims) row.appendChild(el("span", "dim",
        " — " + need.claims + (need.claims === 1 ? " claim" : " claims")));
      body.appendChild(row);
    }
    for (const flow of flowsBy[name] || []) body.appendChild(flowStrip(flow));
    if (home.unit) {
      const territory = el("div");
      territory.appendChild(el("span", "dim", "territory: "));
      const toGraph = el("a", null, "unit " + home.unit + " — open its graph");
      toGraph.onclick = () => { pending = { select: home.unit }; go("graph/units"); };
      territory.appendChild(toGraph);
      if (trouble) {
        const toFindings = el("a", "bad", "  " + trouble + " findings");
        toFindings.href = "#findings/" + (home.unit === "." ? "" : home.unit);
        territory.appendChild(toFindings);
      }
      body.appendChild(territory);
      coneBoard(body, home.unit);
    }
    group.appendChild(body);
    view.appendChild(group);
  }
  if (DATA.receipts)
    view.appendChild(el("div", "dim", "tested is the repository's own testimony; " +
      "verified is arbiter's — " + DATA.receipts.examiner +
      ", sequence " + DATA.receipts.sequence + ", this run"));
}
