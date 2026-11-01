// Helpers for a Google Flow project tab, pasted into the page. They work through its ordinary
// interface: the page itself sends every request, and nothing here touches its session or its checks.
//
//   repaint(change)      types the "same picture, only this change" prompt (the picture is pasted first)
//   cut(side), second(side)   the prompts that take an arm off cleanly: one arm from the whole figure,
//                        then the other from that result. Asking for both at once comes out badly.
//   grabMany([0, 2, 4])  downloads those tiles at 1K, newest first; refs lie between results, so check by eye
//
// A paste of the reference, repaint(...) and Return each take a few seconds; several can be sent without
// waiting for the one before. Downloads land in Downloads under Flow's own names, in the order asked.
window.askTxt = (t) => {
  const ed = [...document.querySelectorAll('[contenteditable="true"]')].pop();
  ed.focus();
  document.execCommand("insertText", false, t);
  return location.pathname.includes("edit") ? "EDIT" : "ok";
};
window.repaint = (change) =>
  window.askTxt(
    "Repaint exactly the attached picture in exactly its style: the same figure with everything the same, the same clothes, gear, pose, camera, size and framing. The only change: " +
      change +
      " Keep the same flat plain light grey background #C8C8C8. No text, no border.",
  );
window.noHead =
  "the figure has no head at all. The neck ends in a ragged raw bloody stump of torn flesh with a pale bone in it, and thick dark blood has soaked the collar and chest and runs down the clothes.";
window.stump =
  "The stump sits exactly where that arm joined the body, at the outer edge of its own shoulder, never on the chest or the belly. In its place the shoulder ends in a short ragged round stump, clearly visible and not hidden by any fur or cloth: torn skin at the rim, wet bright red raw flesh, and the pale white round end of the bone sticking out in the middle. Bright red blood runs from the stump down the side of the body in long streaks.";
window.cut = (side, extra) =>
  "the whole arm on the " + side +
  " side of the picture is chopped off completely at the shoulder joint. Nothing of that arm remains: no upper arm, no elbow, no forearm, no hand, no sleeve hanging, and anything that hand held is gone too. " +
  window.stump + " " + (extra || "The other arm, the head and everything else stay exactly as they are.");
window.second = (side) =>
  window.cut(side, "The stump of the other arm that is already there stays exactly as it is, so the figure has no arms at all now. The head and everything else stay exactly as they are.");

window.toTop = () => {
  document.querySelectorAll("*").forEach((e) => {
    if (e.scrollTop > 0) e.scrollTop = 0;
  });
  window.scrollTo(0, 0);
};
window.wait = (ms) => new Promise((r) => setTimeout(r, ms));
window.genTiles = () => [...document.querySelectorAll("flow-tile-container")].filter((t) => t.querySelector("img"));
window.ev = (el, type, C = MouseEvent) => {
  const r = el.getBoundingClientRect();
  el.dispatchEvent(new C(type, { bubbles: true, cancelable: true, view: window, clientX: r.left + r.width / 2, clientY: r.top + r.height / 2, button: type === "contextmenu" ? 2 : 0, pointerType: "mouse" }));
};
window.items = () => [...document.querySelectorAll('[role="menuitem"],[role="menu"] button,[role="menu"] a')];
// only ever the 1K item: the plan button beside 4K is never touched
window.grabOne = async (i) => {
  window.ev(window.genTiles()[i].querySelector("img"), "contextmenu");
  await window.wait(900);
  const dl = window.items().find((e) => /^\s*(download\s*)?Скачать/i.test(e.innerText || ""));
  if (!dl) return "no download item";
  for (const k of ["pointerover", "pointerenter", "pointermove"]) window.ev(dl, k, PointerEvent);
  for (const k of ["mouseover", "mouseenter", "mousemove"]) window.ev(dl, k);
  await window.wait(1500);
  const one = window.items().find((e) => /^\s*1K/.test((e.innerText || "").trim()));
  if (!one) return "no 1K";
  one.click();
  return "clicked";
};
window.grabRetry = async (i) => {
  for (let n = 0; n < 3; n++) {
    window.toTop();
    await window.wait(300);
    const s = await window.grabOne(i);
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    document.body.click();
    await window.wait(3000);
    if (s === "clicked") return s;
  }
  return "failed";
};
window.grabMany = async (list) => {
  const out = [];
  for (const i of list) out.push(await window.grabRetry(i));
  return out.join(",");
};
