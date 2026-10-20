// Esc hands focus from an edited line back to its notes or transcript (SPEC 7.11) and leaves the
// view where it was. In the app, WebKit still scrolled after focus({preventScroll}) (owner's
// screenshots, 2026-10-20), so the old caret is dropped and the scroll put back as well.
// Call it before the save; run what it returns afterwards.
export function handBack(container: HTMLElement | null) {
  const scroller = container?.closest<HTMLElement>(".docbody"), top = scroller?.scrollTop ?? 0;
  return () => {
    getSelection()?.removeAllRanges();
    container?.focus({preventScroll: true});
    const back = () => { if (scroller) scroller.scrollTop = top; };
    back(); requestAnimationFrame(back);
  };
}
