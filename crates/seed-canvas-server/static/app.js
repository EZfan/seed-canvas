// seed-canvas client — vanilla JS, no build step.
//
// Responsibilities:
//   1. Wire up the "Copy URL" button on artwork detail pages.
//   2. Submit the seed-search form via Enter without page reload.
(() => {
  'use strict';

  document.addEventListener('click', (e) => {
    const target = e.target;
    if (!(target instanceof HTMLElement)) return;
    const copy = target.getAttribute('data-copy');
    if (!copy) return;
    const absolute = new URL(copy, window.location.origin).toString();
    navigator.clipboard?.writeText(absolute).then(
      () => flash(target, '✓ copied'),
      () => flash(target, 'copy failed'),
    );
  });

  function flash(button, message) {
    const original = button.textContent ?? '';
    button.textContent = message;
    setTimeout(() => { button.textContent = original; }, 1200);
  }

  // Keyboard: "/" focuses the search box.
  document.addEventListener('keydown', (e) => {
    if (e.key === '/' && !(e.target instanceof HTMLInputElement)) {
      e.preventDefault();
      const input = document.querySelector('.seed-search input');
      input?.focus();
    }
  });
})();